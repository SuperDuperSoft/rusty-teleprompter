# RustyPrompter validation — October 29, 2026

## Automated checks

- **85 shared Rust engine tests** passed (highlight 4, hold 17, jumps 11, playback 36, retakes 17), covering: omitted words, fillers, skipped sentences, unrelated speech, repeated phrases, stable/revised partials, final-result context, numeric words, UTF-8 offsets, directions, navigation, empty slides, exact countdown duration, countdown pause/resume, pending-revision rules, saved-position behavior, faster highlight commit, retake and take counter, Back a sentence, Hold at end of slide and 1–15 s countdown lengths.
- Includes replay of **64 actual offline Vosk recognition events** from a generated English speech fixture. The Rust matcher reaches the correct ending despite omissions and recognition errors.
- **18 Rust server integration tests** passed: authenticated local editing, denial of remote editor access, optimistic revision checks, concurrent saves, slide ordering, input limits, pairing/rate limiting, token persistence/revocation and real WebSocket library syncing.
- Rust Clippy passed with warnings treated as errors for the core, companion and tests.
- Android release compilation and lint passed. Remaining Android lint warnings concern fixed English UI messages in the small platform adapter.
- Packaged Windows executable smoke test passed using a temporary database: native Rust startup, redb persistence, local editor authentication, bundled interface, pairing and revocation.

## Browser checks using the actual Dioxus/WASM application

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
