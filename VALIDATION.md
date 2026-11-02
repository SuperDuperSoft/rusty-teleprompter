# RustyPrompter validation

## Releases B and C — November 2, 2026

### Automated checks

- **222 shared Rust engine tests** passed: cleanup 9, endings 10, estimate 12, feedback 15, grammar 8, highlight 4, hold 20, jumps 11, numbers 19, playback 37, preroll 24, remote 20, resync 16, retakes 17. New areas: number and verse normalization, hypothesis cleanup, tracker outcome and guarded end-of-slide, voice backward resync, 3-2-1 pre-roll and Forward, heard dot and lost-place timing, run-time estimates, the remote key mapper (hold, debounce, stuck keys, down/up-pair clickers) and the script vocabulary / `[unk]` handling.
- The **64-event offline Vosk replay** still reaches the correct ending, and never triggers a backward resync.
- **18 Rust server integration tests** passed.
- **4 UI unit tests** (remote toast text) passed natively.
- Clippy passed with warnings treated as errors for core, companion and tests, and for the UI crate on wasm32.
- The Dioxus/WASM bundle built. The Android release build (lintRelease and assembleRelease) passed.

### Browser checks (QA page with the test adapter)

Run in the built-in browser on the real Dioxus/WASM bundle with the unshipped QA adapter:

- Home row and prompter header show estimates ("about 8 min", "~7:22 left") and take time.
- Pedal tap started reading; a partial lit the heard dot, which went back to the listening pulse after it; `[unk]`-only speech neither moved the highlight nor lit the dot.
- Mumbling without progress showed the lost-place hint with the next words; Dismiss hid it.
- Bouncy clicker (25 down/up pairs in 1 s) advanced exactly one slide; Pedal hold retook the slide (new take, microphone reset); B paused.
- Pre-roll 3 s: overlay "Starting in 3", "Take N", Start now and Cancel; the microphone stayed open and received one reset with the same take when the roll ended; the white sync flash showed for about 300 ms. Retake rolled again; Start now ended the roll with a reset and flash; Cancel paused and stopped the microphone; resuming rolled the full pre-roll again.
- New settings saved to local storage with their defaults; turning on Script vocabulary sent the vocabulary JSON to the fake native host, turning it off sent an empty grammar.
- Earlier task checks (render rate on a 1000-word slide, real-keyboard keys and modifier passthrough, the non-native Preview key path, the "too large" vocabulary message, Studio estimates) were done when those features were built and are not repeated here.
- Not checked in the browser: a lost-place hint on a slide with a [stage cue] (the QA library has no cues; the hint is built from word indices only).

## Release A — October 29, 2026

### Automated checks

- **85 shared Rust engine tests** passed (highlight 4, hold 17, jumps 11, playback 36, retakes 17), covering: omitted words, fillers, skipped sentences, unrelated speech, repeated phrases, stable/revised partials, final-result context, numeric words, UTF-8 offsets, directions, navigation, empty slides, exact countdown duration, countdown pause/resume, pending-revision rules, saved-position behavior, faster highlight commit, retake and take counter, Back a sentence, Hold at end of slide and 1–15 s countdown lengths.
- Includes replay of **64 actual offline Vosk recognition events** from a generated English speech fixture. The Rust matcher reaches the correct ending despite omissions and recognition errors.
- **18 Rust server integration tests** passed: authenticated local editing, denial of remote editor access, optimistic revision checks, concurrent saves, slide ordering, input limits, pairing/rate limiting, token persistence/revocation and real WebSocket library syncing.
- Rust Clippy passed with warnings treated as errors for the core, companion and tests.
- Android release compilation and lint passed. Remaining Android lint warnings concern fixed English UI messages in the small platform adapter.
- Packaged Windows executable smoke test passed using a temporary database: native Rust startup, redb persistence, local editor authentication, bundled interface, pairing and revocation.

### Browser checks using the actual Dioxus/WASM application

- Created scripts and separate slides, typed titles/bodies, autosaved, reordered, and reloaded to verify persistence.
- Exported a structured JSON backup and imported it again, preserving all three slides and their order.
- Tested the real shared tablet interface with a dedicated, unshipped adapter that supplies controlled Android-style speech/connection events. The Rust UI and engine themselves are unmodified.
- Verified that completing slide one starts a five-second countdown; pausing holds it beyond five seconds; resuming advances to slide two and starts listening.
- Verified queued PC edits remain outside the active take, then apply on Pause.
- Verified final-slide completion stops listening and displays Finished.
- Verified normal, horizontal, vertical and combined mirroring through the displayed transform state.
- Verified a background-app event pauses reading and stops the audio request.
- Checked tablet-sized layout at 600 × 960 CSS pixels and the default desktop layout.

The fixture page is created only in the development test directory and is not included in either application download.

## Physical-device acceptance

Actual-device results are recorded here after installation when an authorized phone/tablet is available. Browser adapter tests do **not** establish microphone accuracy or performance on the A7 Lite.

The physical Galaxy Tab A7 Lite still requires a continuous **30-minute read** with the actual microphone distance, noise conditions and screen orientation. Check startup, pairing over the real Wi-Fi, voice lag, skipped words/sentences, pauses/ad-libs, all mirror combinations, countdown pause/resume, mid-take edits and operation with Wi-Fi/PC switched off.

### Required for Release A (reliable takes) — NOT yet claimed

These items need the physical Galaxy Tab A7 Lite and have not been run:

- Timeout recovery: the microphone resumes by itself after a recognizer timeout or error.
- Reset at slide change without losing the first spoken words of the new slide.
- The microphone stays open through the countdown and through Hold at end of slide.
- After Retake or Sentence, words spoken just before the tap don't move the highlight (speech decoded right after the recognizer reset can carry the new take).

### Required for Releases B and C — NOT yet claimed

These need real hardware (mostly the Galaxy Tab A7 Lite) and have not been run. Nothing below has been verified on a device:

- A real HID Bluetooth **clicker** and **pedal**: the first press after idle, the first press after a permission dialog, hold timing (about 0.9 s to pause/retake), and cheap clickers that send down/up pairs while held (must not skip slides).
- **Media keys** (play/pause, play, pause, next/previous track) forwarded on the tablet.
- **Volume keys** as remote: ignored by the app (normal volume) when the setting is off; Forward/Back only when it is on.
- **Pre-roll**: the first words spoken right after the roll are not lost, and the spoken or heard "3-2-1" is never matched to the script; the first-use microphone permission prompt during the roll restarts the roll once allowed.
- **Sync flash** clearly visible in footage from a separate camera.
- **Script vocabulary (grammar) mode** on the A7 Lite with the bundled model: it builds, the recognizer build happens off the main thread (no UI freeze), switching it on/off and changing scripts restarts listening correctly, and the fallback message appears if the build fails.
- How Vosk actually speaks **years and large numbers** ("two thousand twenty four" vs "twenty twenty four", "fifteen hundred") against the number rules.
- **Heard-dot render cost** on a 1000-word slide on the tablet (no visible lag while speaking).
- A **30-minute remote-driven session** (clicker or pedal only, no touches).
