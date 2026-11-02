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
}
export function drain_events() { const values = window.__rustyEvents || []; window.__rustyEvents = []; return JSON.stringify(values); }
export function native_call(command, payload) { if (window.RustyNative) window.RustyNative.command(command, payload); }
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
