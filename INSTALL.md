# Install RustyPrompter

RustyPrompter is a new app. PromptLocal remains installed and its files/data are untouched. RustyPrompter does not need a subscription, account, paid speech API or hosting service.

## Windows

1. Extract **RustyPrompter-Windows.zip**. Keep the executable, `web` folder and other extracted files together.
2. Open **RustyPrompter.exe**. Its local script studio opens in your browser and a small companion status window stays open.
3. If Windows Firewall asks, allow **Private networks**. Keep the status window open while syncing.
4. Click **New script** or **Try the example**. Type the words for each slide in its separate editor. Add, duplicate, rename, reorder and delete slides as needed.

Scripts autosave. **Backup** exports an editable JSON script with its slide structure. **Export text** creates a plain-text copy. **Import** accepts a JSON backup or adds a `.txt` file as one slide. The built-in example is a useful voice-following check.

Your PC database lives at `%LOCALAPPDATA%\RustyPrompter\rustyprompter.redb`. Close the companion before copying this folder for a complete backup. Use JSON Backup for individual scripts. PromptLocal's database is not automatically imported.

## Galaxy Tab A7 Lite

1. Copy **RustyPrompter-Android.apk** onto the tablet using USB, OneDrive or your normal transfer method.
2. Open it in Samsung My Files and allow installation of this APK when Android asks. The application is locally signed, not distributed through Google Play.
3. Open **RustyPrompter**. Its example script is available before pairing. The first launch prepares the bundled offline English speech model.
4. Tap **Connect to PC** and enter the address and six-digit code shown in the PC studio. Both devices need the same private Wi-Fi/LAN. The port is **8766**, different from PromptLocal's port.
5. Open a script, tap **Start**, and grant microphone permission. Voice following runs entirely on the tablet.

Minimum Android version is **Android 11**. The app uses the tablet's Android System WebView to display its bundled Dioxus/Rust interface, so keep that system component updated through your normal Android updates. It does not load the interface from the PC or internet.

If the PC shows multiple addresses, choose its Wi-Fi/LAN address. Guest Wi-Fi isolation, VPNs and Windows Firewall can prevent pairing. If the PC's address changes, connect again using the new address. Pairing tokens otherwise survive restarts.

## Reading

- Read naturally. The matcher tolerates skipped words, fillers, short ad-libs and some recognition errors. Silence does not advance a slide.
- The end of the **spoken text** starts a countdown (**5–4–3–2–1** by default; 1–15 seconds in settings), then the next nonempty slide opens and begins listening automatically.
- **Retake** restarts the current slide as a new take and ignores speech from before it. **Sentence** steps back one sentence; **Back** goes to the previous slide. **From the top** in the Aa settings sheet restarts the script from the first slide.
- With **Hold at end of slide** on (settings), the end of a slide waits for you instead of counting down; tap Next to continue.
- Pause freezes reading or the countdown. Resume continues. Retake, Sentence, Back and Next cancel the countdown.
- The final slide displays **Finished**. A completed session reopens at the first slide.
- Drag to scroll manually. Tap a word to resume voice following there. Seeing the bottom of the screen alone never completes a slide.
- If you skip the entire ending or the recognizer cannot find it confidently, use **Next**.
- Put nonspoken directions in `[square brackets]`. They remain visible but are ignored by the voice matcher. Empty slides and slides containing only directions are skipped.
- Settings pause the read and let you change font size, margins, spacing, guide position, orientation, horizontal mirroring and vertical flipping. The countdown mirrors with the script; operator controls remain convenient to use on the tablet itself.
- Leaving the app stops the microphone and pauses the take. The screen stays awake while RustyPrompter is open.
- Numbers can be spoken naturally: "John three sixteen" for John 3:16, "five dollars" for $5, "twenty first" for 21st. If you say a number differently from how it is written (for example "two thousand twenty four" for 2024), type the words in the script.
- The dot by the title lights when the app hears new speech. If you keep speaking without the highlight moving (5 s by default), a **Lost your place?** pill shows the next words, with **Sentence back** and **Dismiss**.
- The prompter header shows time read and an estimate of time left. Studio shows an estimate per slide and for the whole script; the tablet home list shows minutes per script.

### Pre-roll

With **Pre-roll** above 0 in the Aa settings sheet, Start or Resume (and Retake or Sentence during a take, and starting again after Finished) shows a 3-2-1 countdown before listening begins. It ends in a short white **sync flash** (switch off with **Sync flash**) that a separate camera can use to line up audio. **Start now** skips the rest of the countdown; **Cancel** pauses. Pausing during a pre-roll and resuming plays the full pre-roll again. Slide changes never pre-roll.

The microphone opens when the pre-roll starts, so the very first time you use voice following, Android's microphone permission prompt appears during the countdown. The prompt pauses the take; once you allow it, the countdown starts over by itself. Speech during the countdown is ignored.

### Remote keys (clicker, pedal, keyboard)

Pair a Bluetooth clicker or pedal that acts as a keyboard with the tablet (or use a PC keyboard in the browser Preview). Keys work only while the prompter is shown and no settings sheet, help or dialog is open.

| Key | Press | Hold (about 1 s) |
|---|---|---|
| Page Down, Right arrow, Down arrow, Next track | Start, or next slide | Pause / resume |
| Page Up, Left arrow, Up arrow, Previous track | Back one sentence | Retake the slide |
| Space, Enter, Play/Pause | Start or pause | — |
| Play | Start or resume | — |
| Pause, B, period (a clicker's blank-screen button) | Pause | — |
| Volume up / down (only with **Volume keys as remote** on, tablet only) | Like Page Down / Page Up | Like Page Down / Page Up |

A quick repeat of the same action (within 0.4 s of the last one) is ignored, so a bouncy clicker cannot skip slides, and a second Next right after Start does not cancel the pre-roll. After Finished, Next starts the script again from the first slide. Media keys are reliable on the tablet only; desktop browsers usually keep them for the operating system. A large toast confirms each remote action.

### Reading settings

The Aa settings sheet also has: **Pre-roll (s)** (0 = off, up to 10), **Sync flash**, **Speaking pace (wpm)** for estimates (80–240, default 150), **Lost-place hint** and **Hint after (s)** (2–15, default 5), **Follow re-reading** (on by default; moves the highlight back when you re-read earlier text), and on the tablet **Volume keys as remote** (off by default) and **Script vocabulary (beta)** (off by default). Script vocabulary limits recognition to the script's words; it restarts the microphone when switched or when the script changes, and falls back to the full vocabulary with a message for very large scripts or if the model cannot build it. Older saved settings load with these defaults.

### After upgrading from the previous release

Slides containing numbers are now split into words differently, so the reading position saved by the previous release is not restored: the script you were last reading opens at its first slide instead of where you stopped. This happens only once, on the first open after the upgrade.

Desktop **Preview** shows the prompting interface with manual navigation. Offline voice recognition runs in the Android app; the browser preview does not claim to use the PC microphone.

## Edits, offline use and privacy

While reading, the tablet keeps the complete current script revision stable, including following slides and countdowns. PC updates wait until Pause or completion. On applying an update, it retains the same slide and finds a unique phrase around the previous reading position; if that position was removed or ambiguous, it resets to that slide's start. If the slide was deleted, it selects the first nonempty slide.

After syncing, prompting continues with Wi-Fi off and the PC shut down. Scripts are cached privately on the tablet. Microphone audio is neither recorded nor uploaded. Local sync uses authenticated HTTP/WebSocket traffic on your private LAN; it is not encrypted, and no router port forwarding is needed or supported.

The PC's **Setup & help** can disconnect paired tablets. Cached scripts remain on the tablet, but it must pair again to receive new edits. A fresh pairing code is created at each PC launch; already paired tablets do not need the new code.

Keep **RustyPrompter-Signing-Backup.zip** privately backed up. It contains the key and password needed to install future APK upgrades without losing the application's data. Do not publish it.

## Physical-device check

Before relying on the app for a shoot, install it on the actual A7 Lite and read for **30 minutes** at your normal microphone distance. Check voice latency, skipped words/sentences, pauses, ad-libs, all four mirror combinations, both orientations, countdown pause/resume and a mid-take PC edit. If you use them, also check your clicker or pedal (press and hold), the pre-roll and sync flash on your camera, and Script vocabulary. These checks require the tablet; automated desktop tests do not replace them.

## Rebuild

With Python 3.11 available, run `python scripts\bootstrap.py`, `python scripts\android_tools.py`, then `python scripts\build.py` from the source folder. Tools are downloaded into the parent workspace's `.tools` directory. No global Rust install or PATH edit is needed. Restore the private signing backup into `.signing` before building an update if that folder was lost.
