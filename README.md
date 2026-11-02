# RustyPrompter

A new, subscription-free teleprompter built with **Rust and Dioxus**. It installs alongside PromptLocal and uses a separate application ID, database and pairing port.

- Edit an ordered set of script slides on your Windows PC; changes autosave and sync over your private Wi-Fi network.
- Read one slide at a time on Android. Offline speech recognition and forgiving phrase matching follow your voice.
- Reach the end of the spoken text and a **countdown** (5 seconds by default, 1–15 in settings) opens the next nonempty slide. Pause freezes the countdown. Finishing the final slide ends the session.
- **Retake** restarts the slide with a new take; speech from before the retake is ignored. **Sentence** steps back one sentence.
- Optional **Hold at end of slide** (off by default) waits for you at the end of each slide instead of counting down.
- Android listening recovers by itself from recognizer timeouts and errors, and resets on retake and slide change.
- Mirror horizontally, vertically or both. Adjust text size, margins, line spacing, reading guide and orientation.
- Pause during a take to apply queued PC edits; the entire active revision stays stable until then.
- Import/export plain text and structured JSON backups that preserve slide boundaries and order.
- **Bluetooth clicker and pedal** support: Page Down/Up, arrows, Space, Enter, B and period drive Next, Sentence back, Retake (hold back) and pause (hold next), with debounce so a bouncy clicker cannot skip slides. Media keys work on Android; volume keys are an opt-in remote.
- **Numbers, Bible references, money, percentages, years and ordinals** are followed when spoken naturally ("John three sixteen", "five dollars", "twenty first").
- An **"I hear you" dot** lights when speech arrives, and a dismissible **lost-place hint** shows the next words when you have been speaking for a while (5 s by default) without the highlight moving.
- Optional **3-2-1 pre-roll** (off by default, up to 10 s) before each take, ending in a white **sync flash** for a separate camera.
- **Run-time estimates** at a configurable speaking pace (150 wpm by default) in Studio, the tablet home list and the prompter, plus elapsed time and time left while reading.
- **Guarded end-of-slide**: a slide only completes when its final words were actually heard. **Follow re-reading** (on by default) moves the highlight back when you re-read earlier text.
- Optional **script vocabulary** (beta, off by default) restricts offline recognition to the script's words on Android.

## Implementation

`crates/core` is the shared, deterministic Rust engine: UTF-8-aware tokenization, fuzzy phrase alignment, playback state, saved positions and revision anchoring.

`crates/ui` is the actual Dioxus application, compiled to WebAssembly. The same offline bundle provides the PC script studio and Android prompting interface. A small JavaScript module handles browser/platform plumbing; it does not implement the matching algorithm or playback state machine.

`crates/companion` is a native Rust Windows executable using Axum, Tokio and the pure-Rust redb transactional database. It opens the local script studio in your browser and displays a small native status window. **No Python runtime is required to use it.**

`android` is a thin Kotlin platform host: the system WebView displays the bundled Rust/Dioxus UI, and the adapter handles microphone permission, offline Vosk recognition, local caching and authenticated WebSocket transport. Vosk never calls a cloud speech service. UI, matching, timing and script-version decisions remain in Rust.

The shared UI is packaged explicitly with Cargo and upstream wasm-bindgen. This avoids a Windows path failure in Dioxus CLI 0.7.10's bundler. Dioxus itself remains the application framework; there is no dependency on a running development server or externally hosted page.

## Using it

See [INSTALL.md](INSTALL.md). Downloads are generated in `dist/`:

- `RustyPrompter-Windows.zip`
- `RustyPrompter-Android.apk`
- `RustyPrompter-Source.zip`
- `RustyPrompter-Signing-Backup.zip` — private backup, not for public sharing

The PC stores scripts in `%LOCALAPPDATA%\RustyPrompter\rustyprompter.redb` and listens on port **8766**. Editor access is restricted to the local PC. Paired tablets have read-only library access. Sync traffic is unencrypted and intended only for a trusted private LAN; do not forward the port to the internet.

## Development

Build scripts use Python 3.11 solely as development orchestration. They install compiler/linker tools into the parent workspace's `.tools/rustyprompter` folder and leave global PATH unchanged.

```powershell
python scripts\bootstrap.py
python scripts\android_tools.py
python scripts\build.py
```

`scripts/task.py cargo test -p rusty-core -p rusty-companion` runs the Rust tests. The Dioxus UI can be rebuilt separately with `python scripts\bundle_ui.py`. Cargo.lock pins dependencies. Keep `.signing` or its private backup for future Android updates.

The app still requires microphone, layout and 30-minute acceptance testing on the physical Galaxy Tab A7 Lite, plus real clicker/pedal, pre-roll, sync-flash and script-vocabulary checks; desktop/model fixtures cannot establish tablet performance. See [VALIDATION.md](VALIDATION.md).
