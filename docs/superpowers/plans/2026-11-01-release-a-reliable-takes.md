# Release A — Takes Don't Break Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make recorded takes robust. Listening never silently stops, the highlight keeps up, retakes take one tap, and the reader chooses what happens at the end of a slide.

**Architecture:** All decisions live in `crates/core` (pure, unit-tested). `crates/ui` (Dioxus/WASM) wires actions, settings and the mic lifecycle to the core. `MainActivity.kt` gains only mechanical retry/reset/tagging. A `take` counter in `Session` lets the UI tell the native host to reset the recognizer and lets the core drop stale speech.

**Tech Stack:** Rust (rusty-core, Dioxus 0.7 WASM UI), Kotlin Android host with Vosk, Python dev orchestration scripts.

**Spec:** `docs/superpowers/specs/2026-11-01-release-a-reliable-takes-design.md`

## Global Constraints

- Offline only. No new network calls, services or dependencies.
- Kotlin may restart, reset and tag. It must never decide matching, playback or timing.
- Defaults preserve today's behaviour: `hold_at_end = false`, `countdown_secs = 5.0`.
- `countdown_secs` is clamped to `1.0..=15.0`.
- Speech events without a `take` field are accepted (backward compatible).
- All existing tests keep passing, including `real_offline_recognition_stream`.
- Clippy must pass with warnings as errors.
- Commands run from `rustyprompter/` through `python scripts\task.py …`, which sets the isolated toolchain.
  - Core tests: `python scripts\task.py cargo test -p rusty-core`
  - Lint: `python scripts\task.py cargo clippy -p rusty-core -p rusty-companion --all-targets -- -D warnings`
- The workspace is not currently a git repository. Commit steps apply once the user has initialized one; otherwise skip them.

## Review Focus

1. **Speech from before a rewind arrives after it.** A final result queued before Retake must not move the highlight on the restarted slide. Pinned in Task 2 (`stale_take_speech_is_ignored`).
2. **A PC edit deletes the slide you're holding on.** Expect `Paused` at the start of the fallback slide, not `Holding` on a slide that was never read. Pinned in Task 4 (`revision_removing_held_slide_pauses`).
3. **A slide with no punctuation at all.** Back a sentence goes to the slide start, and never panics. Pinned in Task 3 (`no_punctuation_is_one_sentence`).
4. **Countdown length changed while a countdown runs.** The remaining time is unchanged and the next countdown uses the new length. Pinned in Task 4 (`configure_mid_countdown_keeps_remaining`).
5. **Retake after the final slide finished.** Expect `Reading` from the start of the last slide, not of the whole script. Pinned in Task 2 (`retake_after_finished_reads_last_slide`).

---

### Task 1: Faster highlight commit in `Tracker`

**Files:**
- Modify: `crates/core/src/lib.rs` — `Tracker::accept`, non-far voting tail (currently around lines 258–272)
- Test: `crates/core/tests/highlight.rs` (create)

**Interfaces:**
- Consumes: existing `Tracker::new`, `Tracker::accept(&str, bool) -> Option<usize>`.
- Produces: no new API; behaviour change only.

- [ ] **Step 1: Write the failing tests**

```rust
use rusty_core::*;
const TEXT: &str = "Welcome to our bright green studio and the camera is ready";
// words: welcome0 to1 our2 bright3 green4 studio5 and6 the7 camera8 is9 ready10

#[test]
fn growing_partial_commits_previous_end() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright", false);
    assert_eq!(t.position, None);
    t.accept("welcome to our bright green studio", false);
    assert_eq!(t.position, Some(3));
}
#[test]
fn rewritten_newest_word_does_not_overshoot() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright", false);
    t.accept("welcome to our bright green studio", false);
    t.accept("welcome to our bright green stadium", false);
    assert!(t.position <= Some(4));
}
#[test]
fn common_only_extension_does_not_commit() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our bright green studio", false);
    t.accept("welcome to our bright green studio and the", false);
    assert_eq!(t.position, None);
}
#[test]
fn extension_beyond_four_words_does_not_commit() {
    let mut t = Tracker::new(TEXT);
    t.accept("welcome to our", false);
    t.accept("welcome to our bright green studio and the camera", false);
    assert_eq!(t.position, None);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `python scripts\task.py cargo test -p rusty-core --test highlight`
Expected: `growing_partial_commits_previous_end` FAILS (position `None`). The others may pass already; that's fine.

- [ ] **Step 3: Implement the early commit in `Tracker::accept`**

On the non-far path, before the `partial_end == Some(end)` vote check, add the spec §4 rule: if `partial_end == Some(prev)`, `prev < end && end <= prev + 4`, any of `self.words[prev+1..=end]` is not `is_common`, and `self.position.is_none_or(|p| prev > p)`, then set `self.position = Some(prev)`. Leave the existing vote/`is_final` logic for `end` unchanged after it.

- [ ] **Step 4: Run the whole core suite**

Run: `python scripts\task.py cargo test -p rusty-core`
Expected: all tests PASS, including `real_offline_recognition_stream` and the four new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): commit confirmed part of growing partials"
```

---

### Task 2: `take` counter, stale-speech filter, `Retake`

**Files:**
- Modify: `crates/core/src/lib.rs` — `Session` struct, `new`, `move_to`, `seek`, `apply_revision`, `speech`; new `retake`, `speech_for_take`
- Test: `crates/core/tests/retakes.rs` (create)

**Interfaces:**
- Consumes: existing `Session`, `Phase`.
- Produces:
  - `pub take: u64` on `Session` (starts at 0).
  - `pub fn retake(&mut self)`.
  - `pub fn speech_for_take(&mut self, text: &str, is_final: bool, take: Option<u64>)`, which ignores the event when `take` is `Some(t)` and `t != self.take`, and otherwise calls `speech`.
  - `pub(crate) fn rewind_to(&mut self, next_word: usize)`: the shared seek + phase rule + `take += 1` + `remaining_ms` reset, reused by Task 3.

- [ ] **Step 1: Write the failing tests**

Use a two-slide `script()` helper like `playback.rs` (slide "a": `"Welcome to our bright green studio"`, slide "b": `"This is the next slide"`).

```rust
#[test] fn take_increments_on_moves_and_rewinds() {
    let mut s = Session::new(script()); let t0 = s.take;
    s.start(); s.next(); assert_eq!(s.take, t0 + 1);
    s.seek(2); assert_eq!(s.take, t0 + 2);
    s.retake(); assert_eq!(s.take, t0 + 3);
}
#[test] fn retake_mid_read_restarts_slide_reading() {
    let mut s = Session::new(script()); s.start();
    s.speech("welcome to our bright", true);
    s.retake();
    assert_eq!(s.phase, Phase::Reading);
    assert_eq!(s.tracker.position, None);
    assert_eq!(s.slide_index, 0);
}
#[test] fn retake_while_paused_stays_paused_at_start() {
    let mut s = Session::new(script()); s.start();
    s.speech("welcome to our bright", true); s.pause();
    s.retake();
    assert_eq!(s.phase, Phase::Paused); assert_eq!(s.tracker.position, None);
}
#[test] fn retake_in_countdown_cancels_it() {
    let mut s = Session::new(script()); s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Countdown);
    s.retake();
    assert_eq!(s.phase, Phase::Reading); assert_eq!(s.slide_index, 0);
}
#[test] fn retake_after_finished_reads_last_slide() {
    let mut s = Session::new(script()); s.start(); s.next();
    s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
    s.retake();
    assert_eq!(s.phase, Phase::Reading); assert_eq!(s.slide_index, 1);
    assert_eq!(s.tracker.position, None);
}
#[test] fn stale_take_speech_is_ignored() {
    let mut s = Session::new(script()); s.start();
    let old = s.take; s.retake();
    s.speech_for_take("welcome to our bright green", true, Some(old));
    assert_eq!(s.tracker.position, None);
    s.speech_for_take("welcome to our bright green", true, Some(s.take));
    assert!(s.tracker.position.is_some());
}
#[test] fn untagged_speech_is_accepted() {
    let mut s = Session::new(script()); s.start();
    s.speech_for_take("welcome to our bright green", true, None);
    assert!(s.tracker.position.is_some());
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `python scripts\task.py cargo test -p rusty-core --test retakes`
Expected: compile FAIL (no `take`, `retake`, `speech_for_take`).

- [ ] **Step 3: Implement**

- Add `take` and increment it in `move_to`, `seek` and `apply_revision`.
- Add `rewind_to(next_word)`:
  - `tracker.seek(next_word)` and `remaining_ms` reset.
  - `resume = Reading` and `take += 1`.
  - Phase: `Ready`/`Paused` unchanged; `Reading`/`Countdown`/`Finished` (and `Holding` once Task 4 exists) → `Reading`.
- `retake()` is `rewind_to(0)`.
- Make `seek` reuse the same take bump (its existing phase rule already matches).

- [ ] **Step 4: Run the core suite**

Run: `python scripts\task.py cargo test -p rusty-core`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): take counter, stale speech filter, retake slide"
```

---

### Task 3: Sentence starts and Back a sentence

**Files:**
- Modify: `crates/core/src/lib.rs` — new `sentence_starts`, `Session::back_sentence`
- Test: `crates/core/tests/retakes.rs`

**Interfaces:**
- Consumes: `Word { text, start, end }`, `words()`, `Session::rewind_to` (Task 2).
- Produces:
  - `pub fn sentence_starts(text: &str, words: &[Word]) -> Vec<usize>`, ascending, always starting with 0 when `words` is non-empty, and empty when it is empty.
  - `pub fn back_sentence(&mut self)`.

- [ ] **Step 1: Write the failing tests**

```rust
fn starts(text: &str) -> Vec<usize> { sentence_starts(text, &words(text)) }
#[test] fn abbreviations_verses_and_blank_lines() {
    // dr0 smith1 spoke2 read3 john4 three5 sixteen6 today7 new8 part9 here10
    assert_eq!(starts("Dr. Smith spoke. Read John 3:16 today!\n\nnew part here"), vec![0, 3, 8]);
}
#[test] fn cue_inside_gap_still_breaks() {
    assert_eq!(starts("Hello there. [pause] Next one"), vec![0, 2]);
}
#[test] fn lowercase_after_period_is_not_a_start() {
    assert_eq!(starts("We use e.g. this one"), vec![0]);
}
#[test] fn no_punctuation_is_one_sentence() {
    assert_eq!(starts("just words with no punctuation at all"), vec![0]);
}
fn sentences() -> Session { /* one-slide script: "One two three. Four five six. Seven eight nine." , started */ }
#[test] fn back_goes_to_current_sentence_start() {
    let mut s = sentences(); s.seek(8); s.back_sentence();
    assert_eq!(s.tracker.position, Some(5)); // next word = 6 ("seven")
}
#[test] fn back_within_first_two_words_goes_further() {
    let mut s = sentences(); s.seek(7); s.back_sentence();
    assert_eq!(s.tracker.position, Some(2)); // next word = 3 ("four")
}
#[test] fn back_at_first_sentence_stays_at_zero() {
    let mut s = sentences(); s.seek(1); s.back_sentence();
    assert_eq!(s.tracker.position, None);
}
#[test] fn back_sentence_bumps_take_and_reads() {
    let mut s = sentences(); s.seek(8); let t = s.take; s.back_sentence();
    assert_eq!(s.take, t + 1); assert_eq!(s.phase, Phase::Reading);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `python scripts\task.py cargo test -p rusty-core --test retakes`
Expected: compile FAIL (`sentence_starts`, `back_sentence` missing).

- [ ] **Step 3: Implement `sentence_starts` and `back_sentence`**

Follow the boundary rules in spec §5:
- A blank line in the gap counts as a sentence start.
- Otherwise the gap must contain `.`, `?` or `!`, and the next word's first character must be uppercase or a digit.
- The previous word must not be one of `dr mr mrs ms st rev vs etc jr sr no`.

`back_sentence`:
- `next = position.map_or(0, |p| p + 1)`.
- `c` is the largest start `<= next`.
- The target is `c` if `next - c >= 2`; otherwise the largest start `< c`, or 0 if there is none.
- Finish with `rewind_to(target)`.

- [ ] **Step 4: Run core suite**

Run: `python scripts\task.py cargo test -p rusty-core`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): back a sentence"
```

---

### Task 4: Hold mode, countdown length, `listening()`

**Files:**
- Modify: `crates/core/src/lib.rs`:
  - `Phase` (add `Holding`, add `listening()`).
  - `Settings` (add `hold_at_end`, `countdown_secs`; update `Default` and `clamp`).
  - `Session` (add `countdown_ms`, `hold_at_end`, `configure`). Replace every `5000` with `self.countdown_ms`. Update `speech`, `start`, `pause`, `tick`, `rewind_to` and `apply_revision`.
- Test: `crates/core/tests/hold.rs` (create)

**Interfaces:**
- Consumes: `Session::rewind_to`, `take` (Task 2).
- Produces:
  - `Phase::Holding`.
  - `Phase::listening(self) -> bool`: true for `Reading | Countdown | Holding`.
  - `Settings { hold_at_end: bool, countdown_secs: f64, .. }`.
  - `Session::configure(&mut self, settings: &Settings)`.
  - `pub countdown_ms: u64`, defaulting to 5000.

- [ ] **Step 1: Write the failing tests**

```rust
fn held() -> Session { let mut s = Session::new(script());
    s.configure(&Settings { hold_at_end: true, ..Settings::default() }); s.start(); s }
#[test] fn end_of_slide_holds() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    assert_eq!(s.phase, Phase::Holding); s.tick(60_000);
    assert_eq!(s.phase, Phase::Holding); assert_eq!(s.slide_index, 0);
}
#[test] fn final_slide_still_finishes_when_holding() {
    let mut s = held(); s.next(); s.speech("this is the next slide", true);
    assert_eq!(s.phase, Phase::Finished);
}
#[test] fn start_from_holding_goes_next() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    s.start(); assert_eq!((s.slide_index, s.phase), (1, Phase::Reading));
}
#[test] fn pause_and_resume_return_to_holding() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    s.pause(); assert_eq!(s.phase, Phase::Paused);
    s.start(); assert_eq!(s.phase, Phase::Holding);
}
#[test] fn retake_from_holding_reads_slide_start() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    s.retake(); assert_eq!((s.slide_index, s.phase), (0, Phase::Reading));
}
#[test] fn revision_applies_while_holding() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    let mut u = script(); u.revision = 2; u.slides[1].text = "Changed next slide".into();
    assert!(s.apply_revision(u)); assert_eq!(s.phase, Phase::Holding);
}
#[test] fn revision_removing_held_slide_pauses() {
    let mut s = held(); s.speech("welcome to our bright green studio", true);
    let mut u = script(); u.revision = 2; u.slides.remove(0);
    assert!(s.apply_revision(u));
    assert_eq!(s.phase, Phase::Paused); assert_eq!(s.tracker.position, None);
}
#[test] fn countdown_length_is_configurable() {
    let mut s = Session::new(script());
    s.configure(&Settings { countdown_secs: 1.0, ..Settings::default() }); s.start();
    s.speech("welcome to our bright green studio", true);
    assert_eq!(s.remaining_ms, 1000); s.tick(1000); assert_eq!(s.slide_index, 1);
}
#[test] fn configure_mid_countdown_keeps_remaining() {
    let mut s = Session::new(script()); s.start();
    s.speech("welcome to our bright green studio", true); s.tick(1000);
    s.configure(&Settings { countdown_secs: 15.0, ..Settings::default() });
    assert_eq!(s.remaining_ms, 4000); assert_eq!(s.countdown_ms, 15_000);
}
#[test] fn settings_clamp_and_old_json() {
    let mut st: Settings = serde_json::from_str(r#"{"font_size":40}"#).unwrap();
    assert!(!st.hold_at_end); assert_eq!(st.countdown_secs, 5.0);
    st.countdown_secs = 99.0; st.clamp(); assert_eq!(st.countdown_secs, 15.0);
    st.countdown_secs = 0.0; st.clamp(); assert_eq!(st.countdown_secs, 1.0);
}
#[test] fn listening_phases() {
    assert!(Phase::Reading.listening() && Phase::Countdown.listening() && Phase::Holding.listening());
    assert!(!Phase::Paused.listening() && !Phase::Ready.listening() && !Phase::Finished.listening());
}
```

If `serde_json` isn't a dev-dependency of `rusty-core`, add it under `[dev-dependencies]` in `crates/core/Cargo.toml` (it's already in the workspace lockfile).

- [ ] **Step 2: Run to verify they fail**

Run: `python scripts\task.py cargo test -p rusty-core --test hold`
Expected: compile FAIL.

- [ ] **Step 3: Implement per spec §6**

- `configure` sets `countdown_ms = (settings.countdown_secs * 1000.0).round() as u64` and `hold_at_end`. It never touches `remaining_ms`.
- `speech`: at the last word, if a later playable slide exists, enter `Holding` when `hold_at_end` is set and `Countdown` otherwise.
- `start`: `Holding` calls `next()`.
- `pause`: `Holding` sets `resume = Holding`.
- `tick` ignores `Holding`, and `running()` stays false for it.
- `seek` and `rewind_to` treat `Holding` like `Countdown` (→ `Reading`). Tapping a word while holding resumes reading there.
- `apply_revision`: if the phase was `Holding` and the old slide was not found, set the phase to `Paused`.

- [ ] **Step 4: Run core suite and clippy**

Run: `python scripts\task.py cargo test -p rusty-core`, then the clippy command from Global Constraints.
Expected: all PASS, and clippy shows no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): hold at end of slide and configurable countdown"
```

---

### Task 5: UI wiring — pump, actions, controls, settings

**Files:**
- Modify: `crates/ui/src/app.rs`:
  - `Ui::action` (~273–292).
  - `save_settings` (~294).
  - Session creation (~237).
  - `pump()` speech handler (~471) and the stall/mic block (~522–549).
  - Prompter phase label, Hold card and footer (~1340–1418).
  - Settings overlay (~1713).
  - The slide-list overlay and the help text (~1524).
- Modify: `scripts/qa.py` (test adapter only)

**Interfaces:**
- Consumes: `Session::{retake, back_sentence, speech_for_take, configure, take}`, `Phase::{Holding, listening}`, and `Settings::{hold_at_end, countdown_secs}`.
- Produces (native protocol for Task 6):
  - `native_call("start", "<take>")` and `native_call("reset", "<take>")`.
  - Speech events may carry `"take": <u64>`.

- [ ] **Step 1: Wire core calls**

- `Ui::action` gains `"retake" => s.retake()` and `"back_sentence" => s.back_sentence()`.
- Call `session.configure(&self.settings)` after `Session::new` and in `save_settings`.
- The `speech` event calls `speech_for_take(text, final, event["take"].as_u64())`.

- [ ] **Step 2: Replace the stall pause and the mic logic in `pump()`**

- Delete the `elapsed > 1500` pause. Tick the countdown with `elapsed.min(250)`.
- `desired` becomes `native && voice_ready && session.phase.listening()`.
- Key on `(script.id, revision, take)`:
  - Open/close transitions send `stop`, or `start` with the take.
  - When the key changes while the mic stays open, send `reset` with the take.

- [ ] **Step 3: Prompter UI**

- Footer order is ↺ Retake · ↶ Sentence · ‹ Back · Start/Pause · Next ›, with aria labels "Retake this slide" and "Back one sentence".
- The main button reads "Next" in `Holding`.
- In `Holding`, show an overlay card with "End of slide" and two buttons: **Next slide** (action `toggle`) and **Retake** (action `retake`).
- The slide-list overlay gets a "From the top" button (action `restart`).
- In the settings overlay:
  - Add `SettingSwitch { label: "Hold at end of slide", field: "hold_at_end" }`.
  - Add `SettingSlider` "Countdown", 1–15 s, step 1, field `countdown_secs`, rendered only when Hold is off.
  - Extend the field-name matches in `SettingSwitch`/`SettingSlider` for the new fields.
- Help text: replace "starts a five-second countdown" with wording that depends on the setting: "starts a {n}-second countdown" or "holds until you tap Next".

- [ ] **Step 4: Extend the QA adapter in `scripts/qa.py`**

- The `command` handler writes `Microphone: reset (take N)` into `#qa-audio` when it receives `reset`.
- Add a button "Speak stale take" that pushes `{type:'speech', text:<slide one text>, final:true, take:999999}`. Use a large positive number: `-1` parses as `None` through `as_u64()` and would be accepted.

- [ ] **Step 5: Build and browser-check**

Run: `python scripts\bundle_ui.py`, then `python scripts\task.py cargo build --release -p rusty-companion`, then `python scripts\qa.py`. Open `http://127.0.0.1:8877/assets/qa-tablet.html` in the built-in browser.

Expected:
- **Footer:** shows the five controls.
- **Retake and Sentence:** after speaking slide one partly, Retake returns to the start and the audio label shows `reset`. Sentence moves back one sentence.
- **Stale take:** "Speak stale take" leaves the highlight unmoved.
- **Hold on:** speaking slide one shows the End-of-slide card and the mic label stays `listening`. Next slide advances.
- **Hold off, countdown 2 s:** the countdown shows 2.
- **Queued edits:** "Queue PC update" during Hold applies immediately.

Stop the QA server afterwards using the pid in `.tools/rustyprompter/qa/process.json`.

- [ ] **Step 6: Commit**

```bash
git add crates/ui scripts/qa.py web-dist
git commit -m "feat(ui): retake, back sentence, hold card, countdown setting, mic reset"
```

---

### Task 6: Android host — retry, reset, take tagging

**Files:**
- Modify: `android/app/src/main/java/com/rustyprompter/app/MainActivity.kt`:
  - The command dispatcher (search for `"start"` / `"stop"` handling).
  - `startAudio` (~158–169) and `stopAudio`.

**Interfaces:**
- Consumes: the native protocol from Task 5.
- Produces:
  - Speech events include `"take"` once a take has been set.
  - `audio_error` is emitted only after 3 failed retries.

- [ ] **Step 1: Implement**

- Add `private var take: Long? = null` and `private var retries = 0`.
- `start` parses the payload into `take` (ignore it if blank or invalid) and then follows the existing start path.
- `reset` sets `take` and calls `service?.reset()`. Ignore it if `service` is null.
- `result()` adds `.put("take", take)` when `take` is non-null, and sets `retries = 0`.
- In `onTimeout` and `onError` (same generation and `wantedAudio`):
  - If `retries < 3`, `stopAudio(false)` and `main.postDelayed({ if (wantedAudio && !destroyed) startAudio() }, delays[retries++])` with `delays = longArrayOf(500, 1000, 2000)`.
  - Otherwise, keep the existing `stopAudio()` + `audio_error`.

- [ ] **Step 2: Build and lint**

Run: `python scripts\build.py`
Expected: the APK builds, Android lint shows no errors, and `dist/RustyPrompter-Android.apk` is updated.

- [ ] **Step 3: Commit**

```bash
git add android
git commit -m "feat(android): self-healing mic, recognizer reset, take tagging"
```

---

### Task 7: Docs and full verification

**Files:**
- Modify: `README.md` (feature bullets), `VALIDATION.md`, `INSTALL.md` (if it describes the controls or the 5-second countdown)

- [ ] **Step 1: Update docs**

- README bullets: Retake/Back a sentence, Hold at end of slide, configurable countdown, and self-recovering listening.
- VALIDATION.md: new automated test counts. Under physical-device acceptance, add the three items from spec §Testing "Device", explicitly not yet claimed.

- [ ] **Step 2: Full verification**

Run:
- `python scripts\task.py cargo test -p rusty-core -p rusty-companion`
- The clippy command from Global Constraints.
- `python scripts\verify.py`

Expected: every test passes (the original 36 core + 18 companion, plus the new ones), clippy is clean, and artifact verification passes.

- [ ] **Step 3: Commit**

```bash
git add README.md VALIDATION.md INSTALL.md
git commit -m "docs: Release A features and validation"
```
