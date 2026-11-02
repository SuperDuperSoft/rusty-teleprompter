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
- **Retake** restarts the current slide as a new take and ignores speech from before it. **Sentence** steps back one sentence; **Back** goes to the previous slide. **From the top** in the Aa settings sheet restarts the slide.
- With **Hold at end of slide** on (settings), the end of a slide waits for you instead of counting down; tap Next to continue.
- Pause freezes reading or the countdown. Resume continues. Previous, Next and Restart cancel the countdown.
- The final slide displays **Finished**. A completed session reopens at the first slide.
- Drag to scroll manually. Tap a word to resume voice following there. Seeing the bottom of the screen alone never completes a slide.
- If you skip the entire ending or the recognizer cannot find it confidently, use **Next**.
- Put nonspoken directions in `[square brackets]`. They remain visible but are ignored by the voice matcher. Empty slides and slides containing only directions are skipped.
- Settings pause the read and let you change font size, margins, spacing, guide position, orientation, horizontal mirroring and vertical flipping. The countdown mirrors with the script; operator controls remain convenient to use on the tablet itself.
- Leaving the app stops the microphone and pauses the take. The screen stays awake while RustyPrompter is open.

Desktop **Preview** shows the prompting interface with manual navigation. Offline voice recognition runs in the Android app; the browser preview does not claim to use the PC microphone.

## Edits, offline use and privacy

While reading, the tablet keeps the complete current script revision stable, including following slides and countdowns. PC updates wait until Pause or completion. On applying an update, it retains the same slide and finds a unique phrase around the previous reading position; if that position was removed or ambiguous, it resets to that slide's start. If the slide was deleted, it selects the first nonempty slide.

After syncing, prompting continues with Wi-Fi off and the PC shut down. Scripts are cached privately on the tablet. Microphone audio is neither recorded nor uploaded. Local sync uses authenticated HTTP/WebSocket traffic on your private LAN; it is not encrypted, and no router port forwarding is needed or supported.

The PC's **Setup & help** can disconnect paired tablets. Cached scripts remain on the tablet, but it must pair again to receive new edits. A fresh pairing code is created at each PC launch; already paired tablets do not need the new code.

Keep **RustyPrompter-Signing-Backup.zip** privately backed up. It contains the key and password needed to install future APK upgrades without losing the application's data. Do not publish it.

## Physical-device check

Before relying on the app for a shoot, install it on the actual A7 Lite and read for **30 minutes** at your normal microphone distance. Check voice latency, skipped words/sentences, pauses, ad-libs, all four mirror combinations, both orientations, countdown pause/resume and a mid-take PC edit. These checks require the tablet; automated desktop tests do not replace them.

## Rebuild

With Python 3.11 available, run `python scripts\bootstrap.py`, `python scripts\android_tools.py`, then `python scripts\build.py` from the source folder. Tools are downloaded into the parent workspace's `.tools` directory. No global Rust install or PATH edit is needed. Restore the private signing backup into `.signing` before building an update if that folder was lost.
