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
