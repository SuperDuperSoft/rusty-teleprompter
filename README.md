# Rusty Teleprompter

**A teleprompter that listens to you. Built in Rust. Free forever.**

Write your script on your PC, read it on an Android tablet, and the text follows your voice as you speak. No subscription, no account, no cloud. Everything runs on your own devices, even with the internet off.

> The app installs as **RustyPrompter**.

---

## Why it's different

- 🆓 **Free forever.** No subscription, no trial, no ads, no paid speech service. Open source under the Apache 2.0 license: use it, change it, share it.
- 🦀 **Built in Rust.** The voice tracking, the reading logic and the app itself are written in Rust. That makes it fast, predictable and hard to crash.
- 🔒 **Private.** Speech recognition runs on your own device, tablet or PC. Your words and your voice never leave your devices.
- 📴 **Works offline.** Once your script is on the tablet, you don't need Wi-Fi or your PC.

## What it does

**Follows your voice**
- Highlights the word you are saying, not the one you just said. A microphone always hears late, so the highlight runs ahead of it at your own pace, and the next two words are lifted so your eye can run ahead.
- Skip a word, say "um", or ad-lib a little, and it keeps up. Long slides that repeat a sentence don't make it leap.
- If you go back and re-read a sentence, the highlight goes back with you.
- Reads numbers the way you say them: "John 3:16" → "John three sixteen", "$5" → "five dollars", "21st" → "twenty first".
- A slide only finishes when you actually say its last words.

**Made for recording video**
- **Record yourself** on the phone, tablet or PC: tap 🎥 and the camera shows behind your script, then tap **REC**. The video is just you; the words stay on your screen. Saved to the gallery (Movies/RustyPrompter) on Android, or to Videos\RustyPrompter on the PC.
- **Retake** restarts the slide in one tap. **Sentence** steps back one sentence.
- Optional **3-2-1 countdown** before each take, ending in a white **flash** your camera can use to line up audio.
- At the end of a slide, either count down to the next one (1–15 seconds) or **hold** until you're ready.
- **Bluetooth clicker or foot pedal** support, so you can control everything from where you're standing.

**Helps you stay on track**
- A small dot lights up when the app hears you.
- Optional: if you lose your place, a popup shows the next words (off by default; turn on "Lost-place popup" in Aa settings).
- Shows how long each slide and the whole script will take, plus time left while you read.

**Fits your setup**
- Mirror the text horizontally, vertically or both (for teleprompter glass).
- Adjust text size, margins, line spacing, reading guide and orientation.
- Edit on your PC while the tablet reads. Changes wait until you pause, so a take is never disturbed.
- Edit scripts and slides on the tablet too. Edits save on the tablet right away and sync to the PC when it's connected. If the same script changed on both, the tablet's version is kept as a copy.
- No tablet? **Read on this PC** follows your voice with the PC's microphone, with all the same reading features.
- Import and export plain text or JSON backups.

## How it works

1. **Write on your PC.** Run the Windows app. It opens a script editor in your browser. Each script is a list of slides, and everything saves automatically.
2. **Connect your tablet.** On the same Wi-Fi, tap **Connect to PC** and enter the six-digit code from the PC.
3. **Read.** Open a script, tap **Start**, and talk. That's it.

Full setup steps, controls and settings: **[INSTALL.md](INSTALL.md)**

## Remote control

Any Bluetooth clicker or pedal that acts like a keyboard works.

| Button | Tap | Hold (about 1 second) |
|---|---|---|
| Page Down / → | Start, or next slide | Pause |
| Page Up / ← | Back one sentence | Retake the slide |
| Space / Enter | Start or pause | — |

More keys, including media keys and the optional volume-key remote, are listed in [INSTALL.md](INSTALL.md#remote-keys-clicker-pedal-keyboard).

## Requirements

- **PC:** Windows 10 or 11.
- **Tablet:** Android 11 or newer. Tested target: Samsung Galaxy Tab A7 Lite.
- Both on the same private Wi-Fi network when syncing scripts.

## Building it yourself

The whole project builds from source. You need Python 3.11 to run the build scripts. They download the Rust, Android and linker toolchains into a local `.tools` folder and don't change your system PATH.

```powershell
python scripts\bootstrap.py
python scripts\android_tools.py
python scripts\build.py
```

The Windows zip and Android APK appear in `dist/`.

Run the tests:

```powershell
python scripts\task.py cargo test -p rusty-core -p rusty-companion -p rusty-ui
```

## What's inside

| Folder | What it is |
|---|---|
| `crates/core` | The Rust engine: voice matching, number reading, reading and countdown logic. Fully unit-tested. |
| `crates/ui` | The app's interface, written in Rust with [Dioxus](https://dioxuslabs.com) and compiled to WebAssembly. The same interface runs on PC and tablet. |
| `crates/companion` | The Windows app, in Rust: stores scripts and syncs them to the tablet. |
| `android` | A thin Android wrapper. It shows the Rust interface and handles the microphone with [Vosk](https://alphacephei.com/vosk/) offline speech recognition. |

All decisions about matching, timing and playback live in Rust. The Android code only passes along microphone and button events.

## Privacy and security

- Speech recognition never uses a cloud service.
- The script editor only accepts connections from the PC itself. Paired tablets can read and edit scripts. A tablet edit is rejected if the PC has a newer version, so nothing is overwritten.
- Sync traffic is meant for your private home or church network. It isn't encrypted, so don't expose port 8766 to the internet.

## Status

Automated tests cover the engine, the PC server and the interface ([VALIDATION.md](VALIDATION.md)). Long real-world reading sessions on the tablet are still being tested.

## License

[Apache License 2.0](LICENSE). Free to use, change and share, forever.

Building something on top of Rusty Teleprompter? Go for it, even commercially. Just give credit: if you share or sell your version, the license requires you to include the [NOTICE](NOTICE) file, which says your app is based on Rusty Teleprompter.

Third-party components are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
