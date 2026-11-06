# Camera recording — record yourself with the script on top

Date: 2026-11-05
Status: Approved in chat ("DO IT TO IT").

## Goal

While reading, you can turn on the camera, see yourself behind the script,
and record. Works on Android (phone/tablet) and when reading on the PC.

## What the user sees

- Reading screen header gets a **camera** button. On: the live camera fills
  the reading area behind the text. A dark tint and text shadow keep the
  words readable. A **switch camera** button appears next to it.
- Footer gets a red **REC** button while the camera is on. Recording shows
  a blinking dot and a timer (`0:42`) on the button and in the status line.
- Stop recording, leave the reading screen, or switch apps: the video is
  saved and a message says where.
- The saved video is the camera only. Script text is never burned in.
- Front camera is the default; the last choice is remembered per device.
- Voice following keeps working while recording.

## Where videos go

| Platform | Folder | Format |
|---|---|---|
| Android | Gallery › `Movies/RustyPrompter` (MediaStore, no storage permission) | MP4 (H.264 + AAC) |
| PC | `%USERPROFILE%\Videos\RustyPrompter` (macOS `~/Movies`, Linux `~/Videos`) | MP4 when the browser supports it, else WebM |

File name: `RustyPrompter_<Script title>_<YYYY-MM-DD_HH-MM-SS>.<ext>`,
title limited to safe characters.

## Architecture

Rust owns the state and buttons; platforms only run the camera.

- **`rusty-core::recording`** (pure): `safe_stem()` and `file_stem()`.
  Shared by the UI (name it) and companion (re-check it).
- **UI (`app.rs`)**: `Camera` state on `Ui` (`on`, `front`, `recording`
  start time, shown seconds). Sends `camera(command, payload)` to JS.
  Handles `{"type":"camera","state":...}` events: `open`, `closed`,
  `recording`, `saved` (`folder`), `error` (`message`). Closes the camera
  whenever no script is open.
- **`interop.js` `camera()`**: on Android forwards to
  `RustyNative.command("camera_<cmd>")`. In a browser uses
  `getUserMedia` + `MediaRecorder` (1 s chunks), draws a fixed
  `<video id="rusty-camera">` behind the page, and uploads each chunk in
  order to the companion.
- **Companion**: `POST /api/recordings/{stem}?ext=mp4|webm&part=N`, editor
  cookie + loopback + origin check (same as script saves), 32 MB per chunk.
  `part=0` creates the file (never overwrites), later parts append. Chunked
  upload means a crash keeps everything recorded so far.
- **Android (`MainActivity.kt`)**: Camera2 + `MediaRecorder` (platform
  APIs, no new libraries). `TextureView` sits behind a transparent WebView,
  center-cropped and rotated for the display. Recording uses the
  `CAMCORDER` mic, an orientation hint, ~10 Mbps 1080p. Pausing the app
  stops and saves; the camera reopens on resume if it was on.

## Errors

- Camera permission denied / no camera / busy → `error` event → message;
  camera turns off.
- Upload or encoder failure → message; recording stops; parts already saved
  stay on disk.
- Empty recording (stopped instantly) → file removed on Android.

## Known risk

Android must give the mic to the voice tracker (`VOICE_RECOGNITION`) and
the recorder (`CAMCORDER`) at once. Android 10+ allows concurrent capture
inside one app, but this is verified on the Galaxy S26 before release.

## Testing

- Core unit tests: name sanitizing.
- Companion tests: auth, bad names/extensions rejected, part 0 create, append,
  no overwrite, missing file for part > 0.
- Manual: S26 front/back, portrait/landscape, voice follow during recording,
  app switch mid-recording; PC Edge/Chrome record and playback.
