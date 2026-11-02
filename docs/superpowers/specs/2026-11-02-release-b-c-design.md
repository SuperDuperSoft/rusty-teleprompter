# Releases B and C — Hands-free, Feedback and Smarter Tracking

Date: 2026-11-02
Status: Draft for implementation, revision 2 (written from the Release A
roadmap; revised after written-spec review).

## Context

Release A ("Takes don't break") shipped: self-healing mic, take tagging,
faster highlight, Retake / Back a sentence, Hold mode and configurable
countdown. The roadmap in
`docs/superpowers/specs/2026-11-01-release-a-reliable-takes-design.md` left
two releases:

- **B — Hands-free and feedback**: Bluetooth clicker/pedal, number and verse
  normalization, "I hear you" dot + lost-place hint, 3-2-1 pre-roll, run-time
  estimate.
- **C — Smarter tracking**: guarded end-of-slide, voice backward resync,
  script-vocabulary Vosk grammar.

This spec covers both. The older PromptLocal (Kotlin) design
`../docs/superpowers/specs/2026-11-01-recording-session-controls-design.md`
is borrowed from for remote key mapping, hold rules, pre-roll and sync
flash, adapted to RustyPrompter's rule that Rust owns every decision.

Current architecture facts this design relies on:

- `crates/core/src/lib.rs` (crate `rusty_core`) is pure: no clock, IO or UI.
  Time enters only through `Session::tick(elapsed)`.
- `words(text)` produces exactly one `Word { text, start, end }` per regex
  match; the UI renders highlighting from those byte spans
  (`crates/ui/src/app.rs`, Prompter, the `parts` loop uses
  `raw[parts[i-1].end..word.start]` for gaps and `raw[word.start..word.end]`
  for words).
- `Tracker::accept` is forward-only (`end <= pos` candidates skipped). Its
  `matches` count includes matches against `context` (older finals), and the
  far path returns early after updating `far_end`/`far_votes`.
- `Session::new` builds its tracker before `Self` exists.
- The UI pump (`app.rs` `pump()`) drains `window.__rustyEvents` every 50 ms,
  takes `state.write()` only for drained events and while in `Countdown`
  (to tick), and drives the mic from `Phase::listening()`: a change of the
  desired state sends `native_call("start", take)` / `native_call("stop")`;
  a take change while the mic stays open sends `native_call("reset", take)`.
- Every `state.write()` re-renders the Prompter, which builds one span per
  word (a long slide is 1000+ spans) and clones `Ui` pieces. Writes must stay
  rare.
- The status `phase` sent to native/companion is `format!("{:?}", phase)`
  (`"Reading"`, `"Countdown"`, …); the companion stores it as-is.
- Kotlin (`MainActivity.kt`) has no key handling, builds
  `Recognizer(model, 16000f)` on the main thread without a grammar, handles
  `"reset"` by `service?.reset()` plus re-tagging (no rebuild), handles
  `"stop"` by tearing down `SpeechService` and `Recognizer`, and emits a
  `pause` event from `onPause` (including when a permission dialog shows).
- Neither `interop.js` nor `app.rs` has any keyboard handling.
- The voice-not-ready guard in `Ui::action` blocks `"toggle"` whenever the
  phase is not `running()` (Ready, Paused, Holding, Finished).

## Goals

1. Every reading action can be driven from a two-button Bluetooth clicker or
   pedal (HID keyboard class: PageUp/PageDown, arrows, Space, Enter, B,
   period) in the Android app and in a PC browser. Media keys work on
   Android; in desktop browsers they are best-effort only (the OS or Media
   Session usually takes them).
2. Numbers, Bible references, money, percentages, years and ordinals in the
   script are followed when spoken naturally.
3. The reader can see that the app hears them, and is told (gently) when the
   app has lost their place.
4. Recording sessions can start with a configurable 3-2-1 pre-roll ending in
   a camera sync flash, without losing the take's first words.
5. Studio, the tablet home list and the prompter show estimated read time,
   elapsed time and time left, from one core implementation.
6. A slide only completes when its final words were actually heard.
7. Re-reading earlier text moves the highlight back, without ever harming
   forward tracking.
8. Optionally, Vosk can be restricted to the script's vocabulary.

## Constraints

- Offline, no subscriptions, private LAN only. No new crates, Gradle
  dependencies or network calls.
- **Rust owns all decisions** (matching, playback, timing, key mapping, hold
  detection, debounce). Kotlin may restart, reset, tag and forward raw events
  (including raw key names), and apply plumbing flags Rust sends it. It never
  maps a key to an action.
- All new `Settings` fields live under the existing `#[serde(default)]`, get
  a `Default` value and a `clamp()` rule. Old settings JSON must load.
- Defaults preserve today's behaviour except where a change is clearly a
  correctness fix or a non-intrusive aid (see "Defaults" below).
- All existing core tests (85 `#[test]`s at time of writing), all 18
  companion `#[tokio::test]`s and the 64-event Vosk replay
  (`playback.rs` `real_offline_recognition_stream`, fixture
  `fixtures/offline-voice.tsv`) must keep passing. Clippy with
  `-D warnings` must pass.
- **No dead code between tasks.** Every private item (field, fn, const) a
  task introduces must have a reader in that same task, or clippy fails.
  Where this spec lists a private item under a later section, it arrives in
  the task that first reads it (called out per task).
- **Regex rules.** The `regex` crate has no lookaround and its `\d` is
  Unicode. All numeric patterns use `[0-9]`, never `\d`. Boundary checks
  ("not preceded/followed by a letter or digit") are done by hand on the
  surrounding `char`s. Every number parse (`u64::from_str`) that fails falls
  back to digit-by-digit words; nothing in `words()` may `unwrap` a parse.
  Static regexes are only those written in this spec, compiled once in a
  `OnceLock`.
- Commands run from `rustyprompter/` through `python scripts\task.py …`:
  - Core tests: `python scripts\task.py cargo test -p rusty-core`
  - Lint: `python scripts\task.py cargo clippy -p rusty-core -p rusty-companion --all-targets -- -D warnings`
  - UI build: `python scripts\bundle_ui.py`
  - QA page: `python scripts\task.py cargo build --release -p rusty-companion`,
    then `python scripts\qa.py`, then open
    `http://127.0.0.1:8877/assets/qa-tablet.html`.
- The workspace is not a git repository; skip commit steps unless one is
  initialised.

### Defaults (decided)

| Setting | Default | Why |
|---|---|---|
| `pre_roll_secs` | `0.0` (off) | Start must stay instant for existing users and for live (non-recorded) use. Recording users turn it on once. |
| `sync_flash` | `true` | Only has an effect when pre-roll is on; a flash is what pre-roll users want for camera sync. |
| `speaking_wpm` | `150.0` | Typical presentation pace; replaces the hard-coded 140 wpm (a 7% change to a display-only estimate). |
| `show_lost_hint` | `true` | A small dismissible pill; no effect on tracking. |
| `lost_place_secs` | `5.0` | Long enough to ignore short ad-libs, short enough to help. |
| `voice_resync` | `true` | Evidence rules are stricter than far-forward jumps and it only runs when no forward candidate exists; a switch allows opting out. |
| `volume_keys` | `false` | Intercepting volume keys surprises people; opt-in (PromptLocal rule). |
| `script_grammar` | `false` | Grammar mode forces every utterance into script words, so ad-libs become forced misrecognitions that can look like script matches and undermine the jump safeguards; it also needs a recognizer rebuild and is unverified on the device. Beta, opt-in. |

Number normalization and the guarded end-of-slide are always on (no
setting): they fix matching errors and have no user-visible downside.

Intended behaviour changes (beyond the new features):

- The voice-not-ready guard keeps today's scope (every non-running phase,
  including Holding) and extends it to the new `"forward"` and `"play"`
  actions. `Rolling` is running, so the guard never blocks "Cancel".
- The mic stays open during the pre-roll (see §5); speech during the roll is
  ignored by the core and flushed with a `reset` when the roll ends.
- Word indices change for slides with numbers (see §2 "Index effects"). A
  position saved by Release A is not restored after the upgrade (one-time:
  the reader starts the slide from the top).

## Non-goals

- A beep at roll end, take/marker log export, learned (measured) speaking
  pace, per-script settings, key-learning UI, user-remappable keys.
- AVRCP media buttons from Bluetooth headphones (they go to a
  `MediaSession`, not the activity). Only HID keyboard-class clickers and
  pedals are in scope. Desktop-browser media keys are not promised.
- Keyboard shortcuts in Studio (keys act only while the prompter is shown
  and no sheet or modal is open).
- Alternate spoken forms for the same number beyond the rules in §2 (for
  example "two thousand twenty four" for a `2024` that the year rule reads
  as "twenty twenty four", or "five thirty p m" for `5:30pm`): the reader
  can type the words if they say it that way. Listed under Device checks.
- Grammar support for large Vosk models; changes to the companion protocol
  (the status `phase` string simply gains the value `"Rolling"`, from
  `format!("{:?}")`).

## Design

### 1. Settings and Session configuration (core)

`Settings` (`crates/core/src/lib.rs`) gains, in this order after
`countdown_secs`:

```rust
pub pre_roll_secs: f64,    // default 0.0, clamp 0.0..=10.0
pub sync_flash: bool,      // default true
pub speaking_wpm: f64,     // default 150.0, clamp 80.0..=240.0
pub show_lost_hint: bool,  // default true
pub lost_place_secs: f64,  // default 5.0, clamp 2.0..=15.0
pub voice_resync: bool,    // default true
pub volume_keys: bool,     // default false
pub script_grammar: bool,  // default false
```

`Session` gains public fields (initialised in `Session::new` to the values
`Settings::default()` would configure):

```rust
pub pre_roll_ms: u64,              // 0
pub roll_ms: u64,                  // 0; remaining pre-roll while Rolling
pub sync_mark: u64,                // 0; +1 each time a pre-roll ends
pub wpm: f64,                      // 150.0
pub elapsed_ms: u64,               // 0; take time, see §6
pub heard_ago_ms: Option<u64>,     // None; ms since speech last changed, see §4
pub stuck_ms: u64,                 // 0; speaking time without progress, see §4
pub lost_after_ms: u64,            // 5000; 0 = hint disabled
pub voice_resync: bool,            // true
```

The only new private `Session` field is `last_heard: String`, which arrives
with its reader in task B4 (§4). The pre-roll debounce (§5) needs no extra
field: it uses `pre_roll_ms - roll_ms`.

`configure(&Settings)` additionally sets (same style as the existing
`countdown_ms` line):

- `self.pre_roll_ms = (settings.pre_roll_secs * 1000.0).round() as u64`
- `self.wpm = settings.speaking_wpm`
- `self.lost_after_ms = if settings.show_lost_hint { (settings.lost_place_secs * 1000.0).round() as u64 } else { 0 }`
- `self.voice_resync = settings.voice_resync` and
  `self.tracker.allow_back = settings.voice_resync` (the tracker field
  arrives in §8 / task C2; B1 sets only `voice_resync`).

As today, `configure` never alters a running `remaining_ms` or `roll_ms`.

Every place that builds a new tracker (`new`, `move_to`, `apply_revision`,
`restore`) goes through a private **associated** function
`fn new_tracker(text: &str, allow_back: bool) -> Tracker` (no `&self`, so
`Session::new` can call it before `Self` exists; `new` passes `true`, the
others pass `self.voice_resync`). B1 introduces it as
`fn new_tracker(text: &str) -> Tracker` (just `Tracker::new`); C2 adds the
`allow_back` parameter, sets `tracker.allow_back` from it and updates the
four callers.

### 2. Number and verse normalization (core)

New module `crates/core/src/numbers.rs` (`mod numbers;` in `lib.rs`), pure.

```rust
pub fn is_number_word(w: &str) -> bool;              // re-exported from lib.rs
pub(crate) fn cardinal(n: u64) -> Vec<&'static str>; // 0..=999_999_999_999
pub(crate) fn ordinal(n: u64) -> Vec<String>;
pub(crate) fn year(n: u64) -> Option<Vec<&'static str>>;
```

**Number words.** `zero`..`nineteen`, `twenty`..`ninety`, `hundred`,
`thousand`, `million`, `billion`, and their ordinals (`first`, `second`,
`third`, `fourth`, `fifth`, … `twelfth`, `thirteenth`, … `twentieth`, …
`ninetieth`, `hundredth`, `thousandth`, `millionth`, `billionth`).
`oh` is **not** a number word (it is an ordinary word that the time/verse
and year forms may emit).

**Cardinal.** No hyphens, no "and": `21` → `twenty one`, `123` → `one
hundred twenty three`, `1,250,000` → `one million two hundred fifty
thousand`.

**Ordinal.** Cardinal with the last word converted: `one→first`,
`two→second`, `three→third`, `five→fifth`, `eight→eighth`, `nine→ninth`,
`twelve→twelfth`, `-ty→-tieth`, otherwise `+th`. `2nd` → `second`, `21st` →
`twenty first`, `100th` → `one hundredth`. The suffix (`st|nd|rd|th`, any
case) is accepted whether or not it is grammatically right.

**Year.** `year(n)` returns `Some` only for `1100..=1999` or `2010..=2099`:
`hi = n/100`, `lo = n%100`; `lo == 0` → `cardinal(hi) + hundred`; `lo < 10`
→ `cardinal(hi) + oh + cardinal(lo)`; else `cardinal(hi) + cardinal(lo)`. So
`1990` → `nineteen ninety`, `1905` → `nineteen oh five`, `1900` → `nineteen
hundred`, `2024` → `twenty twenty four`.

The year form is used **only** for a 4-digit integer without commas that
`year()` accepts **and** whose previous emitted word (same slide, ignoring
cues) is one of: `in`, `since`, `by`, `of`, `from`, `until`, `till`,
`before`, `after`, `during`, `year`, `circa`, `around`, `summer`, `spring`,
`autumn`, `fall`, `winter`, or a month name (`january`..`december`, `jan`,
`feb`, `mar`, `apr`, `jun`, `jul`, `aug`, `sep`, `sept`, `oct`, `nov`,
`dec`). Otherwise cardinal: `1500 people` → `one thousand five hundred
people`, `2024 members` → `two thousand twenty four members`, `in 2024` →
`in twenty twenty four`, `1066` → cardinal everywhere.

**Scanner.** `words()` becomes a position scanner (it can no longer be a
single `find_iter`):

1. Find the cue ranges as today (`\[[\s\S]*?\]`).
2. From `pos`, find the next token start with `START = \$[0-9]|[\p{L}\p{N}]`.
   If it lies inside a cue, set `pos` to the cue's end and repeat.
3. At start `s`, if the char at `s` is `$` or `0-9` **and** the char before
   `s` (if any) is not alphanumeric (`char::is_alphanumeric`), try the
   numeric forms below in table order, each as an anchored regex (`^…`) on
   `&text[s..]`. A form *applies* only if the char right after its match is
   not alphanumeric; as a special case, a match followed by `'s` or `’s`
   whose next char is not alphanumeric applies and the `'s` is included in
   the span (`1990's` → `nineteen ninety`, span covers `1990's`). The first
   form that applies emits its words; `pos` = end of its span.
4. Otherwise (letter start, or no form applies): if the char at `s` is `$`,
   skip it (`pos = s + 1`) and continue at step 2. Else match the existing
   plain word regex `[\p{L}\p{N}]+(?:['’][\p{L}]+)?` anchored at `s`, emit
   one `Word` exactly as today (lower-case, `’`→`'`, the `0`..`20` table),
   `pos` = its end.

Retrying happens only at token starts found by step 2, never inside a run
a plain word already consumed. Consequences (tested): `3D` → `3d`, `mp3` →
`mp3`, `covid19` → `covid19`, `1990s` → `1990s`, `80s` → `80s`, `5:30pm` →
`five` `30pm` (the time form fails on `p`, then integer `5` applies before
`:`, then `30pm` is a plain word). Non-ASCII digits (`٣`, `３`) never start a
numeric form and stay plain words as today.

**Token forms** (all digits are `[0-9]`):

| Form | Pattern (anchored) | Expansion |
|---|---|---|
| verse / time | `[0-9]{1,3}:[0-9]{1,3}(?:[-–][0-9]{1,3}(?::[0-9]{1,3})?)?` | Each number part is its own unit with its own sub-span: `3:16` → `three`(span "3") `sixteen`(span "16"); `8:28-30` → `eight` `twenty eight` `thirty`; `8:28–9:2` → `eight` `twenty eight` `nine` `two`. A part that is `00` emits nothing (`10:00` → `ten`); a 2-digit part with a leading zero → `oh` + digit (`3:05` → `three oh five`). |
| money | `\$[0-9]{1,3}(?:,[0-9]{3})+(?:\.[0-9]{2})?` or `\$[0-9]+(?:\.[0-9]{2})?` | Dollars `D`, cents `C`. `D > 0` or `C == 0`: `cardinal(D)` + `dollar` (D == 1) / `dollars`. `C > 0`: add `cardinal(C)` + `cent` (C == 1) / `cents`. `$5` → `five dollars`, `$1` → `one dollar`, `$2.50` → `two dollars fifty cents`, `$0.50` → `fifty cents`, `$0` → `zero dollars`. The span starts at `$`. |
| percent | `[0-9]+(?:\.[0-9]+)?%` | number (decimal rule if needed) + `percent`. |
| grouped | `[0-9]{1,3}(?:,[0-9]{3})+` | cardinal (`1,000` → `one thousand`). |
| decimal | `[0-9]+\.[0-9]+` | `cardinal(int)` + `point` + each fraction digit (`3.14` → `three point one four`). |
| ordinal | `[0-9]+(?i:st|nd|rd|th)` | ordinal. |
| integer | `[0-9]+` | leading `0` and length > 1 → each digit (`007` → `zero zero seven`); year rule if eligible (see above); ≤ 12 digits → cardinal; longer → each digit. |

Any integer part of any form longer than 12 digits, or one whose parse
fails, is spoken digit by digit.

**Spans.** A unit that expands to N words emits N `Word`s. Words 1..N-1 get
a zero-width span `start..start` at the unit's start; the last word gets
the full `start..end`. Invariants for all `i`: `w[i].start <= w[i].end` and
`w[i-1].end <= w[i].start`. This keeps every existing slice in the UI and in
`sentence_starts` valid (gaps between parts are empty) and highlights a
number once its last spoken word is reached. New helpers:

```rust
/// First word index of the unit containing word `i` (walks back over
/// zero-width parts that share the unit's start).
pub fn unit_start(words: &[Word], i: usize) -> usize;
/// Last word index of the unit containing word `i` (walks forward while
/// the current word is zero-width and the next one starts at the same byte).
pub fn unit_end(words: &[Word], i: usize) -> usize;
```

**`sentence_starts`.** Its capital check accepts `$` as well as an
uppercase letter or ASCII digit (money units start at `$`), so
`Done. $5 is enough.` keeps the sentence start. Otherwise unchanged:
zero-width parts sit inside the unit, and a unit at a sentence start makes
its first part the start.

**`similar()`.** Fuzzy (edit distance 1) matching is skipped when either
side `is_number_word` (exact equality only), so `eighty`≁`eighth`,
`sixty`≁`sixth`, `fifty`≁`fifth`, `forty`≁`forth`.

**Accepted risk: number words are content words.** Number words are not in
`is_common`, so short frequent ones (`one`, `two`, `four` — also heard for
"for" — `first`) carry content weight in the jump, end and back-resync
thresholds. Normalisation makes them more frequent in scripts. This is
accepted for now (changing `is_common` would move existing gains); revisit
if device tests show false jumps on number-heavy scripts.

**Hypothesis cleanup** (`Tracker`, task B2b). `Tracker::new(text)` builds a
private `Cleanup` from **both** the slide text and its words:

- `has_range`: the slide text (outside cues) contains a match of
  `[0-9]{1,3}:[0-9]{1,3}[-–][0-9]` (a verse range).
- `has_filler[w]` for `chapter`, `verse`, `verses`: the slide's words contain
  `w`.
- the set of (prev, `and`, number word) shapes and (`a`, scale word) pairs
  present in the slide's words.
- the set of (number word, `to|through|thru`, number word) shapes present.

`accept` builds `heard = context + incoming` as today and then runs the
cleanup over the **whole `heard` sequence** (so a filler split across two
finals is handled), adjusting `fresh_from` down by the number of tokens
removed before it. Rules, in order:

1. `a` directly before `hundred|thousand|million|billion` becomes `one`,
   unless the slide contains `a` followed by that scale word.
2. `and` is dropped when the previous token is
   `hundred|thousand|million|billion|dollar|dollars` and the next token is a
   number word **or there is no next token**, unless the slide contains that
   same (prev, `and`, number word) shape.
3. `chapter`, `verse`, `verses` are dropped when the next token is a number
   word **or there is no next token**, and the slide never contains that
   filler word.
4. `to`, `through`, `thru` are dropped when both neighbours are number
   words, `has_range`, and the slide never contains (number word, filler,
   number word).

"No next token" means the end of `heard`, so a partial `john chapter three
verse` drops the trailing `verse` and still advances. So `read john chapter
three verse sixteen` matches `Read John 3:16`, `psalm twenty three` matches
`Psalm 23`, `one hundred and fifty` matches `150`, and `romans eight twenty
eight through thirty` matches `Romans 8:28-30`. `context` stores cleaned
tokens. Vosk never emits digits, but the QA adapter and PC tests speak the
raw slide text, which `words()` normalises the same way. If cleanup leaves
no fresh tokens, the call is `Idle` (§3).

**Index effects and saved positions.** Word indices shift wherever a number
expands to more than one word (`23` was one word, now `twenty three`).
Within one app version, `Position.next_word`, `restore`, `apply_revision`
anchors and `back_sentence` all use `words()` on both sides and stay
consistent. Across the upgrade they would not, so:

- `pub const WORDS_SCHEME: u32 = 1;` in `lib.rs`.
- `Position` gains `#[serde(default)] pub words_scheme: u32` (old JSON → 0).
  `position()` writes `WORDS_SCHEME`.
- `restore` returns without restoring when `saved.words_scheme !=
  WORDS_SCHEME` (same as a revision mismatch).

Studio word counts and estimates also change (expanded numbers count as
spoken words); that is intended. The existing `retakes.rs` expectations
(`john4 three5 sixteen6`) are unchanged because `3` and `16` are single
words.

UI: clicking a word seeks to `unit_start(words, i)` (§11).

### 3. Tracker outcome (core)

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome { #[default] Idle, Advanced, Back, Pending, NoMatch }
```

`Tracker` gains `pub last_outcome: Outcome`, set by every `accept` call:

- `Idle`: no fresh tokens after cleanup, or the slide has no words.
- `Advanced`: `position` increased during this call.
- `Back`: a voice backward resync moved `position` back (§8).
- `Pending`: a candidate existed but was not committed (vote pending, far
  vote pending, back vote pending, or rejected by the end-of-slide guard).
- `NoMatch`: no forward candidate and no backward candidate.

The return type of `accept` is unchanged.

### 4. "I hear you" dot and lost-place hint (core decides, UI shows)

New private field (task B4): `last_heard: String`.

`Session::speech(text, is_final)`:

- If `self.phase.listening()` and `text.trim()` is non-empty and
  (`is_final` or `text != self.last_heard`): set
  `self.heard_ago_ms = Some(0)` and `self.last_heard = text.to_owned()`.
  (Vosk repeats an unchanged partial until endpointing; repeats are not
  "new" speech.) Task C3 refines "non-empty" to "non-empty after removing
  `[unk]`" (§10).
- Then the existing `Reading` gate. After `tracker.accept`, if
  `tracker.last_outcome` is `Advanced` or `Back`, set `stuck_ms = 0`.

`speech_for_take` still drops stale takes before any of this, so stale-take
speech never lights the dot.

Constants and tick accounting (exact even when one `tick` covers a long
interval, so the UI may batch ticks — see §11):

```rust
pub const HEARD_MS: u64 = 700;
pub const SPEAKING_MS: u64 = 1500;
```

`Session::tick(elapsed)` additionally (see §5 for the full tick), in this
order:

1. In `Reading`, if `self.lost_after_ms > 0` and
   `self.heard_ago_ms.is_some_and(|m| m < SPEAKING_MS)`:
   `self.stuck_ms += elapsed.min(SPEAKING_MS - heard_ago)` where `heard_ago`
   is the value before step 2 (only the still-speaking part of the interval
   counts).
2. In any `listening()` phase,
   `self.heard_ago_ms = self.heard_ago_ms.map(|m| m.saturating_add(elapsed))`.

`None` means "never heard since the last reset" and never counts as
hearing or speaking. **Never compare `Option`s with `<`** (`None < Some(_)`
is true in Rust); always use `is_some_and`.

New methods:

```rust
/// True while new speech arrived within the last HEARD_MS.
pub fn hearing(&self) -> bool {
    self.phase.listening() && self.heard_ago_ms.is_some_and(|m| m < HEARD_MS)
}
/// True when the reader has been speaking for lost_after_ms without the
/// position moving.
pub fn lost(&self) -> bool; // phase == Reading && lost_after_ms > 0 && stuck_ms >= lost_after_ms
/// Hides the hint until the reader speaks for lost_after_ms again.
pub fn dismiss_lost(&mut self); // stuck_ms = 0
/// Word-index range of the next `n` expected words, extended to the end of
/// the unit containing its last word. None at the end of the slide.
pub fn upcoming(&self, n: usize) -> Option<std::ops::Range<usize>>;
/// True when tick(elapsed) would change anything the prompter shows
/// (see §11). Pure; does not tick.
pub fn tick_changes_view(&self, elapsed: u64) -> bool;
```

`upcoming(n)`: `next = position + 1` (0 if none); `None` when
`next >= len`; else `next..unit_end(words, min(next + n, len) - 1) + 1`.
The UI builds the hint text from these word indices (never from a byte
range of the raw text, which could include cues and line breaks): for each
index whose word is not zero-width, take `raw[w.start..w.end]` (the last
part of a unit spans the whole unit), joined with single spaces.

`tick_changes_view(elapsed)` returns `false` when `elapsed == 0` or the
phase is not `listening()` and not `Rolling`; `true` in `Countdown` and
`Rolling`; in `Reading`/`Holding` it computes the post-tick values with the
same arithmetic as `tick` (no clone) and returns true if any of these
differ from now: `hearing()`, `lost()`, `elapsed_ms / 1000`.

`stuck_ms = 0`, `heard_ago_ms = None` and `last_heard` cleared are also set
by `start`, `pause`, `move_to`, `rewind_to`, `apply_revision` and `restore`.

### 5. 3-2-1 pre-roll (core)

New `Phase::Rolling` (serde `"rolling"`; the status string the UI sends is
`format!("{:?}")` = `"Rolling"`).

- `running()` = `Reading | Countdown | Rolling` (so `apply_revision` refuses
  and pending PC edits wait, as a roll is part of a take).
- `listening()` = `Reading | Countdown | Holding` (unchanged). It still
  drives the heard dot and `hold.rs` semantics.
- New `mic_open()` = `listening() || Rolling`. **The pump drives the mic from
  `mic_open()`**, so the mic stays warm through the roll (no stop/start, no
  recognizer rebuild, and on first use the permission request happens at
  the start of the roll). Speech arriving in `Rolling` is dropped by the
  existing Reading-only gate in `speech()` (no position change, no votes;
  `heard_ago_ms` is not touched because `Rolling` is not `listening()`).
  When the roll ends the pump sends `native_call("reset", take)` (§11) so
  Vosk discards the audio buffered during "3-2-1".
- `pause()` accepts `listening() || Ready || Rolling`. Pausing in `Rolling`
  stores `resume = Rolling` and sets `roll_ms = 0`.

Private helpers (task B3a):

```rust
fn enter_roll(&mut self);   // only if !tracker.words.is_empty(): phase = Rolling; roll_ms = pre_roll_ms; resume = Reading; tracker.reset_utterance()
fn finish_roll(&mut self);  // phase = Reading; roll_ms = 0; sync_mark += 1; tracker.reset_utterance(); stuck_ms = 0; heard_ago_ms = None
```

`finish_roll` never bumps `take` (the take started when the roll did); a
Back resync never bumps `take` either.

Entry rules (roll only when `pre_roll_ms > 0` and the slide has words;
otherwise behaviour is exactly today's):

- `start()`:
  - `Finished`: no-op (unchanged). `Holding`: `next()` (no roll; slide
    transitions never roll).
  - `Ready`: roll, else `Reading`.
  - `Paused`: if `resume` is `Reading` or `Rolling` → roll (the full
    pre-roll restarts), else return to `resume` (`Countdown`, `Holding`).
    With pre-roll off, a `resume` of `Rolling` returns to `Reading`.
  - `Rolling`: unchanged (stays `Rolling`). Other phases: as today.
- `retake()` and `back_sentence()` from `Reading`, `Countdown`, `Holding`,
  `Finished` or `Rolling` → roll (with a fresh `roll_ms`), bumping `take`
  exactly once (inside `rewind_to`). From `Ready` or `Paused` they stay put
  (as today; `start` will then roll). On a slide with no words, the phase
  is kept (Release A rule) and nothing rolls.
- `seek()` (tapping a word) never rolls: in `Rolling` it keeps rolling with
  the current `roll_ms`; otherwise today's rule.

  Implementation: `rewind_to(next_word, roll: bool)`; `retake` and
  `back_sentence` pass `true`, `seek` passes `false`. `apply_revision` does
  not call `rewind_to` and is unaffected.
- `move_to` (`next`, `previous`, `jump`, `restart`) never starts a roll from
  Ready/Paused/Reading/Countdown/Holding. From `Rolling` it **restarts the
  roll** on the target slide (fresh `roll_ms`, so the take still gets its
  sync flash). In every other case it sets `roll_ms = 0`.

New public methods (task B3b):

```rust
/// Ends the pre-roll now (sync mark included). No-op outside Rolling.
pub fn skip_roll(&mut self);
/// "Again" after Finished: first playable slide, elapsed_ms = 0, then
/// roll (or Reading when pre-roll is off). No-op if nothing is playable.
pub fn read_again(&mut self);
/// The remote's Forward (see table below).
pub fn forward(&mut self);
```

`forward()` by phase:

| Phase | Effect |
|---|---|
| `Ready` | `start()` |
| `Paused` (resume `Holding`) | `next()` then `start()` (reads the next slide; rolls if pre-roll is on, like any resume from pause) |
| `Paused` (other resume) | `start()` |
| `Rolling` | `skip_roll()`, **unless** less than `ROLL_GUARD_MS = 500` of the roll has elapsed (`pre_roll_ms - roll_ms < 500`): then no-op (a double press or key bounce must not kill the pre-roll) |
| `Reading`, `Countdown`, `Holding` | `next()` |
| `Finished` | `read_again()` (so a two-button clicker can start another take without touching the screen; matches PromptLocal, where Forward from Finished replaces "Read again"). The UI's voice-ready guard still applies. |

`pub const ROLL_GUARD_MS: u64 = 500;` lives next to the other Session
constants.

`tick(elapsed)` becomes:

```text
Countdown: remaining_ms -= elapsed; elapsed_ms += elapsed; at 0 → next()   (as today + elapsed)
Rolling:   roll_ms -= elapsed; at 0 → finish_roll()
Reading:   elapsed_ms += elapsed; lost accounting (§4 step 1)
Holding:   elapsed_ms += elapsed
all listening phases: heard_ago_ms accounting (§4 step 2)
others:    nothing
```

`configure` during `Rolling` keeps the current `roll_ms`.

The UI's **sync flash** and the pump's post-roll `reset` react to
`sync_mark` changing (§11).

### 6. Run-time estimate (core)

Top-level pure functions in `lib.rs`:

```rust
pub fn estimate_ms(word_count: usize, wpm: f64) -> u64;   // (count as f64 * 60000.0 / wpm.max(1.0)).round() as u64
pub fn slide_estimate_ms(slide: &Slide, wpm: f64) -> u64; // estimate_ms(words(&slide.text).len(), wpm)
/// All slides plus one transition gap between consecutive playable slides:
/// countdown_secs when !hold_at_end, 0 when holding. Empty slides count 0.
pub fn script_estimate_ms(script: &Script, settings: &Settings) -> u64;
/// "0:42", "12:05", "1:02:03" (seconds rounded up; 0 → "0:00").
pub fn format_duration(ms: u64) -> String;
```

`Session` methods:

```rust
/// Estimated time to finish from here: words after the position on this
/// slide and on every later playable slide at `wpm`, plus a transition gap
/// per later playable slide (countdown_ms unless hold_at_end), with the
/// current remaining_ms replacing the first gap while in Countdown (also
/// when Paused with resume Countdown), and roll_ms added while Rolling.
/// In Holding the first gap is 0. 0 when Finished.
pub fn time_left_ms(&self) -> u64;
```

`elapsed_ms` counts `Reading`, `Countdown` and `Holding` time (§5), is reset
by `read_again` and `restart`, and is not persisted.

Word counts use `words()`, so cues are excluded and expanded numbers count
as spoken words.

### 7. Guarded end-of-slide (core, Release C)

**Vote bookkeeping is unchanged.** `far_end`/`far_votes`,
`partial_end`/`votes` (and §8's back votes) are updated exactly as today
whenever a candidate exists. The guard only suppresses the final
`position = Some(end)` commit (on the mid path and on the far path) and
sets `last_outcome = Pending`. In particular a far partial sighting
followed by a far final sighting gives `far_votes == 2` on the final, which
then commits.

"`pos`" below is the position at the **start** of the `accept` call (before
the "growing partial confirms prev" step can move it).

When the candidate's `end == words.len() - 1`, the commit must also pass:

- **E1 (existing)**: `matches >= min(3, len)` (context matches count, as
  today).
- **E2 heard ending** — at least one of:
  - (a) the winning cell's fresh run contains a content (non-`is_common`)
    match: `fresh_content >= 1`;
  - (b) the slide's last three words are all common and
    `fresh >= min(2, len)`;
  - (c) **contiguous tail**: the fresh run matched every script word from
    `pos + 1` to `end` in order with no gap (`fresh_first == pos + 1` and
    `fresh == end - pos`), and the cell's run (context included) has a
    content match at a script index in `pos - 3 ..= pos` (computed in
    `isize`; `pos = -1` when there is no position).
  The winning candidate must carry `fresh`, `fresh_first`, `fresh_content`
  and the new `Cell.last_content: Option<isize>` (script index of the
  latest non-common match in the run, fresh or context); replace the
  `best` tuple with a small private `Candidate` struct.
- **E3 no jumping onto the end**: if `end - pos > 12`, the commit
  additionally requires `is_final`. This applies to both the mid path and
  the far path (a far jump onto the last word needs its two sightings **and**
  the second one must be final).

Rule (c) is why a reader who pauses before the last common words still
finishes: slide `… Thank you all for coming to this`, final `… for coming`,
then final `to this` (fresh `to this` = the whole remaining tail, context
holds `coming`). Same for one-final-per-word endings (`of`, then `you`).

`Session::speech` is unchanged: completion still happens when `position`
reaches the last word. Existing behaviours stay green:
`separate_finals_keep_context` ("studio" is a fresh content word),
`short_slide_one/two`, `lone_ending_word_cannot_finish`, the 64-event replay
(its final contains "end of the slide").

### 8. Voice backward resync (core, Release C)

`Tracker` gains `pub allow_back: bool` (default `true`) and private
`back_end: Option<usize>`, `back_votes: usize`, both cleared by
`reset_utterance`, **and by any `accept` call that does not produce a back
candidate** (including calls with a forward candidate, `NoMatch` and
`Idle`), mirroring how the mid path clears `far_end`/`far_votes`.

Constants: `BACK_WINDOW = 80`, `BACK_MIN = 4`.

A backward pass runs only when all of these hold: `allow_back`, `position`
is `Some(pos)`, the main pass found no forward candidate, and the incoming
(cleaned, fresh) tokens number at least 5.

`fn back_candidate(&self, incoming: &[String], pos: usize) -> Option<Back>`
(private struct with `end`, `value`, `matches`):

- Aligns **only the incoming tokens** (no `context`, so older speech cannot
  drive it) against script words `pos.saturating_sub(BACK_WINDOW) ..= pos`
  with the same Smith-Waterman gains as the main pass. All index arithmetic
  uses `usize` with `saturating_sub` or `isize`; never `pos - k` on `usize`.
- **Near window**: last-row cells (row = `incoming.len()`) with
  `end + BACK_MIN > pos` (i.e. `end >= pos - 3`). `here` = the best score of
  any near-window cell. If any near-window cell has `matches >= 5`, there is
  no back candidate (the reader is saying a nearby copy of the phrase).
- A **candidate** is a last-row cell with `end + BACK_MIN <= pos` whose run
  has `matches >= 5`, `content >= 3` (non-common matches),
  script-side density `matches * 4 >= span * 3` where
  `span = end - start + 1`, **incoming coverage**
  `matches * 5 >= incoming.len() * 3` (at least 60% of what was said), and
  `start_row <= 3` (the run starts within the first three incoming tokens,
  so an ad-lib that quotes an earlier line — "as I said earlier love is
  patient love is kind" — does not qualify).
- A candidate must score `value >= here + 3.0`.
- Rank: `value - (pos - end) as f64 * 0.045`; highest rank wins, so the
  nearest copy of a repeated passage wins.

Commit rule: commit when `is_final`, or when the same end (±2) was the back
candidate of the **immediately preceding** `accept` call (`back_votes >= 2`;
any call in between without a back candidate cleared it). On commit:
`position = Some(end)`, then `reset_utterance()` (clears `partial_end`,
`votes`, far and back votes, **and `context`**, so tokens from the old,
further-ahead location never feed the next forward pass), and
`last_outcome = Back`. Otherwise `last_outcome = Pending`.

A backward resync is a tracker-level move: **it does not bump `take`** and
does not change phase (it can only happen in `Reading`).

Why it cannot regress forward tracking: it never runs when any forward
candidate exists, it ignores `context`, it requires the reader's newest word
to be behind the position, it requires five matches with three content words
at 75% script density and 60% utterance coverage starting near the
utterance start, a nearby copy blocks it, and it must clearly beat the
alignment at the current position. A revised partial
(`revised_partial_never_reverses`) has fewer than 5 tokens; a final that
re-sends an utterance ending at the position scores best in the near window.

### 9. Bluetooth clicker / pedal (core mapper)

New module `crates/core/src/remote.rs`, re-exported from `lib.rs`. Pure; no
clock (times are passed in).

```rust
pub const HOLD_MS: f64 = 900.0;
pub const STUCK_KEY_MS: f64 = 10_000.0;
pub const REPEAT_GUARD_MS: f64 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAction { Forward, Toggle, Play, Pause, BackSentence, Retake }

/// (short-press action, hold action) for a DOM `KeyboardEvent.key` value.
pub fn remote_binding(key: &str, volume_keys: bool)
    -> Option<(RemoteAction, Option<RemoteAction>)>;

#[derive(Clone, Debug, Default)]
pub struct Remote { pub volume_keys: bool, /* private: held, ignored, last_fire */ }
// struct Held { key: String, down_at: f64, fired: bool }
// ignored: Option<String>   — a key discarded as stuck, ignored until its key-up
// last_fire: Option<(RemoteAction, f64)>

impl Remote {
    pub fn new(volume_keys: bool) -> Self;
    /// Bound keys, for the JS preventDefault allowlist.
    pub fn keys(&self) -> Vec<&'static str>;
    pub fn handles(&self, key: &str) -> bool;
    pub fn key(&mut self, key: &str, down: bool, repeat: bool, now_ms: f64) -> Option<RemoteAction>;
    pub fn tick(&mut self, now_ms: f64) -> Option<RemoteAction>;
    pub fn reset(&mut self);
}
```

Key names are DOM `KeyboardEvent.key` values; single characters compare
case-insensitively; `" "`, `"Spacebar"` and `"Space"` are all Space.

| Key(s) | Short press | Hold (≥ 900 ms) |
|---|---|---|
| `PageDown`, `ArrowRight`, `ArrowDown`, `MediaTrackNext` | Forward | Toggle (pause/resume) |
| `PageUp`, `ArrowLeft`, `ArrowUp`, `MediaTrackPrevious` | BackSentence | Retake |
| Space, `Enter`, `MediaPlayPause` | Toggle | — |
| `MediaPlay` | Play (start/resume only; no-op while running) | — |
| `MediaPause`, `b`, `.` (clicker "blank screen") | Pause | — |
| `AudioVolumeUp` (only if `volume_keys`) | Forward | Toggle |
| `AudioVolumeDown` (only if `volume_keys`) | BackSentence | Retake |

Media keys reach Rust reliably only on Android (forwarded natively, §12).
In desktop browsers they are best-effort: the OS or Media Session usually
consumes them.

Rules (from PromptLocal, adapted):

- **Repeat downs never create a hold record.** A down with `repeat == true`
  only matters for a key already recorded as held (it may fire the hold).
  So a key already held when the prompter opens (first observed event is a
  repeat) does nothing until it is released and pressed again.
- A key **without** a hold action fires its short action on the first
  (non-repeat) key-down; repeat downs and its key-up do nothing.
- A key **with** a hold action records `down_at` on the first key-down. A
  repeat down or `tick` at `now - down_at >= HOLD_MS` fires the hold action
  exactly once. The key-up fires the short action only if the hold did not
  fire.
- A key-up without a matching recorded down does nothing.
- A down of a different bound key discards the previous held key without
  firing, then is handled normally (a no-hold key fires at once).
- `tick` discards a held key older than `STUCK_KEY_MS` (lost key-up) and
  puts it in `ignored`: all events for that key are ignored until its
  key-up (which clears `ignored` and fires nothing). This stops auto-repeat
  from re-arming the hold.
- **Repeat guard (debounce).** A short action equal to `last_fire`'s action
  within `REPEAT_GUARD_MS` of it is dropped, and the drop **extends** the
  guard (`last_fire` time = now). So a double tap, key bounce, or a cheap
  clicker that sends down/up pairs while held fires once and cannot skip
  through slides. Hold actions are not debounced (they already fire once
  per press). Combined with §5's `ROLL_GUARD_MS`, a second Forward right
  after starting cannot cancel the pre-roll.
- `reset()` (window blur) discards the held key and `ignored`.

Rationale for the two-button layout: a two-button clicker gets Next, a
sentence back, a full retake (hold back) and pause (hold next); the
sentence-level retake is RustyPrompter's most frequent correction.

Action dispatch (UI, §12): Forward → `"forward"` (`Session::forward`),
Toggle → `"toggle"`, Play → `"play"`, Pause → `"pause"`, BackSentence →
`"back_sentence"`, Retake → `"retake"`.

### 10. Script-vocabulary grammar (core, Release C)

```rust
pub const GRAMMAR_MAX: usize = 3000;
/// Sorted, de-duplicated tokens of all slides (normalised by words()), plus
/// all number words, plus "chapter", "verse", "verses", "to", "through",
/// "and", "a", "oh", "point", "dollar", "dollars", "cent", "cents",
/// "percent", with "[unk]" appended last. Empty when no slide has words.
pub fn vocabulary(script: &Script) -> Vec<String>;
/// JSON array of vocabulary(script) for Vosk, or None when the script has
/// no playable words or the vocabulary has more than GRAMMAR_MAX entries.
pub fn grammar_json(script: &Script) -> Option<String>;
/// Why grammar_json returned None, for the UI message.
pub fn grammar_skip_reason(script: &Script) -> Option<&'static str>;
// "empty" | "too_large"
```

`[unk]` lets ad-libs map to "unknown" instead of being forced onto script
words. Words missing from the model's lexicon are ignored by Vosk with a log
warning, which is acceptable.

**`[unk]` in hypotheses** (task C3). The CUE regex in `words()` would strip
a literal `[unk]` and make its neighbours adjacent with no ad-lib penalty.
So `Tracker::accept` first replaces every `[unk]` (ASCII case-insensitive)
in the hypothesis with the reserved token `zzunk`, which never matches a
script word (`similar()` and the equality test return false whenever either
side is `zzunk`); it therefore keeps the `-1.4` ad-lib cost. An `[unk]`-only
hypothesis has fresh tokens that can never match, so the call ends as
`NoMatch` (no move, no votes). `Session::speech` treats text that is empty
after removing `[unk]` as not heard (no dot, no `stuck_ms`), and does not
pass it to the tracker (`Idle`).

### 11. UI — pre-roll, flash, heard dot, lost hint, estimates (`app.rs`)

**Pump — render budget.** The pump must not write the `Ui` signal on every
loop. Rules:

- `Remote` lives as a **local variable in `pump`** (not in `Ui`), with its
  `volume_keys` refreshed from `state.peek().settings` each loop.
- The pump keeps a local `clock_ms: u64`. Each loop adds `elapsed.min(250)`
  (a long gap counts as one short step). It then calls
  `session.tick(clock_ms)` under `state.write()` and resets `clock_ms = 0`
  only when one of these is true (checked under `peek`):
  - the loop already takes `state.write()` for drained events (fold the
    tick into that write),
  - `session.tick_changes_view(clock_ms)` (always true in `Countdown` and
    `Rolling`; in `Reading`/`Holding` only when the heard dot, the lost
    hint or the elapsed second would change),
  - `now() >= ui.flash_until` while a flash is shown, or the remote toast's
    deadline has passed (forced write so they disappear).
  In `Ready`, `Paused` and `Finished` nothing is written by the clock
  (`clock_ms` is reset without a write, since `tick` is a no-op there).
  After any tick write, call `apply_pending()`.
- A Remote action (from a key event or `remote.tick(now())`) takes one
  write to dispatch.
- Result: while Reading with no speech the Prompter renders about once a
  second (elapsed time), not 20 times.

**Pump — mic and roll.**

- `desired` (mic) uses `session.phase.mic_open()` instead of `listening()`.
- The pump keeps `seen_mark: Option<u64>` (local). When the session's
  `sync_mark` differs from it: store it; if the mic is open, send
  `native_call("reset", take)` (same take; flushes "3-2-1" audio); and in the
  same write, if `settings.sync_flash`, set `ui.flash_until = now() + 300.0`.
  `seen_mark` is set without flashing when a session is first seen or the
  script changes.
- `"permission_granted"` keeps calling `s.start()`. With the mic opening at
  the start of the roll, the first-use dialog appears during the roll and
  its `onPause` pauses the session (resume `Rolling`); `start()` then plays
  the full roll once more. Accepted and documented in Help/INSTALL.
- Status JSON to native/companion gains nothing new; `phase` may be
  `"Rolling"`.

**Actions** (`Ui::action`):

- `"toggle"` when `Finished` calls `s.read_again()` instead of
  `restart(); start()`.
- New `"forward"` → `s.forward()`, `"play"` → `s.start()` only when
  `!s.phase.running()`, `"skip_roll"` → `s.skip_roll()`,
  `"dismiss_lost"` → `s.dismiss_lost()`.
- The voice-not-ready guard keeps today's condition (`native && !voice_ready
  && !phase.running()`, which includes Holding) and applies it to
  `"toggle"`, `"forward"` and `"play"`.

**Prompter.**

- Status string: `Rolling` → "Get ready".
- Footer play button label: `Rolling` → "Cancel" (it pauses).
- Pre-roll overlay (inside `.prompt-mirror`, same pattern as the countdown
  overlay): label "Starting in", `strong` = `roll_ms.div_ceil(1000)`, note
  "Take {take}", buttons "Start now" (`skip_roll`) and "Cancel" (`pause`).
- Sync flash: `div.sync-flash` rendered **outside `.prompt-mirror`** (which
  always carries a `transform`, so `position: fixed` inside it would be
  relative to the mirror), as a direct child of `main.prompter` or in
  `Overlays`: `position: fixed; inset: 0`, white, `pointer-events: none`,
  z-index above every modal and sheet. Rendered while `now() < flash_until`.
- Heard dot: the existing title dot gets class `"dot heard"` when
  `session.hearing()`, `"dot ok pulse"` when `phase.listening()`, else
  `"dot"`; `aria_label` "Hearing you" / "Listening" / "Not listening".
  CSS: `.dot.heard{background:var(--accent);transform:scale(1.4);transition:transform .12s}`.
- Lost-place hint: when `ui.settings.show_lost_hint && session.lost()`, a
  `div.lost-hint` pill inside `.prompt-mirror` (above the follow-pill):
  "Lost your place? Next: “{hint text}”" where the hint text is built from
  the `upcoming(6)` word indices as described in §4, with buttons
  "Sentence back" (`back_sentence`) and "Dismiss" (`dismiss_lost`). Words
  whose index is in `upcoming(6)` also get class `hint-word` (underline).
  The hint disappears on its own when the position moves.
- Active word: when the current position is a zero-width part, the
  `active-word` class goes on `unit_end(&parts, current)` (the visible part)
  so the highlight never sits on an empty span.
- Word click seeks `unit_start(&parts, i)`.
- Header line appends the times when a session has words:
  `· {format_duration(elapsed_ms)} · ~{format_duration(time_left_ms())} left`.

**Studio and home list.**

- Studio slide footer "About {duration}s" →
  `"~{format_duration(slide_estimate_ms(slide, wpm))}"`; the Studio header
  (or slide-rail footer) shows the script total from `script_estimate_ms`.
- Tablet home row: "about {m} min" uses
  `script_estimate_ms(script, &ui.settings)` rounded up to minutes (min 1).
- Remove both hard-coded `140.0` uses.
- The rail tip that hard-codes "5" / "Five seconds between slides" uses the
  configured `countdown_secs` / hold text (reuse `end_of_slide_text`).

**Settings sheet** (between the countdown slider and "From the top"):

- `SettingSlider` "Pre-roll (s)" field `pre_roll_secs`, 0–10 step 1.
- `SettingSwitch` "Sync flash" field `sync_flash` (only when pre-roll > 0).
- `SettingSlider` "Speaking pace (wpm)" field `speaking_wpm`, 80–240 step 5.
- `SettingSwitch` "Lost-place hint" field `show_lost_hint`; when on,
  `SettingSlider` "Hint after (s)" field `lost_place_secs`, 2–15 step 1.
- `SettingSwitch` "Follow re-reading" field `voice_resync`.
- Native only: `SettingSwitch` "Volume keys as remote" field `volume_keys`
  and `SettingSwitch` "Script vocabulary (beta)" field `script_grammar`.

Each needs a match arm in `SettingSlider`/`SettingSwitch`.

### 12. UI and host — remote keys

**interop.js.** New export `set_remote(prompting, keys_json)` storing
`window.__rustyRemote = {prompting, keys: new Set(JSON.parse(keys_json))}`.
`initialize()` adds:

- `keydown` and `keyup` listeners on `document` (capture phase). Ignore the
  event (no `preventDefault`, nothing pushed) when any of:
  `!__rustyRemote?.prompting`; `e.ctrlKey || e.altKey || e.metaKey`;
  `e.isComposing`; the target is `input`, `textarea`, `select` or
  contenteditable; the target is inside `[role=dialog]`, `.sheet` or
  `.sheet-backdrop`; or `e.key` (lower-cased for single characters, `" "`
  for Space) is not in `keys`. Otherwise `e.preventDefault()` on **both**
  down and up (Space activates a focused button on key-up) and push
  `{type:"key", key:e.key, down, repeat:e.repeat}`.
- `blur` on `window` pushes `{type:"key_reset"}`.

Declare `set_remote` in the `app.rs` extern block.

**app.rs.** `Ui` gains `remote_toast: Option<(String, f64)>` (text,
deadline). The `Remote` itself is a pump local (§11).

- Pump: `"key"` events (only while `ui.session` is `Some`) call
  `remote.key(key, down, repeat, now())`; `"key_reset"` calls
  `remote.reset()`. Every loop calls `remote.tick(now())`. A returned
  action takes a write and calls `ui.remote_action(action)`, which
  dispatches per §9 and sets a 1.5 s toast ("Next ›", "Sentence back",
  "Retake · take {n}", "Paused", "Start", …).
- Event times are ignored; `now()` at drain time is used for every key event
  (DOM and native clocks differ; 50 ms jitter is irrelevant to a 900 ms
  hold).
- `prompting` = the prompter is shown **and** no sheet or modal (settings
  sheet, Help, connect sheet) is open. Pump keeps
  `remote_key: Option<(bool, bool)>` = (`prompting`, `volume_keys`); when it
  changes, call `set_remote(prompting, json(remote.keys()))` and
  `native_call("remote", {"prompting":bool,"volume":bool})`, and call
  `remote.reset()` when `prompting` turns false.
- The toast renders inside `.prompt-mirror` as `div.remote-toast` (large,
  centred near the top) so it is readable from the speaker's mark.

**Kotlin** (`MainActivity.kt`), plumbing only — raw forwarding, no mapping
decision:

- Fields `prompting=false`, `volumeKeys=false`. Command `"remote"` parses
  `{"prompting","volume"}`; when it sets `prompting=true`, call
  `web.requestFocus()`.
- Override `dispatchKeyEvent(event)`. Raw name table:

  | Keycode | DOM key name |
  |---|---|
  | `MEDIA_PLAY_PAUSE`, `HEADSETHOOK` | `MediaPlayPause` |
  | `MEDIA_PLAY` | `MediaPlay` |
  | `MEDIA_PAUSE` | `MediaPause` |
  | `MEDIA_NEXT` | `MediaTrackNext` |
  | `MEDIA_PREVIOUS` | `MediaTrackPrevious` |
  | `PAGE_UP` / `PAGE_DOWN` | `PageUp` / `PageDown` |
  | `DPAD_UP` / `DPAD_DOWN` / `DPAD_LEFT` / `DPAD_RIGHT` | `ArrowUp` / `ArrowDown` / `ArrowLeft` / `ArrowRight` |
  | `SPACE` | `" "` |
  | `ENTER`, `DPAD_CENTER`, `NUMPAD_ENTER` | `Enter` |
  | `B` | `b` |
  | `PERIOD` | `.` |
  | `VOLUME_UP` / `VOLUME_DOWN` | `AudioVolumeUp` / `AudioVolumeDown` |

  If `prompting` and the keycode is in the table (volume keys only when
  `volumeKeys`), and the event has no Ctrl/Alt/Meta modifier, emit
  `{type:"key", key, down: action==ACTION_DOWN, repeat: repeatCount>0}` and
  return `true` (consumed: the WebView never sees it, so focus loss, DPAD
  spatial navigation and focused buttons cannot drop or steal it).
  Everything else → `super.dispatchKeyEvent(event)`.

On Android the JS listener therefore never sees these keys while
prompting; it matters only for PC browsers.

### 13. UI and host — grammar plumbing

- Pump: `grammar_key: Option<Option<(String, u64)>>` (last sent). Desired key
  = `Some((script.id, revision))` when `native && settings.script_grammar &&
  session.is_some()` and `grammar_json(script)` is `Some`, else `None`. On
  change, call `native_call("grammar", grammar_json(script).unwrap_or_default())`
  (empty string = unrestricted). When the switch is on but
  `grammar_skip_reason` is `Some`, set `ui.message` once per key change
  ("Script vocabulary is too large; using the full vocabulary." /
  nothing for an empty script). This runs before the mic logic in the same
  loop, so a fresh `start` already uses the grammar. (No special case for
  `Finished`: the mic is closed there and the grammar is only compiled when
  the recognizer is built.) A revision applied while `Holding` restarts the
  recognizer mid-hold; accepted (no speech is matched in Holding).
- Kotlin: field `grammar: String?`. Command `"grammar"` stores
  `payload.ifBlank { null }`; if `service != null`, restart audio
  (`stopAudio(false); startAudio()`).
- `startAudio` builds the `Recognizer` on `audioWorker` (both forms;
  compiling a 3000-word grammar on the main thread risks jank/ANR on the A7
  Lite) and posts `SpeechService` creation and `startListening` back to the
  main thread, dropping the result if `audioGeneration` changed or
  `wantedAudio` became false meanwhile (closing the recognizer).
- Grammar construction: if `grammar != null` and the model folder lacks
  `graph/Gr.fst` or `graph/HCLr.fst`, emit `grammar_error` and build without
  a grammar. Otherwise `Recognizer(model, 16000f, grammar)` inside
  `try/catch`; on an exception (native creation failed), emit
  `{type:"grammar_error", message:"Script vocabulary is not supported by
  this voice model. Using full vocabulary."}`, set `grammar = null` and
  fall back to `Recognizer(model, 16000f)`. Note: a model without runtime
  graphs makes libvosk log "Runtime graphs are not supported" and silently
  use the full vocabulary without throwing; the file check above is what
  catches that case. The bundled `vosk-model-small-en-us-0.15` has both
  files.
- UI: `"grammar_error"` event sets `ui.message`.

## Error handling

- Malformed key events (missing `key`) are ignored. Unknown keys are not in
  the allowlist, so they are not prevented and never reach Rust.
- A lost key-up cannot leave a key "held" forever (`STUCK_KEY_MS`, window
  blur → `key_reset`), and a stuck key's auto-repeat cannot re-fire its hold.
- Backgrounding during a pre-roll pauses it (existing `pause` event); resume
  restarts the full roll.
- Remote actions while the voice model is not ready show `voice_status` as
  the message and the toast, without starting.
- Grammar construction failure or a model without runtime graphs falls
  back to the unrestricted recognizer with a visible message; an oversized
  vocabulary sends no grammar and shows a message.
- Settings out of range are clamped; old JSON loads with defaults.
- Number expansion of absurd inputs (more than 12 digits, failed parses,
  non-ASCII digits) falls back to digit-by-digit or plain words; no panics
  on any input (tested with a span-invariant test).
- A saved position from an older word scheme is ignored, not misapplied.

## Testing

**Core** (new files in `crates/core/tests`): `numbers.rs`, `cleanup.rs`,
`endings.rs`, `resync.rs`, `preroll.rs`, `feedback.rs`, `estimate.rs`,
`remote.rs`, `grammar.rs`, plus `hold.rs` (`listening_phases` extended; new
tests added next to `settings_clamp_and_old_json`, which itself is not
edited) and `playback.rs` (new assertions only in new tests). Test names are
listed per task below. All existing tests stay green and unchanged, except
`listening_phases`, which is extended (not weakened).

**UI** (QA page, built-in browser): pre-roll overlay and flash, heard dot,
lost hint, estimates, settings controls, remote keys, fake native key
events, grammar command sent, render-rate check on a long slide. A plain PC
browser page without the QA page's fake `RustyNative` (the companion's own
Studio page) is used for the non-native key path.

**Device** (add to `VALIDATION.md` as required, not claimed): real HID
clicker and pedal (first press after idle and after a permission dialog,
hold timing, down/up-pair clickers), volume keys opt-in, first words after
pre-roll not lost and "3-2-1" not matched, flash visible in separate-camera
footage, grammar mode on the A7 Lite with the bundled model (recognizer
build time off the main thread), how Vosk speaks years and large numbers
("two thousand twenty four" vs "twenty twenty four", "fifteen hundred"),
heard-dot render cost on a 1000-word slide, a 30-minute remote-driven
session.

## Files touched

- `crates/core/src/lib.rs`: Settings fields; Session fields, `Rolling`,
  `mic_open`, `start`/`pause`/`tick`/`rewind_to`/`move_to` changes,
  `forward`, `skip_roll`, `read_again`, `hearing`, `lost`, `dismiss_lost`,
  `upcoming`, `tick_changes_view`, `time_left_ms`; `Position.words_scheme`,
  `WORDS_SCHEME`; Tracker `Outcome`, cleanup, `[unk]` handling, end guard,
  backward resync; `words()` scanner, `unit_start`, `unit_end`,
  `sentence_starts` `$`, `similar()` number exemption; estimate functions;
  `vocabulary`, `grammar_json`, `grammar_skip_reason`.
- `crates/core/src/numbers.rs` (new), `crates/core/src/remote.rs` (new).
- `crates/core/tests/*.rs` (new files listed above; `hold.rs` extended).
- `crates/ui/src/app.rs`, `crates/ui/src/interop.js`,
  `crates/ui/assets/main.css`.
- `android/app/src/main/java/com/rustyprompter/app/MainActivity.kt`.
- `scripts/qa.py` (test adapter only).
- `README.md`, `INSTALL.md`, `VALIDATION.md`, Help modal text in `app.rs`.

## Implementation tasks

Tasks run **sequentially**, in the order listed, each by a fresh agent
editing the same files. Each task must leave
`python scripts\task.py cargo test -p rusty-core` and the clippy command
green before finishing, must not leave a private item without a reader
(see Constraints), and must not change behaviour owned by a later task.
Read this whole spec first; the "Design" section is the source of truth for
names and rules. Order: B1, B2a, B2b, C1, C2, B3a, B3b, B4, B5, B6, C3, B7,
B8a, B8b, B9, B10, B11, C4, Z1.

### B1 — Settings fields and Session configuration

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/hold.rs`.
- **Produces:** the eight `Settings` fields with defaults and clamps (§1);
  the **public** `Session` fields of §1 with the stated initial values (no
  private fields: `last_heard` arrives in B4); `configure` copying
  `pre_roll_ms`, `wpm`, `lost_after_ms`, `voice_resync` with the exact
  `(x * 1000.0).round() as u64` form; private associated
  `fn new_tracker(text: &str) -> Tracker` used by `new`, `move_to`,
  `apply_revision`, `restore` (C2 adds the `allow_back` parameter).
- **Tests** (`hold.rs`, added, `settings_clamp_and_old_json` not edited):
  `new_settings_defaults_and_old_json` (old JSON without the new keys loads
  with the §1 defaults), `new_settings_clamp` (each numeric bound, low and
  high), `configure_copies_new_settings` (including `show_lost_hint=false`
  → `lost_after_ms == 0`).
- **Acceptance:** all tests and clippy green; no behaviour change.

### B2a — Numbers module, scanner, spans and word scheme

- **Files:** `crates/core/src/numbers.rs` (new), `crates/core/src/lib.rs`
  (`mod numbers;`, `words()` scanner, `unit_start`, `unit_end`,
  `sentence_starts` `$`, `similar()` number exemption, `WORDS_SCHEME`,
  `Position.words_scheme`, `restore` check), `crates/core/tests/numbers.rs`
  (new).
- **Produces:** `pub fn is_number_word`, `cardinal`/`ordinal`/`year`, the
  scanner and numeric token forms with the year-context rule, the `'s`
  rule, the span rule, `unit_start`, `unit_end`, the word-scheme version
  (§2). Use `[0-9]` only, boundaries by hand, no `unwrap` on parses.
- **Tests** (`numbers.rs`): `cardinals` (0, 7, 13, 21, 99, 100, 123, 1000,
  1,250,000), `years` (`in 1990`, `since 1905`, `by 1900`, `in 2000`,
  `in 2005`, `in 2024`, `in 1066` → cardinal), `year_needs_context`
  (`1500 people` and `2024 members` → cardinal), `ordinals` (1st, 2nd, 3rd,
  21st, 100th, 12th), `money_and_percent` ($1 → one dollar, $5, $2.50,
  $0.50 → fifty cents, $1,250.00, 50%, 2.5%), `decimals_and_leading_zeros`
  (3.14, 007), `verse_parts_keep_their_own_spans` (`John 3:16` spans are
  "3" and "16"), `verse_range_and_time` (8:28-30, 8:28–9:2, 3:05, 10:00),
  `letters_next_to_digits_unchanged` (3D, mp3, covid19, 1990s, 80s,
  5:30pm → `five` `30pm`), `possessive_year` (`in 1990's` → `in nineteen
  ninety`, no stray `s`), `non_ascii_digits_are_plain_words` (`٣` and `３`
  stay single plain words, no panic), `huge_number_digit_by_digit`
  (13+ digits), `spans_never_overlap` (a long mixed string including all
  forms above, `$`, `%`, cues and non-ASCII digits: for all i,
  `start <= end` and `prev.end <= start`, and slicing every gap and word
  succeeds), `unit_start_and_end_walk`, `sentence_start_on_expanded_number`
  (`Done. 21 people came.` → `[0, 1]`), `sentence_start_on_money`
  (`Done. $5 is enough.` → second start is the first part of `$5`),
  `eighty_does_not_match_eighth` (tracker: slide `the eighth day`, final
  `the eighty day` does not reach the last word), `position_saved_with_old_scheme_is_not_restored`
  (JSON without `words_scheme` → `restore` leaves the session untouched;
  a current `position()` round-trips).
- **Acceptance:** all existing tests unchanged and green (notably
  `numbers_are_spoken_words`, `abbreviations_verses_and_blank_lines`,
  `real_offline_recognition_stream`, `long_slide_stays_fast`, all of
  `retakes.rs`).

### B2b — Hypothesis cleanup

- **Files:** `crates/core/src/lib.rs` (private `Cleanup` built in
  `Tracker::new(text)` from text and words; applied in `accept` over
  `context + incoming` with `fresh_from` adjusted), `crates/core/tests/cleanup.rs`
  (new).
- **Produces:** the four cleanup rules of §2 including the "no next token"
  trailing rule and `has_range` from the slide text.
- **Tests** (`cleanup.rs`): `spoken_chapter_and_verse_matches`
  (`Read John 3:16 with me today` + final `read john chapter three verse
  sixteen with me today` → last word), `spoken_psalm_number_matches`
  (`Psalm 23 is a song` + `psalm twenty three is a song`),
  `hundred_and_matches` (`We fed 150 people today` + `we fed one hundred
  and fifty people today`), `a_hundred_matches`, `range_through_matches`
  (`Romans 8:28-30 today` + `romans eight twenty eight through thirty
  today`), `script_word_verse_is_not_dropped` (slide containing the word
  "verse" keeps it), `partial_ending_in_verse_still_advances` (partials
  `john chapter three verse` twice → position on `three`),
  `final_split_between_chapter_and_number` (final `… john chapter`, then
  final `three sixteen …` → advances), `split_hundred_and`
  (final `… one hundred and`, then `fifty people today`).
- **Acceptance:** all existing tests green; the 64-event replay positions
  unchanged.

### C1 — Tracker outcome and guarded end-of-slide

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/endings.rs` (new).
- **Produces:** `pub enum Outcome`, `Tracker.last_outcome` (§3); private
  `Candidate` struct, `Cell.last_content`; end guard E1–E3 with votes
  recorded as today and `pos` taken at call start (§7).
- **Tests** (`endings.rs`): `outcome_advanced_pending_nomatch` (one call
  each), `outcome_idle_on_empty`, `mid_jump_onto_last_word_needs_final`
  (≈30-word slide, position near the start, two identical partials of the
  last 6 words do not finish; a final then does),
  `far_jump_onto_last_word_needs_final` (≈80-word slide, two partial
  sightings of the last sentence do not finish; a final sighting does),
  `far_partial_then_final_finishes` (one far partial sighting, then the same
  text as a final → finishes), `ending_needs_content_word` (context-only
  match of the content tail plus a fresh common word that is not the
  contiguous tail does not finish), `all_common_ending_can_finish` (slide
  ending "… and that is it for us", read normally, finishes),
  `separate_final_common_tail_finishes` (`… Thank you all for coming to
  this`: final `… for coming`, then final `to this` → finishes),
  `word_by_word_finals_finish_common_ending` (one final per word near an
  ending `… thank all of you`), `normal_read_still_finishes_with_partials`
  (word-by-word partials within 4 words of the end still finish on the
  second matching partial).
- **Acceptance:** all existing tests green, including
  `separate_finals_keep_context`, `short_slide_one/two`,
  `lone_ending_word_cannot_finish`, the 64-event replay and all of
  `hold.rs`.

### C2 — Voice backward resync

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/resync.rs` (new),
  `crates/core/tests/playback.rs` (one new test).
- **Produces:** `Tracker.allow_back`, `back_end`/`back_votes` with the
  clearing rule, `back_candidate` with near window, coverage and
  `start_row` rules, `saturating_sub` arithmetic, commit with
  `reset_utterance` (context cleared) and `Outcome::Back` (§8);
  `new_tracker(text, allow_back)` with callers passing `true` (`new`) or
  `self.voice_resync`; `configure` also sets `tracker.allow_back`.
- **Tests** (`resync.rs`): `rereading_earlier_sentence_moves_back` (read 3
  sentences, final re-reading sentence 1 → position at the end of the
  re-read words, outcome `Back`), `back_resync_needs_final_or_two_partials`,
  `back_vote_cleared_by_intervening_forward_advance`,
  `back_vote_cleared_by_nomatch`, `rereading_last_words_does_not_move_back`
  (repeat the last 3 words), `common_words_never_resync_back`,
  `final_ending_at_position_does_not_resync` (final re-sends a 9-word
  utterance that ends at the position),
  `repeated_passage_resyncs_to_nearest_copy`,
  `repeated_passage_three_words_past_near_copy_does_not_jump_to_far_copy`,
  `adlib_quoting_earlier_sentence_does_not_resync`,
  `forward_after_resync_ignores_pre_resync_context`, `resync_keeps_take`
  (`Session.take` unchanged, phase stays `Reading`),
  `resync_disabled_by_setting` (`voice_resync=false` via `configure`),
  `allow_back_survives_new_trackers` (after `next()`, `jump`,
  `apply_revision` and `restore`), `small_position_no_underflow` (position
  2 with a 5-token final), `forward_reading_after_resync_continues`;
  `playback.rs`: `replay_never_resyncs_back` (the 64-event fixture with
  `last_outcome != Back` on every line).
- **Acceptance:** all existing tests green, especially
  `revised_partial_never_reverses`, `repeated_paragraph_stays_on_nearest_copy`,
  `shared_phrase_ad_lib_does_not_jump`,
  `full_read_of_repetitive_script_tracks_every_sentence`,
  `long_slide_stays_fast` and the 64-event replay (monotonic).

### B3a — Pre-roll phase rules

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/preroll.rs` (new),
  `crates/core/tests/hold.rs`.
- **Produces:** `Phase::Rolling`, `running`/`listening`/`mic_open`/`pause`
  rules, `enter_roll` (non-empty slide only), `finish_roll`, `start` rules,
  `rewind_to(next_word, roll)` with `retake`/`back_sentence`/`seek`,
  `move_to` (restart roll from Rolling, else `roll_ms = 0`), and the
  `Rolling` arm of `tick` (`roll_ms` countdown → `finish_roll`) so the
  phase can end (§5).
- **Tests** (`preroll.rs`): `preroll_off_starts_reading` (default),
  `start_from_ready_rolls_then_reads` (3 s: 2999 ms still Rolling, 1 ms more
  → Reading and `sync_mark == 1`), `speech_ignored_while_rolling` (same
  take: position and outcome unchanged, no votes armed — a following
  identical partial in Reading does not commit on one sighting),
  `rolling_is_running_and_mic_open_not_listening`,
  `pause_in_roll_restarts_full_roll`, `resume_reading_rolls_again`,
  `resume_countdown_does_not_roll`, `retake_and_back_sentence_roll`,
  `take_bumped_once_by_retake_from_rolling` (and never by `finish_roll`),
  `stale_take_speech_after_roll_is_dropped`, `seek_does_not_roll`,
  `slide_transitions_never_roll` (countdown → next, holding → start),
  `retake_on_wordless_slide_with_preroll_stays_put`,
  `jump_during_roll_restarts_roll`, `pause_revision_then_start_rolls`
  (pause in Rolling, `apply_revision` while Paused, `start` → Rolling),
  `configure_during_roll_keeps_roll_ms`, `revision_refused_while_rolling`;
  `hold.rs` `listening_phases` extended with `!Phase::Rolling.listening()`,
  `Phase::Rolling.running()` and `Phase::Rolling.mic_open()`.
- **Acceptance:** all existing tests green (pre-roll defaults to off).

### B3b — Forward, skip, read again and take time

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/preroll.rs`.
- **Produces:** `skip_roll`, `read_again`, `forward` (full §5 table
  including `ROLL_GUARD_MS`, Paused-from-Holding → `next(); start()`,
  Finished → `read_again()`), and the Countdown/Reading/Holding `elapsed_ms`
  parts of `tick` (§5, §6 elapsed rule), `elapsed_ms` reset in `read_again`
  and `restart`.
- **Tests** (`preroll.rs`): `skip_roll_marks_sync`,
  `read_again_rolls_from_first_slide`, `forward_by_phase` (Ready, Paused,
  Rolling after 600 ms, Reading, Countdown, Holding, Finished),
  `forward_right_after_start_keeps_roll` (Forward at 100 ms into the roll
  is ignored), `forward_from_paused_holding_goes_next`,
  `elapsed_counts_reading_not_rolling`, `restart_resets_elapsed`.
- **Acceptance:** all tests green.

### B4 — Hearing and lost place

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/feedback.rs` (new).
- **Produces:** `HEARD_MS`, `SPEAKING_MS`, private `last_heard`, speech/tick
  accounting with the exact `is_some_and` expressions and the
  speaking-portion rule (§4), `hearing`, `lost`, `dismiss_lost`,
  `upcoming` (word indices, unit-end extension), `tick_changes_view`, and
  the resets listed in §4 (plus `finish_roll`).
- **Tests** (`feedback.rs`): `not_hearing_before_any_speech`,
  `hearing_after_new_speech_then_decays`,
  `repeated_identical_partial_is_not_new_speech`,
  `hearing_in_countdown_but_not_paused_or_rolling`,
  `stale_take_speech_does_not_light_dot`,
  `lost_after_speaking_without_progress` (5 s of varying unrelated
  finals/partials with ticks → `lost()`), `silence_does_not_count_toward_lost`
  (Reading, no speech, 10 s of ticks → not lost),
  `batched_ticks_equal_single_ticks` (one 3000 ms tick vs sixty 50 ms ticks
  give the same `stuck_ms`, `heard_ago_ms`, `elapsed_ms`),
  `progress_clears_lost`, `back_resync_clears_lost`,
  `dismiss_and_rewind_clear_lost`, `lost_disabled_when_hint_off`,
  `upcoming_covers_next_words` (word indices; end of slide → `None`),
  `upcoming_extends_to_unit_end` (range ending inside `3:16`'s units or
  `$2.50` covers the whole unit), `tick_changes_view_cases` (false in
  Ready/Paused/Finished and for a quiet 50 ms in Reading; true in
  Countdown/Rolling, when the dot decays past 700 ms, when `lost()` flips,
  and when `elapsed_ms` crosses a whole second).
- **Acceptance:** all tests green.

### B5 — Run-time estimate

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/estimate.rs` (new).
- **Produces:** `estimate_ms`, `slide_estimate_ms`, `script_estimate_ms`,
  `format_duration`, `Session::time_left_ms` (§6).
- **Tests** (`estimate.rs`): `estimate_at_150_wpm` (150 words → 60000),
  `cues_do_not_count`, `numbers_count_as_spoken`, `script_total_adds_gaps`
  (3 playable slides + 1 empty → 2 gaps), `hold_mode_has_no_gaps`,
  `format_duration_cases` (0, 999, 1000, 61000, 3723000),
  `time_left_shrinks_as_reading_progresses`,
  `time_left_in_countdown_uses_remaining`,
  `time_left_paused_from_countdown_uses_remaining`,
  `time_left_in_rolling_adds_roll`, `time_left_in_holding`,
  `time_left_finished_is_zero`.
- **Acceptance:** all tests green.

### B6 — Remote key mapper

- **Files:** `crates/core/src/remote.rs` (new), `crates/core/src/lib.rs`
  (`mod remote; pub use remote::*;`), `crates/core/tests/remote.rs` (new).
- **Produces:** everything in §9, including `Play`, `MediaPlay`/`MediaPause`,
  the repeat-down rule, the stuck-key `ignored` rule and the repeat guard.
- **Tests** (`remote.rs`): `every_bound_key_short_press`,
  `keys_without_hold_fire_on_down`, `hold_fires_once_on_repeat`,
  `hold_fires_from_tick_without_repeats`, `hold_suppresses_short`,
  `auto_repeat_ignored`, `up_without_down_ignored`,
  `first_event_repeat_is_ignored`, `other_key_cancels_hold`,
  `no_hold_key_while_other_key_held` (fires at once, held key discarded),
  `stuck_key_cleared`, `repeat_after_stuck_discard_does_not_refire`,
  `reset_clears_held`, `double_forward_within_debounce_fires_once`,
  `clicker_down_up_pairs_while_held_do_not_skip_slides` (PageDown down/up
  every 40 ms for 2 s → one Forward), `forward_after_guard_fires_again`
  (two presses 500 ms apart → two), `media_play_and_pause_are_distinct`,
  `volume_keys_only_when_enabled`, `letters_case_insensitive_and_space_aliases`,
  `keys_list_matches_bindings`.
- **Acceptance:** all tests green.

### C3 — Vocabulary, grammar JSON and `[unk]`

- **Files:** `crates/core/src/lib.rs`, `crates/core/tests/grammar.rs` (new).
- **Produces:** `GRAMMAR_MAX`, `vocabulary`, `grammar_json`,
  `grammar_skip_reason` (§10); `[unk]` → `zzunk` in `Tracker::accept`,
  `zzunk` never-equal in matching, `Session::speech` `[unk]`-only rule.
- **Tests** (`grammar.rs`): `vocabulary_is_sorted_unique_with_unk_last`,
  `cues_excluded_numbers_expanded`, `number_and_filler_words_included`,
  `grammar_json_is_a_json_array`, `oversized_vocabulary_returns_none`
  (reason `too_large`), `vocabulary_of_wordless_script` (empty; JSON
  `None`, reason `empty`), `unk_tokens_keep_adlib_penalty` (`the [unk]
  [unk] lord` does not align as `the lord`), `unk_only_partial_is_idle`
  (no move, not heard, no `stuck_ms`).
- **Acceptance:** all tests green. Core work is complete after this task.

### B7 — QA adapter events

- **Files:** `scripts/qa.py` (test adapter only).
- **Produces:** QA buttons/functions: `qaKey(key, down, repeat=false)`
  pushing `{type:"key",key,down,repeat}`; "Pedal tap" (PageDown down+up),
  "Pedal hold" (PageUp down, repeats every 100 ms, then up after 1200 ms),
  "Bouncy clicker" (PageDown down/up pairs every 40 ms for 1 s);
  `qaPartial(text)` (final:false); "Speak verse" (`john chapter three verse
  sixteen`); "Mumble" (every 400 ms for 6 s pushes a partial whose text
  changes each time, e.g. `blue cat 1`, `blue cat 2` spelled as words:
  `blue cat one`, `blue cat two`, …, unrelated to the slide); "Speak unk"
  (partial `[unk] [unk]`); fake `RustyNative.command` logs `remote`,
  `reset` and `grammar` payloads into a `#qa-native` span. The QA library
  gains a slide containing `John 3:16`, `$5`, `in 2024` and `21st`, and a
  1000-word slide for the render-rate check.
- **Acceptance:** `python scripts\qa.py` serves the page; unknown events are
  harmlessly ignored by the current UI.

### B8a — UI: pump budget, pre-roll, flash and actions

- **Files:** `crates/ui/src/app.rs`, `crates/ui/assets/main.css`.
- **Produces:** pump `clock_ms` batching with `tick_changes_view` and forced
  writes on flash/toast expiry (§11); mic `desired` from `mic_open()`; local
  `seen_mark` with post-roll `native_call("reset", take)` and
  `flash_until`; actions `forward`/`play`/`skip_roll`/`dismiss_lost`,
  `toggle` → `read_again` on Finished, voice guard (today's condition) on
  `toggle`/`forward`/`play`; "Get ready" status, "Cancel" label, pre-roll
  overlay, `.sync-flash` outside `.prompt-mirror`, active-word on
  `unit_end` for zero-width parts, word click → `unit_start`; Help text:
  pre-roll (including the first-use permission dialog note) and "numbers
  can be spoken naturally".
- **Tests:** UI has no unit tests; `python scripts\bundle_ui.py` must build
  with no new warnings. Manual QA: pre-roll 3 s → overlay counts 3-2-1,
  flash covers the whole screen (status bar and footer too), Reading; the
  fake native log shows no `stop`/`start` around the roll and one `reset`
  when it ends; pause during roll; Retake rolls; "Start now"; on the
  1000-word QA slide, in Reading with no speech, the Prompter renders about
  once per second (count renders with a temporary `console.count` or the
  browser performance panel; remove any instrumentation).
- **Acceptance:** core tests and clippy still green; QA checks pass.

### B8b — UI: settings sheet

- **Files:** `crates/ui/src/app.rs`.
- **Produces:** all settings controls of §11 with their
  `SettingSlider`/`SettingSwitch` match arms (native-only switches simply
  save settings for now).
- **Tests:** bundle builds; settings persist and reload; old saved settings
  load with defaults.
- **Acceptance:** as above.

### B9 — UI: heard dot, lost hint, run-time display

- **Files:** `crates/ui/src/app.rs`, `crates/ui/assets/main.css`.
- **Produces:** heard dot classes and CSS, lost-hint pill with text built
  from `upcoming(6)` word indices (no cues) and `hint-word` class, header
  elapsed / time left, Studio per-slide and total estimates, home-list
  estimate, rail-tip fix, removal of both `140.0` constants (§11).
- **Tests:** bundle builds; QA: speak partials → dot enlarges then settles;
  "Mumble" → hint appears after ~5 s, Dismiss hides it, "Sentence back"
  works; the hint never shows a stage cue; estimates change with the pace
  slider; the render-rate check of B8a still holds with the dot and times
  shown.
- **Acceptance:** as above.

### B10 — UI and JS: remote keys

- **Files:** `crates/ui/src/interop.js`, `crates/ui/src/app.rs`,
  `crates/ui/assets/main.css`.
- **Produces:** `set_remote` export and extern, keydown/keyup/blur
  listeners with the modifier, composition, form-field and dialog/sheet
  exclusions (§12), pump-local `Remote`, `Ui.remote_toast`,
  `remote_action`, pump handling of `key`/`key_reset` and `remote.tick`,
  `prompting` that is false while a sheet or modal is open, `set_remote`
  and `native_call("remote", …)` on change, toast markup/CSS, Help
  paragraph "Remote & keys" with the §9 table in plain words (media keys
  noted as Android-only).
- **Tests:** bundle builds. QA page (fake native): "Pedal tap" advances,
  "Pedal hold" fires Retake once, "Bouncy clicker" advances one slide only,
  the `remote` payload is logged when the prompter opens and closes.
  **Plain PC browser** (the companion's own Studio page, which has no fake
  `RustyNative`; open the prompter there) with a real keyboard: PageDown
  starts/advances, hold PageDown pauses, PageUp = sentence back, hold
  PageUp = retake, Space toggles without clicking a focused button, B
  pauses; with the settings sheet or Help open, Enter/Space act on the
  focused control and do not toggle; Alt+ArrowLeft, Ctrl+B and Ctrl+Enter
  are not intercepted; typing Space in the Studio editor is unaffected.
  Media keys are not part of the PC checklist.
- **Acceptance:** as above.

### B11 — Android: key forwarding

- **Files:** `android/app/src/main/java/com/rustyprompter/app/MainActivity.kt`.
- **Produces:** `prompting`/`volumeKeys` fields, `"remote"` command with
  `web.requestFocus()` on `prompting=true`, `dispatchKeyEvent` override
  forwarding and consuming every keycode in the §12 table while prompting
  (volume only when enabled, no modifiers).
- **Tests:** Android build (`python scripts\build.py` or `BUILD-APK.bat`)
  compiles if the Android toolchain is present; otherwise state that it was
  not built. Device checks go to `VALIDATION.md` (Z1).
- **Acceptance:** keys only consumed while prompting; volume keys only when
  enabled; no other behaviour change.

### C4 — Grammar plumbing (UI and Android)

- **Files:** `crates/ui/src/app.rs`,
  `android/app/src/main/java/com/rustyprompter/app/MainActivity.kt`.
- **Produces:** pump `grammar_key` logic (no grammar for a script without
  playable words), `grammar_skip_reason` message, `"grammar_error"`
  handling (§13); Kotlin `grammar` field, `"grammar"` command with restart,
  recognizer built on `audioWorker` with the generation check, `Gr.fst` /
  `HCLr.fst` check, 3-arg `Recognizer` in `try/catch` with fallback.
- **Tests:** bundle builds; QA page shows the `grammar` payload when the
  switch is on, an empty payload when off, and the "too large" message for
  an oversized script; "Speak unk" does not move the highlight or light
  the dot; Android build as in B11.
- **Acceptance:** with the switch off, native receives `""` once and audio
  behaviour is unchanged (apart from the off-main-thread recognizer build,
  which must not change what is heard).

### Z1 — Docs and full verification

- **Files:** `README.md` (feature list), `INSTALL.md` (remote keys,
  pre-roll including the first-use permission note, settings, the one-time
  saved-position reset after upgrade), `VALIDATION.md` (device checks
  listed under Testing, marked as required, not claimed).
- **Tests:** run the full core suite, companion tests
  (`python scripts\task.py cargo test -p rusty-companion`), clippy, the UI
  bundle and the QA walkthrough of B8a–B10 and C4.
- **Acceptance:** everything green; any unverified item (device, Android
  build) is listed explicitly as not verified.
