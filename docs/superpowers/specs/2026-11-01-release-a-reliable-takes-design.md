# Release A — Takes Don't Break

Date: 2026-11-01
Status: Design approved in conversation; awaiting written-spec review.

## Context

RustyPrompter is used to record video (retakes are possible). The user's top
concern is tracking reliability, but the app has not yet been used much on the
Galaxy Tab A7 Lite, and the user chose not to build diagnostics tooling first.
Twelve features were shortlisted and split into three releases:

- **A — Takes don't break** (this spec): keep listening, faster highlight,
  Retake / Back a sentence, Hold mode + countdown length.
- **B — Hands-free and feedback**: Bluetooth clicker/pedal, number and verse
  normalization, "I hear you" dot + lost-place hint, 3-2-1 pre-roll, run-time
  estimate.
- **C — Smarter tracking**: guarded end-of-slide, voice backward resync,
  script-vocabulary Vosk grammar.

B and C get their own specs later. B's clicker builds on A's Retake and Hold.

## Goals

1. Listening never silently stops during a take.
2. The highlight keeps up with fluent reading.
3. Retaking a slide or a sentence is one tap.
4. The reader controls what happens at the end of a slide.

## Constraints

- Offline, no subscriptions, private LAN only.
- Rust owns all decisions (matching, playback, timing). Kotlin stays a thin
  adapter: it may restart, reset and tag, never decide.
- Existing behaviour stays the default: Hold is off and the countdown remains
  5 s unless the user changes settings.
- All 36 existing core tests, 18 companion tests and the 64-event Vosk replay
  must keep passing. Clippy with warnings-as-errors must pass.

## Non-goals

Diagnostics recording, any Release B/C feature, protocol changes between the
tablet and the PC companion, and changes to the PC Studio editor.

## Design

### 1. No stall pauses

`crates/ui/src/app.rs` `pump()` currently calls `ui.action("pause")` when a
loop iteration's `elapsed > 1500`. Remove that. Backgrounding is already
covered by Kotlin `onPause` and the browser `visibilitychange` handler in
`interop.js`, both of which push a `pause` event.

Countdown ticks use `elapsed.min(250)` so a stall cannot consume the
countdown in one jump.

### 2. Self-healing microphone (Kotlin)

In `MainActivity.kt`, `onTimeout` and `onError` currently call `stopAudio()`
and emit `audio_error`. New behaviour while `wantedAudio` is true:

- Restart listening (`stopAudio(false)` then `startAudio()`) after 500 ms,
  then 1000 ms, then 2000 ms.
- Any speech result resets the retry count to 0.
- After the third failed retry, fall back to the current behaviour
  (`stopAudio()` + `audio_error`, "Tap Resume").
- Stale retries are ignored using the existing `audioGeneration` check.

### 3. Microphone stays open for the whole take

**Mic wanted** becomes: native and voice ready and phase is `Reading`,
`Countdown` or `Holding` (new, see §6). It closes on `Paused`, `Ready`,
`Finished`, or app backgrounding. `Session::speech` already ignores input
outside `Reading`, so nothing new is matched during Countdown/Holding.

**Take counter.** `Session` gains `pub take: u64`, incremented by every
rewind or slide move: `move_to`, `retake`, `back_sentence`, `seek`, and
`apply_revision`. `pump()` replaces the `slide_key` stop/start logic:

- Mic open/close transitions still use `start` / `stop`.
- When the mic stays open but `(script id, revision, take)` changes, the UI
  calls `native_call("reset", "<take>")` instead of stop/start.

**Kotlin `reset` command.** Stores the given take number, calls
`service.reset()` on the Vosk `SpeechService` (no new `Recognizer` or audio
stream), and includes `"take": <n>` in every `speech` event. `start` takes the
same payload so the first events of a take are tagged.

**UI filtering.** The `speech` event handler drops events whose `take` does
not equal the session's current `take`. Events without a `take` field (older
native host, PC browser) are accepted, preserving current behaviour.

### 4. Faster highlight (core)

In `Tracker::accept` (`crates/core/src/lib.rs`), on the non-far path, before
voting:

- If `partial_end` is `Some(prev)` and the new candidate `end` satisfies
  `prev < end <= prev + 4`, and at least one script word in `prev+1..=end` is
  not `is_common`, and `prev > position`, set `position = Some(prev)`.
- Then continue with the existing vote/`is_final` logic for `end`.

Position never decreases. The newest word still needs confirmation (a second
partial or a final result), so recognizer rewrites of the latest word cannot
push the highlight ahead.

### 5. Retake slide and Back a sentence (core)

New `Session` methods:

- `retake()`: tracker reset to the current slide's start (`seek(0)`),
  `remaining_ms = countdown_ms`, `take += 1`. Phase: `Ready` and `Paused`
  stay as they are. `Reading`, `Countdown`, `Holding` and `Finished` become
  `Reading`. `resume` becomes `Reading`.
- `back_sentence()`: let `next = position + 1` (0 if none). Compute sentence
  starts for the slide. Let `c` be the largest start `<= next`. If
  `next - c >= 2`, target `c`. Otherwise target the largest start `< c`, or
  0 if there is none. Then seek to the target with the same phase rules and
  `take` bump as `retake()`.

`pub fn sentence_starts(text: &str, words: &[Word]) -> Vec<usize>`. Word
index `i` is a start when any of these hold:

- `i == 0`.
- The text between `words[i-1].end` and `words[i].start` contains a blank
  line (`\n` followed by optional whitespace and another `\n`).
- That gap contains `.`, `?` or `!`, the next word's first character in the
  original text is uppercase or a digit, and the previous word (lowercased)
  is not one of: `dr`, `mr`, `mrs`, `ms`, `st`, `rev`, `vs`, `etc`, `jr`,
  `sr`, `no`.

Bracketed cues are already excluded from `words`, so a gap may contain a
cue. Colons do not end sentences, so `3:16` stays inside its sentence.

UI: `Ui::action` gains `"retake"` and `"back_sentence"` arms.

### 6. Hold mode and countdown length

**Settings** (`core::Settings`, already `#[serde(default)]`):

- `hold_at_end: bool`, default `false`.
- `countdown_secs: f64`, default `5.0`, clamped to `1.0..=15.0` in `clamp()`.

**Session** gains `pub countdown_ms: u64` and `pub hold_at_end: bool`, and
`pub fn configure(&mut self, settings: &Settings)`. The UI calls `configure`
when a session is created and whenever settings are saved. Every hard-coded
`5000` in `Session` becomes `self.countdown_ms`. A running countdown is not
shortened or lengthened mid-way; the new length applies to the next one.

**New phase** `Phase::Holding`:

- Entered by `speech()` at the last word when `hold_at_end` is true and a
  later playable slide exists. The final slide still goes to `Finished`.
- `running()` stays false for `Holding`, so `apply_pending()` applies queued
  PC edits there, and `apply_revision` accepts them.
- `pause()` from `Holding` → `Paused` with `resume = Holding`.
- `start()` / toggle from `Holding` calls `next()`, which moves to `Reading`
  on the next slide.
- `retake()` from `Holding` → `Reading` at the slide start.
- `tick()` ignores `Holding`.

**UI:**

- The settings overlay gains a "Hold at end of slide" `SettingSwitch` and a
  "Countdown" `SettingSlider` (1–15 s), shown only when Hold is off.
- In `Holding`, the prompter shows an "End of slide" card with **Next slide**
  and **Retake** buttons. The main control reads **Next**.
- Help text that promises a "five-second countdown" describes the configured
  behaviour instead.

### 7. Prompter controls

The footer becomes **↺ Retake · ↶ Sentence · ‹ Back · Start/Pause · Next ›**,
each with an `aria_label`. "Restart whole script" moves to the slide-list
overlay as "From the top", calling the existing `restart` action. "Again"
after `Finished` is unchanged.

## Error handling

- Mic retries are capped at 3, then the existing visible error appears.
- `reset` arriving while no `SpeechService` exists is ignored. The next
  `start` carries the take number.
- An unknown or missing `take` on a speech event is accepted (backward
  compatible), as described in §3.
- Invalid saved settings are clamped. Older settings JSON loads with the new
  defaults.

## Testing

**Core** (`crates/core/tests`):

- Growing partials move the highlight without a final result. A rewritten
  newest word does not overshoot. The common-words-only extension does not
  commit.
- `retake` in every phase. `back_sentence`: mid-sentence, first two words,
  first sentence, blank lines, abbreviations, `3:16`, cues inside gaps.
- `take` increments on every rewind and move.
- Holding: entry, final slide still finishes, pause/resume, toggle → next,
  retake, `apply_revision` while holding, `tick` ignored.
- Countdown lengths 1 s and 15 s. Clamping. Old settings JSON deserializes.
- All existing tests, including the 64-event Vosk replay, pass unchanged.

**UI** (manual browser check with the existing unshipped test adapter):

- Footer controls, the Hold card, both settings, and `take`-mismatched
  speech events being dropped.
- A simulated 2 s stall does not pause the take.

**Device** (added to `VALIDATION.md` as required, not claimed):

- Timeout recovery, reset at slide change without losing the first words,
  and the mic staying open through countdown and hold on the A7 Lite.

## Files touched

- `crates/core/src/lib.rs`: Tracker commit rule, Session
  `take`/`retake`/`back_sentence`/`configure`/`Holding`, `sentence_starts`,
  Settings fields.
- `crates/core/tests/playback.rs`, `crates/core/tests/jumps.rs` (or a new
  `retakes.rs`).
- `crates/ui/src/app.rs`: `pump()` stall/mic/reset logic, actions, footer,
  Hold card, settings controls, help text.
- `android/app/src/main/java/com/rustyprompter/app/MainActivity.kt`:
  retry/backoff, `reset` command, `take` tagging.
- `VALIDATION.md`, `README.md` (feature list).
