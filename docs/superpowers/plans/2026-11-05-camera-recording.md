# Camera recording — implementation plan

Spec: `docs/superpowers/specs/2026-11-05-camera-recording-design.md`.
Executed inline.

1. **Core names (TDD).** `crates/core/src/recording.rs` + `tests/recording.rs`:
   `safe_stem` keeps `A-Z a-z 0-9 - _`, turns spaces into `_`, trims to 100
   chars, rejects empty; `file_stem(title, stamp)`.
2. **Companion endpoint (TDD).** `tests/recordings.rs` first. Then
   `paths::videos_folder_for`, `AppState.recordings`, route
   `/api/recordings/{stem}` with its own 32 MB body limit.
3. **UI.** `Camera` state + `camera_*` methods on `Ui`, event handling,
   pump timer + auto-close, header/footer buttons, CSS.
   `interop.js`: `camera()`, `local_stamp()`.
4. **Android.** Manifest `CAMERA` + optional camera feature. Camera2 preview,
   MediaRecorder to MediaStore, permission request 102, lifecycle, bridge
   commands `camera_open/close/flip/record/stop`.
5. **Verify.** `cargo test` core + companion, native type-check of the UI,
   review, README/INSTALL/VALIDATION notes, commit. User builds APK + PC zip
   and runs the manual checks in the spec.
