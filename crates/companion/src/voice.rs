//! Offline voice for reading on this PC: Vosk (libvosk.dll) and the default
//! microphone. Like the Android host it only turns audio into speech events;
//! matching, timing and playback stay in the Rust UI.
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
enum Status {
    Idle,
    Loading,
    Ready(Arc<Model>),
    Failed(String),
}
pub struct Voice {
    folder: PathBuf,
    status: Mutex<Status>,
}
impl Voice {
    /// `folder` holds libvosk.dll, its runtime DLLs and `model-en-us`.
    pub fn new(folder: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            folder,
            status: Mutex::new(Status::Idle),
        })
    }
    /// Loads the model in the background the first time a reader connects.
    pub fn ensure_loaded(self: &Arc<Self>) {
        {
            let mut status = self.status.lock().unwrap();
            if !matches!(*status, Status::Idle) {
                return;
            }
            *status = Status::Loading;
        }
        let voice = self.clone();
        std::thread::spawn(move || {
            let loaded = Model::load(&voice.folder);
            *voice.status.lock().unwrap() = match loaded {
                Ok(model) => Status::Ready(Arc::new(model)),
                Err(message) => Status::Failed(message),
            };
        });
    }
    /// The `voice` event the UI understands.
    pub fn status_event(&self) -> Value {
        match &*self.status.lock().unwrap() {
            Status::Idle | Status::Loading => {
                json!({"type":"voice","ready":false,"message":"Preparing offline voice model…"})
            }
            Status::Ready(_) => {
                json!({"type":"voice","ready":true,"message":"Offline voice model ready"})
            }
            Status::Failed(message) => json!({"type":"voice","ready":false,"message":message}),
        }
    }
    /// Opens the microphone and streams `speech` events tagged with `take`.
    pub fn listen(
        &self,
        grammar: Option<String>,
        take: Arc<AtomicU64>,
        events: UnboundedSender<Value>,
    ) -> Option<Listener> {
        let Status::Ready(model) = self.status.lock().unwrap().clone() else {
            return None;
        };
        let stop = Arc::new(AtomicBool::new(false));
        let reset = Arc::new(AtomicBool::new(false));
        let flags = (stop.clone(), reset.clone());
        let thread = std::thread::spawn(move || {
            listen(&model, grammar, &take, &events, &flags.0, &flags.1)
        });
        Some(Listener {
            stop,
            reset,
            thread: Some(thread),
        })
    }
}
/// Transcribes 16 kHz mono samples; backs `--voice-test` in packaging checks.
pub fn transcribe(folder: &std::path::Path, samples: &[i16]) -> Result<String, String> {
    let model = Model::load(folder)?;
    let recognizer = model.recognizer(None).ok_or("The offline voice model could not start.")?;
    let mut text = Vec::new();
    for chunk in samples.chunks(1600) {
        if recognizer.accept(chunk) {
            text.push(serde_json::from_str::<Value>(&recognizer.result()).unwrap_or_default()["text"].as_str().unwrap_or("").to_string());
        }
    }
    let last = unsafe { CStr::from_ptr((recognizer.api.final_result)(recognizer.raw)).to_string_lossy().into_owned() };
    text.push(serde_json::from_str::<Value>(&last).unwrap_or_default()["text"].as_str().unwrap_or("").to_string());
    Ok(text.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" "))
}
/// A running microphone. Dropping it closes the microphone.
pub struct Listener {
    stop: Arc<AtomicBool>,
    reset: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Listener {
    /// Drops speech in flight, e.g. the "3-2-1" said during a pre-roll.
    pub fn reset(&self) {
        self.reset.store(true, Ordering::SeqCst);
    }
    /// False once the microphone failed for good (an `audio_error` was sent).
    pub fn alive(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The capture loop polls every 10 ms, so joining is quick.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
/// Restarts a failed microphone with backoff (500/1000/2000 ms), as on Android.
fn listen(
    model: &Model,
    grammar: Option<String>,
    take: &AtomicU64,
    events: &UnboundedSender<Value>,
    stop: &AtomicBool,
    reset: &AtomicBool,
) {
    let recognizer = match grammar.as_deref().filter(|_| model.grammar).and_then(|g| model.recognizer(Some(g))) {
        Some(r) => r,
        None => {
            if grammar.is_some() {
                let _ = events.send(json!({"type":"grammar_error","message":"Script vocabulary is not supported by this voice model. Using full vocabulary."}));
            }
            match model.recognizer(None) {
                Some(r) => r,
                None => {
                    let _ = events.send(json!({"type":"audio_error","message":"The offline voice model could not start."}));
                    return;
                }
            }
        }
    };
    let mut retries = 0;
    loop {
        match capture(&recognizer, take, events, stop, reset, &mut retries) {
            Ok(()) => return,
            Err(message) => {
                if retries >= 3 || stop.load(Ordering::SeqCst) {
                    let _ = events.send(json!({"type":"audio_error","message":message}));
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis([500, 1000, 2000][retries]));
                retries += 1;
                recognizer.reset();
            }
        }
    }
}
/// Sends a recognizer result as a `speech` event when it has words.
fn emit(json_text: &str, is_final: bool, take: &AtomicU64, events: &UnboundedSender<Value>, last_partial: &mut String) -> bool {
    let value: Value = serde_json::from_str(json_text).unwrap_or_default();
    let text = value[if is_final { "text" } else { "partial" }].as_str().unwrap_or("");
    if is_final {
        last_partial.clear();
    } else if text == last_partial {
        return false;
    } else {
        *last_partial = text.to_string();
    }
    if text.trim().is_empty() {
        return false;
    }
    let _ = events.send(json!({"type":"speech","text":text,"final":is_final,"take":take.load(Ordering::SeqCst)}));
    true
}

#[cfg(windows)]
fn capture(
    recognizer: &Recognizer,
    take: &AtomicU64,
    events: &UnboundedSender<Value>,
    stop: &AtomicBool,
    reset: &AtomicBool,
    retries: &mut usize,
) -> Result<(), String> {
    use windows_sys::Win32::Media::Audio::*;
    const SAMPLES: usize = 1600; // 100 ms at 16 kHz
    const BUFFERS: usize = 4;
    const NO_MIC: &str = "No microphone found. Plug one in, and allow desktop apps to use the microphone in Windows Settings › Privacy & security › Microphone.";
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 1,
        nSamplesPerSec: 16000,
        nAvgBytesPerSec: 32000,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    let mut handle: HWAVEIN = std::ptr::null_mut();
    // WAVE_MAPPER converts the default microphone to 16 kHz mono for Vosk.
    if unsafe { waveInOpen(&mut handle, WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL) } != 0 {
        return Err(NO_MIC.into());
    }
    let size = std::mem::size_of::<WAVEHDR>() as u32;
    let mut data: Vec<Box<[i16; SAMPLES]>> = (0..BUFFERS).map(|_| Box::new([0; SAMPLES])).collect();
    let mut headers: Vec<Box<WAVEHDR>> = data
        .iter_mut()
        .map(|d| {
            Box::new(WAVEHDR {
                lpData: d.as_mut_ptr() as *mut u8,
                dwBufferLength: (SAMPLES * 2) as u32,
                ..Default::default()
            })
        })
        .collect();
    let mut result = Ok(());
    unsafe {
        for h in headers.iter_mut() {
            let h: *mut WAVEHDR = &mut **h;
            if waveInPrepareHeader(handle, h, size) != 0 || waveInAddBuffer(handle, h, size) != 0 {
                result = Err(NO_MIC.into());
            }
        }
        if result.is_ok() && waveInStart(handle) != 0 {
            result = Err(NO_MIC.into());
        }
        let mut last_partial = String::new();
        let mut next = 0;
        while result.is_ok() && !stop.load(Ordering::SeqCst) {
            if reset.swap(false, Ordering::SeqCst) {
                recognizer.reset();
                last_partial.clear();
            }
            let h: *mut WAVEHDR = &mut *headers[next];
            // The driver sets WHDR_DONE; buffers complete in queue order.
            let flags = std::ptr::read_unaligned(std::ptr::addr_of!((*h).dwFlags));
            if flags & WHDR_DONE == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            let bytes = std::ptr::read_unaligned(std::ptr::addr_of!((*h).dwBytesRecorded)) as usize;
            let samples = &data[next][..(bytes / 2).min(SAMPLES)];
            if !samples.is_empty() {
                let heard = if recognizer.accept(samples) {
                    emit(&recognizer.result(), true, take, events, &mut last_partial)
                } else {
                    emit(&recognizer.partial(), false, take, events, &mut last_partial)
                };
                if heard {
                    *retries = 0;
                }
            }
            if waveInAddBuffer(handle, h, size) != 0 {
                result = Err("The microphone stopped. Check that it is still plugged in.".into());
            }
            next = (next + 1) % BUFFERS;
        }
        waveInReset(handle);
        for h in headers.iter_mut() {
            waveInUnprepareHeader(handle, &mut **h, size);
        }
        waveInClose(handle);
    }
    result
}
#[cfg(not(windows))]
fn capture(
    _: &Recognizer,
    _: &AtomicU64,
    _: &UnboundedSender<Value>,
    _: &AtomicBool,
    _: &AtomicBool,
    _: &mut usize,
) -> Result<(), String> {
    Err("Reading on this computer needs Windows.".into())
}

use std::ffi::{c_char, c_float, c_int, c_short, c_void, CStr, CString};
/// The handful of Vosk C API calls we use, loaded at runtime so the
/// companion still starts (and syncs) when the voice files are missing.
struct Api {
    _library: libloading::Library,
    model_new: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    model_free: unsafe extern "C" fn(*mut c_void),
    recognizer_new: unsafe extern "C" fn(*mut c_void, c_float) -> *mut c_void,
    recognizer_new_grm: unsafe extern "C" fn(*mut c_void, c_float, *const c_char) -> *mut c_void,
    recognizer_free: unsafe extern "C" fn(*mut c_void),
    accept: unsafe extern "C" fn(*mut c_void, *const c_short, c_int) -> c_int,
    result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    partial: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    final_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    reset: unsafe extern "C" fn(*mut c_void),
}
impl Api {
    fn load(path: &std::path::Path) -> Result<Self, libloading::Error> {
        unsafe {
            // Resolve libvosk's own DLLs (libstdc++ etc.) from its folder.
            #[cfg(windows)]
            let library: libloading::Library = libloading::os::windows::Library::load_with_flags(
                path,
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )?
            .into();
            #[cfg(not(windows))]
            let library = libloading::Library::new(path)?;
            if let Ok(level) = library.get::<unsafe extern "C" fn(c_int)>(b"vosk_set_log_level\0") {
                level(-1);
            }
            Ok(Self {
                model_new: *library.get(b"vosk_model_new\0")?,
                model_free: *library.get(b"vosk_model_free\0")?,
                recognizer_new: *library.get(b"vosk_recognizer_new\0")?,
                recognizer_new_grm: *library.get(b"vosk_recognizer_new_grm\0")?,
                recognizer_free: *library.get(b"vosk_recognizer_free\0")?,
                accept: *library.get(b"vosk_recognizer_accept_waveform_s\0")?,
                result: *library.get(b"vosk_recognizer_result\0")?,
                partial: *library.get(b"vosk_recognizer_partial_result\0")?,
                final_result: *library.get(b"vosk_recognizer_final_result\0")?,
                reset: *library.get(b"vosk_recognizer_reset\0")?,
                _library: library,
            })
        }
    }
}
struct Model {
    api: Arc<Api>,
    model: *mut c_void,
    /// The model has the runtime graphs that script vocabulary needs.
    grammar: bool,
}
// Vosk models are reference counted and safe to share between recognizers.
unsafe impl Send for Model {}
unsafe impl Sync for Model {}
impl Model {
    fn load(folder: &std::path::Path) -> Result<Self, String> {
        let missing = "Offline voice files are missing. Extract all files from the Windows ZIP together, including the voice folder.";
        let dll = if cfg!(windows) { "libvosk.dll" } else { "libvosk.so" };
        let model_folder = folder.join("model-en-us");
        if !folder.join(dll).exists() || !model_folder.join("am/final.mdl").exists() {
            return Err(missing.into());
        }
        let api = Api::load(&folder.join(dll)).map_err(|_| missing.to_string())?;
        let path = CString::new(model_folder.to_string_lossy().as_bytes()).map_err(|_| missing.to_string())?;
        let model = unsafe { (api.model_new)(path.as_ptr()) };
        if model.is_null() {
            return Err("Offline voice model could not load. Close other apps and reopen RustyPrompter.".into());
        }
        let graph = model_folder.join("graph");
        Ok(Self {
            api: Arc::new(api),
            model,
            grammar: graph.join("Gr.fst").exists() && graph.join("HCLr.fst").exists(),
        })
    }
    fn recognizer(&self, grammar: Option<&str>) -> Option<Recognizer> {
        let raw = unsafe {
            match grammar {
                Some(words) => {
                    let words = CString::new(words).ok()?;
                    (self.api.recognizer_new_grm)(self.model, 16000.0, words.as_ptr())
                }
                None => (self.api.recognizer_new)(self.model, 16000.0),
            }
        };
        (!raw.is_null()).then(|| Recognizer {
            api: self.api.clone(),
            raw,
        })
    }
}
impl Drop for Model {
    fn drop(&mut self) {
        unsafe { (self.api.model_free)(self.model) }
    }
}
struct Recognizer {
    api: Arc<Api>,
    raw: *mut c_void,
}
impl Recognizer {
    /// True when an utterance ended and `result` has its final text.
    fn accept(&self, samples: &[i16]) -> bool {
        unsafe { (self.api.accept)(self.raw, samples.as_ptr(), samples.len() as c_int) == 1 }
    }
    fn result(&self) -> String {
        unsafe { CStr::from_ptr((self.api.result)(self.raw)).to_string_lossy().into_owned() }
    }
    fn partial(&self) -> String {
        unsafe { CStr::from_ptr((self.api.partial)(self.raw)).to_string_lossy().into_owned() }
    }
    fn reset(&self) {
        unsafe { (self.api.reset)(self.raw) }
    }
}
impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe { (self.api.recognizer_free)(self.raw) }
    }
}
