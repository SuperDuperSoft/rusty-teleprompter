# RustyPrompter

A new, subscription-free teleprompter built with **Rust and Dioxus**. It installs alongside PromptLocal and uses a separate application ID, database and pairing port.

- Edit an ordered set of script slides on your Windows PC; changes autosave and sync over your private Wi-Fi network.
- Read one slide at a time on Android. Offline speech recognition and forgiving phrase matching follow your voice.
- Reach the end of the spoken text and a **countdown** (5 seconds by default, 1–15 in settings) opens the next nonempty slide. Pause freezes the countdown. Finishing the final slide ends the session.
- **Retake** restarts the slide with a new take; speech from before the retake is ignored. **Back a sentence** steps to the previous sentence.
- Optional **Hold at end of slide** (off by default) waits for you at the end of each slide instead of counting down.
- Android listening recovers by itself from recognizer timeouts and errors, and resets on retake and slide change.
- Mirror horizontally, vertically or both. Adjust text size, margins, line spacing, reading guide and orientation.
- Pause during a take to apply queued PC edits; the entire active revision stays stable until then.
- Import/export plain text and structured JSON backups that preserve slide boundaries and order.

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

The first release still requires microphone, layout and 30-minute acceptance testing on the physical Galaxy Tab A7 Lite; desktop/model fixtures cannot establish tablet performance. See [VALIDATION.md](VALIDATION.md).
