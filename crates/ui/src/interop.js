// Platform plumbing only. The application UI and playback state live in Rust.
export function native_boot() {
  try { return window.RustyNative ? window.RustyNative.initialState() : "{}"; } catch { return "{}"; }
}
export function initialize() {
  window.__rustyEvents ||= [];
  if (window.__rustyInitialized) return;
  window.__rustyInitialized = true;
  window.addEventListener("beforeunload", e => { if (window.__rustyDirty) { e.preventDefault(); e.returnValue = ""; } });
  document.addEventListener("visibilitychange", () => { if (document.hidden) window.__rustyEvents.push({type:"pause"}); });
  window.addEventListener("resize", () => { const word=window.__rustyLastWord; if(window.__rustyFollowing&&word)follow_word(word.index,word.guide,true); });
  document.addEventListener("change", async e => {
    if (e.target.id !== "import-file") return;
    const file = e.target.files?.[0]; if (!file) return;
    if (file.size > 2000000) window.__rustyEvents.push({type:"error",message:"Import a file smaller than 2 MB."});
    else window.__rustyEvents.push({type:"import",name:file.name,text:await file.text()});
    e.target.value = "";
  });
  const key = down => e => {
    const r = window.__rustyRemote, t = e.target;
    if (!r?.prompting || typeof e.key !== "string" || e.ctrlKey || e.altKey || e.metaKey || e.isComposing) return;
    if (t instanceof Element && (t.matches("input,textarea,select") || t.isContentEditable || t.closest("[role=dialog],.sheet,.sheet-backdrop"))) return;
    const name = e.key === "Spacebar" || e.key === "Space" ? " " : e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (!r.keys.has(name)) return;
    e.preventDefault();
    window.__rustyEvents.push({type:"key",key:e.key,down,repeat:e.repeat});
  };
  document.addEventListener("keydown", key(true), true);
  document.addEventListener("keyup", key(false), true);
  window.addEventListener("blur", () => window.__rustyEvents.push({type:"key_reset"}));
}
export function set_remote(prompting, keys_json) { window.__rustyRemote = {prompting, keys: new Set(JSON.parse(keys_json))}; }
export function drain_events() { const values = window.__rustyEvents || []; window.__rustyEvents = []; return JSON.stringify(values); }
export function native_call(command, payload) {
  if (window.RustyNative) window.RustyNative.command(command, payload);
  else if (voice?.readyState === 1 && VOICE_COMMANDS.has(command)) voice.send(JSON.stringify({command, payload}));
}
// On the PC the companion runs the offline voice model; this socket is its microphone bridge.
const VOICE_COMMANDS = new Set(["start", "stop", "reset", "grammar"]);
let voice;
export function desktop_voice() {
  if (window.RustyNative || voice) return;
  const open = () => {
    voice = new WebSocket(`ws://${location.host}/api/voice`);
    voice.onmessage = e => { try { window.__rustyEvents.push(JSON.parse(e.data)); } catch {} };
    voice.onclose = () => {
      window.__rustyEvents.push({type:"voice",ready:false,message:"Voice engine offline · keep RustyPrompter open on this PC"});
      setTimeout(open, 3000);
    };
  };
  open();
}
// Keeps the PC screen on while reading, like the tablet does.
let wakeLock = null, wakePending = false;
export function keep_awake(on) {
  if (!on) { wakeLock?.release().catch(() => {}); wakeLock = null; return; }
  // Called every frame while reading; re-acquires after the tab was hidden.
  if (wakeLock || wakePending || !navigator.wakeLock || document.hidden) return;
  wakePending = true;
  navigator.wakeLock.request("screen").then(lock => { wakeLock = lock; lock.addEventListener("release", () => { if (wakeLock === lock) wakeLock = null; }); }).catch(() => {}).finally(() => { wakePending = false; });
}
export function toggle_fullscreen() {
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  else document.documentElement.requestFullscreen?.().catch(() => {});
}
export function now() { return performance.now(); }
export function local_get(key) { try { return localStorage.getItem(key) || ""; } catch { return ""; } }
export function local_set(key, value) { try { if (value) localStorage.setItem(key,value); else localStorage.removeItem(key); } catch {} }
export function set_dirty(value) { window.__rustyDirty = value; }
export function viewport_width() { return window.innerWidth || 0; }
export function reveal_active_chip() {
  requestAnimationFrame(() => document.querySelector(".slide-chip.active")?.scrollIntoView({block: "nearest", inline: "nearest"}));
}
export function set_following(value) { window.__rustyFollowing = value; }
export function uuid() { return crypto.randomUUID ? crypto.randomUUID() : Date.now().toString(16) + "-" + Math.random().toString(16).slice(2); }
export function request_import() { document.getElementById("import-file")?.click(); }
export function download_file(name, text) {
  const a = document.createElement("a"); const url = URL.createObjectURL(new Blob([text],{type:"text/plain;charset=utf-8"}));
  a.href = url; a.download = name; a.click(); setTimeout(()=>URL.revokeObjectURL(url),1000);
}
export function follow_word(index, guide, force) {
  window.__rustyLastWord={index,guide};
  requestAnimationFrame(() => {
    const viewport = document.getElementById("script-viewport"), word = document.getElementById("word-"+index);
    if (!viewport || !word) return;
    viewport.firstElementChild.style.paddingTop=viewport.clientHeight*guide/100+'px';
    viewport.scrollTo({top:Math.max(0,word.offsetTop - viewport.clientHeight*guide/100),behavior:force?"instant":"smooth"});
  });
}
export function local_stamp() {
  const d = new Date(), p = n => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth()+1)}-${p(d.getDate())}_${p(d.getHours())}-${p(d.getMinutes())}-${p(d.getSeconds())}`;
}
// Camera. Rust decides when; this runs the camera. Results come back as
// {type:"camera", state:"open"|"closed"|"recording"|"saved"|"error", ...}.
// Android runs it natively (Camera2) behind the transparent WebView; a PC
// browser uses getUserMedia + MediaRecorder and uploads 1 s chunks to the
// companion, which writes them to the Videos folder as they arrive.
const camEvent = value => window.__rustyEvents.push({type:"camera", ...value});
let cam = null; // {stream, video, front, rec}
export function camera(command, payload) {
  const value = (() => { try { return JSON.parse(payload || "{}"); } catch { return {}; } })();
  if (window.RustyNative) { window.RustyNative.command("camera_" + command, JSON.stringify(value)); return; }
  ({open: camOpen, close: camClose, record: camRecord, stop: camStop}[command] || (() => {}))(value);
}
async function camOpen({front = true}) {
  if (cam?.rec) await camStop();
  camRelease();
  if (!navigator.mediaDevices?.getUserMedia) { camEvent({state:"error", message:"This browser can't use a camera. Try Edge or Chrome."}); return; }
  const want = cam = {front, stream:null, video:null, rec:null};
  try {
    const stream = await navigator.mediaDevices.getUserMedia({
      video: {facingMode: front ? "user" : "environment", width: {ideal: 1920}, height: {ideal: 1080}, frameRate: {ideal: 30}},
      // Raw voice sounds better in a video than call-style processing.
      audio: {echoCancellation: false, noiseSuppression: false, autoGainControl: false},
    });
    if (cam !== want) { stream.getTracks().forEach(t => t.stop()); return; }
    let video = document.getElementById("rusty-camera");
    if (!video) { video = document.createElement("video"); video.id = "rusty-camera"; video.muted = true; video.playsInline = true; video.autoplay = true; document.body.prepend(video); }
    video.classList.toggle("mirror", front);
    video.srcObject = stream;
    video.play?.().catch(() => {});
    Object.assign(want, {stream, video});
    camEvent({state:"open", front});
  } catch (e) {
    if (cam !== want) return; // a newer open or a close replaced this one
    cam = null;
    const denied = e?.name === "NotAllowedError" || e?.name === "SecurityError";
    camEvent({state:"error", message: denied ? "Allow camera and microphone access in the browser to record." : e?.name === "NotFoundError" ? "No camera found on this PC." : "Could not open the camera. Close other apps using it and try again."});
  }
}
function camRelease(target = cam) {
  target?.stream?.getTracks().forEach(t => t.stop());
  if (target?.video) { target.video.srcObject = null; target.video.remove(); }
  if (cam === target) cam = null;
}
async function camClose() {
  const mine = cam;
  if (mine?.rec) await camStop();
  camRelease(mine); // never the camera a newer open started meanwhile
  camEvent({state:"closed"});
}
function camRecord({name = "RustyPrompter"}) {
  if (!cam?.stream || cam.rec) { camEvent({state:"stopped"}); return; }
  const types = ["video/mp4;codecs=avc1.640028,mp4a.40.2", "video/mp4;codecs=avc1,mp4a.40.2", "video/mp4", "video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"];
  const mimeType = types.find(t => window.MediaRecorder?.isTypeSupported?.(t));
  if (!mimeType) { camEvent({state:"error", message:"This browser can't record video. Try Edge or Chrome."}); return; }
  const ext = mimeType.startsWith("video/mp4") ? "mp4" : "webm";
  let recorder;
  try { recorder = new MediaRecorder(cam.stream, {mimeType, videoBitsPerSecond: 8000000, audioBitsPerSecond: 192000}); }
  catch { camEvent({state:"error", message:"Could not start recording."}); return; }
  const rec = cam.rec = {recorder, part: 0, chain: Promise.resolve(), failed: false, folder: "", done: null, name};
  const url = part => `/api/recordings/${encodeURIComponent(rec.name)}?ext=${ext}&part=${part}`;
  recorder.ondataavailable = e => {
    if (!e.data?.size || rec.failed) return;
    const part = rec.part++;
    rec.chain = rec.chain.then(async () => {
      if (rec.failed) return;
      try {
        const send = () => fetch(url(part), {method: "POST", body: e.data, headers: {"Content-Type": "application/octet-stream"}});
        let r = await send();
        // Same name taken (two takes in one second): try Name_2, Name_3, ...
        for (let n = 2; r.status === 409 && part === 0 && n < 10; n++) { rec.name = `${name}_${n}`; r = await send(); }
        const body = await r.json().catch(() => ({}));
        if (!r.ok) { const err = new Error(body.detail || "Could not save the recording."); err.server = true; throw err; }
        rec.folder = body.folder || rec.folder;
      } catch (err) {
        rec.failed = true;
        if (recorder.state !== "inactive") recorder.stop();
        camEvent({state:"error", message: (err?.server ? err.message : "Lost the connection to RustyPrompter on this PC.") + (part > 0 ? " What was recorded before this is kept." : "")});
      }
    });
  };
  rec.done = new Promise(resolve => {
    recorder.onstop = () => setTimeout(async () => {
      await rec.chain;
      if (cam?.rec === rec) cam.rec = null;
      if (!rec.failed && rec.part > 0) camEvent({state:"saved", folder: rec.folder, file: `${rec.name}.${ext}`});
      else if (!rec.failed) camEvent({state:"stopped"});
      resolve();
    }, 0);
  });
  recorder.onerror = () => { if (recorder.state !== "inactive") recorder.stop(); };
  try { recorder.start(1000); }
  catch { cam.rec = null; camEvent({state:"error", message:"Could not start recording. Is the camera still connected?"}); return; }
  camEvent({state:"recording"});
}
async function camStop() {
  const rec = cam?.rec;
  if (!rec) return;
  if (rec.recorder.state !== "inactive") rec.recorder.stop();
  // onstop always follows a started recorder; the timeout only guards a broken one.
  await Promise.race([rec.done, new Promise(r => setTimeout(r, 15000))]);
  if (cam?.rec === rec) cam.rec = null;
}
