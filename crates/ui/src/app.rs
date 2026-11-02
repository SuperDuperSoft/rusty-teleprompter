use dioxus::prelude::*;
use gloo_net::http::Request;
use rusty_core::*;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
#[wasm_bindgen(module = "/src/interop.js")]
extern "C" {
    fn initialize();
    fn native_boot() -> String;
    fn drain_events() -> String;
    fn native_call(command: &str, payload: &str);
    fn now() -> f64;
    fn local_get(key: &str) -> String;
    fn local_set(key: &str, value: &str);
    fn set_dirty(value: bool);
    fn set_following(value: bool);
    fn uuid() -> String;
    fn request_import();
    fn download_file(name: &str, text: &str);
    fn follow_word(index: usize, guide: f64, force: bool);
}
#[derive(Clone)]
struct Ui {
    tablet: bool,
    native: bool,
    library: Library,
    selected: String,
    selected_slide: usize,
    session: Option<Session>,
    pending: Option<Script>,
    removed: bool,
    settings: Settings,
    dirty: bool,
    blocked: bool,
    change: u64,
    last_edit: f64,
    save_status: String,
    message: String,
    addresses: Vec<String>,
    code: String,
    connection: String,
    tablet_status: String,
    voice_ready: bool,
    voice_status: String,
    help: bool,
    pairing: bool,
    settings_open: bool,
    address: String,
    pair_code: String,
    pair_busy: bool,
    follow: bool,
    confirm: Option<String>,
    recovery: Option<Script>,
    preview: bool,
}
impl Ui {
    fn new() -> Self {
        initialize();
        let boot: Value = serde_json::from_str(&native_boot()).unwrap_or_default();
        let native = boot["native"].as_bool().unwrap_or(false);
        let settings: Settings = serde_json::from_value(boot["settings"].clone())
            .ok()
            .or_else(|| serde_json::from_str(&local_get("rusty:settings")).ok())
            .unwrap_or_default();
        let library = serde_json::from_value(boot["library"].clone())
            .unwrap_or_else(|_| Library {
                generation: 0,
                scripts: if native { vec![demo()] } else { vec![] },
            });
        if boot["position"].get("script_id").is_some() {
            local_set("rusty:position", &boot["position"].to_string());
        }
        let selected = library.scripts.first().map(|s| s.id.clone()).unwrap_or_default();
        let recovery = serde_json::from_str(&local_get("rusty:draft")).ok();
        Self {
            tablet: native,
            native,
            library,
            selected,
            selected_slide: 0,
            session: None,
            pending: None,
            removed: false,
            settings,
            dirty: false,
            blocked: false,
            change: 0,
            last_edit: 0.0,
            save_status: "Saved on this PC".into(),
            message: String::new(),
            addresses: vec![],
            code: String::new(),
            connection: boot["connection"]
                .as_str()
                .unwrap_or("Not paired · scripts work offline")
                .into(),
            tablet_status: "Waiting for tablet".into(),
            voice_ready: boot["voiceReady"].as_bool().unwrap_or(false),
            voice_status: boot["voiceStatus"]
                .as_str()
                .unwrap_or("Preparing offline voice model…")
                .into(),
            help: false,
            pairing: false,
            settings_open: false,
            address: ip_only(boot["endpoint"].as_str().unwrap_or("")),
            pair_code: String::new(),
            pair_busy: false,
            follow: true,
            confirm: None,
            recovery,
            preview: false,
        }
    }
    fn script(&self) -> Option<&Script> {
        self.library.scripts.iter().find(|s| s.id == self.selected)
    }
    fn edit(&mut self, f: impl FnOnce(&mut Script)) {
        let id = self.selected.clone();
        if let Some(s) = self.library.scripts.iter_mut().find(|s| s.id == id) {
            f(s);
        }
        self.changed();
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.change += 1;
        self.last_edit = now();
        self.save_status = "Saving…".into();
        set_dirty(true);
        if let Some(s) = self.script() {
            local_set("rusty:draft", &serde_json::to_string(s).unwrap());
        }
    }
    fn can_switch(&mut self) -> bool {
        if self.dirty {
            self.message = "Your changes are still saving. Give them a moment, or export a backup."
                .into();
            false
        } else {
            true
        }
    }
    fn new_script(&mut self, sample: bool) {
        if !self.can_switch() {
            return;
        }
        let mut s = if sample {
            demo()
        } else {
            Script {
                id: uuid(),
                title: "Untitled script".into(),
                revision: 0,
                slides: vec![
                    Slide {
                        id: uuid(),
                        title: "Slide 1".into(),
                        text: String::new(),
                    },
                ],
            }
        };
        s.id = uuid();
        s.revision = 0;
        for slide in &mut s.slides {
            slide.id = uuid();
        }
        self.selected = s.id.clone();
        self.selected_slide = 0;
        self.library.scripts.push(s);
        self.changed();
    }
    fn select(&mut self, id: String) {
        if self.selected == id || self.can_switch() {
            self.selected = id;
            self.selected_slide = 0;
        }
    }
    fn duplicate_script(&mut self) {
        if !self.can_switch() {
            return;
        }
        if let Some(mut s) = self.script().cloned() {
            s.id = uuid();
            s.revision = 0;
            s.title = format!("{} copy", s.title.chars().take(190).collect::<String>());
            for slide in &mut s.slides {
                slide.id = uuid();
            }
            self.selected = s.id.clone();
            self.selected_slide = 0;
            self.library.scripts.push(s);
            self.changed();
        }
    }
    fn add_slide(&mut self) {
        let id = uuid();
        self.edit(|s| {
            s.slides
                .push(Slide {
                    id,
                    title: format!("Slide {}", s.slides.len() + 1),
                    text: String::new(),
                })
        });
        self.selected_slide = self
            .script()
            .map(|s| s.slides.len().saturating_sub(1))
            .unwrap_or(0);
    }
    fn duplicate_slide(&mut self) {
        let i = self.selected_slide;
        self.edit(|s| {
            if let Some(mut slide) = s.slides.get(i).cloned() {
                slide.id = uuid();
                slide.title = format!(
                    "{} copy",
                    slide.title.chars().take(190).collect::<String>(),
                );
                s.slides.insert(i + 1, slide);
            }
        });
        self.selected_slide = i + 1;
    }
    fn move_slide(&mut self, delta: isize) {
        let i = self.selected_slide;
        let target = i as isize + delta;
        let n = self.script().map(|s| s.slides.len()).unwrap_or(0);
        if target >= 0 && (target as usize) < n {
            self.edit(|s| s.slides.swap(i, target as usize));
            self.selected_slide = target as usize;
        }
    }
    fn open(&mut self, id: String) {
        if let Some(s) = self.library.scripts.iter().find(|s| s.id == id).cloned() {
            let mut session = Session::new(s);
            if let Ok(saved) = serde_json::from_str(&local_get("rusty:position")) {
                session.restore(&saved);
            }
            self.session = Some(session);
            self.pending = None;
            self.removed = false;
            self.follow = true;
        }
    }
    fn apply_pending(&mut self) {
        if let (Some(session), Some(pending)) = (&mut self.session, &self.pending) {
            if !session.phase.running() {
                session.apply_revision(pending.clone());
                self.pending = None;
                self.follow = true;
            }
        }
    }
    fn receive_library(&mut self, library: Library) {
        if let Some(s) = &self.session {
            let newest = library.scripts.iter().find(|v| v.id == s.script.id);
            self.removed = newest.is_none();
            self.pending = newest.filter(|v| v.revision != s.script.revision).cloned();
        }
        self.library = library;
        self.apply_pending();
    }
    fn action(&mut self, action: &str) {
        if action == "toggle"
            && self.session.as_ref().is_some_and(|s| !s.phase.running()) && self.native
            && !self.voice_ready
        {
            self.message = self.voice_status.clone();
            return;
        }
        if let Some(s) = &mut self.session {
            match action {
                "toggle" => {
                    if s.phase.running() {
                        s.pause();
                    } else {
                        if s.phase == Phase::Finished {
                            s.restart();
                        }
                        s.start();
                    }
                }
                "pause" => s.pause(),
                "next" => s.next(),
                "previous" => s.previous(),
                "restart" => s.restart(),
                _ => {}
            }
        }
        self.apply_pending();
        self.follow = true;
    }
    fn save_settings(&mut self) {
        self.settings.clamp();
        let text = serde_json::to_string(&self.settings).unwrap();
        local_set("rusty:settings", &text);
        native_call("settings", &text);
        if let Some(session)=&self.session { follow_word(session.tracker.position.unwrap_or(0),self.settings.guide,true); }
    }
    fn export(&self, plain: bool) {
        if let Some(s) = self.script() {
            let name: String = s
                .title
                .chars()
                .filter(|c| c.is_alphanumeric() || " -_".contains(*c))
                .take(80)
                .collect();
            let name = if name.is_empty() { "script".into() } else { name };
            if plain {
                let text = format!(
                    "{}\n\n{}",
                    s.title,
                    s
                        .slides
                        .iter()
                        .enumerate()
                        .map(|(i, sl)| {
                            format!(
                                "=== Slide {}: {} ===\n\n{}",
                                i + 1,
                                sl.title,
                                sl.text,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                );
                download_file(&format!("{name}.txt"), &text);
            } else {
                download_file(
                    &format!("{name}.rustyprompter.json"),
                    &serde_json::to_string_pretty(s).unwrap(),
                );
            }
        }
    }
    fn import(&mut self, name: &str, text: &str) {
        if name.ends_with(".json") {
            if !self.can_switch() {
                return;
            }
            match serde_json::from_str::<Script>(text) {
                Ok(mut s) => {
                    s.id = uuid();
                    s.revision = 0;
                    for slide in &mut s.slides {
                        slide.id = uuid();
                    }
                    self.selected = s.id.clone();
                    self.selected_slide = 0;
                    self.library.scripts.push(s);
                    self.changed();
                }
                Err(_) => {
                    self.message = "That file is not a RustyPrompter script backup."
                        .into();
                }
            }
        } else {
            if self.script().is_none() {
                self.new_script(false);
            }
            let title = name
                .trim_end_matches(".txt")
                .chars()
                .take(200)
                .collect::<String>();
            let slide = Slide {
                id: uuid(),
                title,
                text: text.into(),
            };
            if text.chars().count() > 100_000 {
                self.message = "Use at most 100,000 characters per slide.".into();
                return;
            }
            self.edit(|s| {
                if s.slides.len() == 1 && s.slides[0].text.is_empty() {
                    s.slides[0] = slide;
                } else {
                    s.slides.push(slide);
                }
            });
            self.selected_slide = self.script().unwrap().slides.len() - 1;
        }
    }
}
/// Keep only characters that can appear in an IPv4 address with optional port.
/// A pasted "http://192.168.1.5:8766/" becomes "192.168.1.5" (default port dropped).
fn ip_only(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let host = without_scheme.split('/').next().unwrap_or("");
    let host = host.strip_suffix(":8766").unwrap_or(host);
    host.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ':')
        .take(21)
        .collect()
}
#[allow(non_snake_case)]
pub fn App() -> Element {
    let state = use_signal(Ui::new);
    use_future(move || pump(state));
    let tablet = state.read().tablet;
    rsx! {
        document::Link { rel: "stylesheet", href: "/assets/main.css" }
        if tablet {
            TabletApp { state }
        } else {
            Studio { state }
        }
        Overlays { state }
    }
}
async fn fetch_json(
    path: &str,
    method: &str,
    body: Option<&Script>,
) -> Result<Value, String> {
    let request = match method {
        "PUT" => Request::put(path).json(body.unwrap()).map_err(|e| e.to_string())?,
        "DELETE" => Request::delete(path).build().map_err(|e| e.to_string())?,
        "POST" => Request::post(path).build().map_err(|e| e.to_string())?,
        _ => Request::get(path).build().map_err(|e| e.to_string())?,
    };
    let response = request
        .send()
        .await
        .map_err(|_| {
            "The PC companion is unavailable. Keep its window open.".to_string()
        })?;
    let ok = response.ok();
    let value: Value = response
        .json()
        .await
        .map_err(|_| "The companion returned an unreadable response.".to_string())?;
    if ok {
        Ok(value)
    } else {
        Err(value["detail"].as_str().unwrap_or("Could not save this script.").into())
    }
}
async fn pump(mut state: Signal<Ui>) {
    native_call("ui_ready", "");
    let mut previous = now();
    let mut last_poll = -3000.0;
    let mut last_persist = -2000.0;
    let mut audio = false;
    let mut current_slide = String::new();
    let mut shown_word = None;
    let mut was_following = true;
    loop {
        gloo_timers::future::TimeoutFuture::new(50).await;
        let time = now();
        let elapsed = (time - previous).max(0.0) as u64;
        previous = time;
        let events: Vec<Value> = serde_json::from_str(&drain_events())
            .unwrap_or_default();
        for event in events {
            let mut ui = state.write();
            match event["type"].as_str().unwrap_or("") {
                "library" => {
                    if let Ok(lib) = serde_json::from_value(event.clone()) {
                        ui.receive_library(lib);
                    }
                }
                "speech" => {
                    let text = event["text"].as_str().unwrap_or("");
                    let is_final = event["final"].as_bool().unwrap_or(false);
                    if let Some(session) = &mut ui.session {
                        session.speech(text, is_final);
                    }
                    ui.apply_pending();
                }
                "voice" => {
                    ui.voice_ready = event["ready"].as_bool().unwrap_or(false);
                    ui.voice_status = event["message"].as_str().unwrap_or("").into();
                }
                "connection" => {
                    ui.connection = event["message"].as_str().unwrap_or("").into();
                }
                "paired" => {
                    ui.pair_busy = false;
                    ui.pairing = false;
                    ui.message.clear();
                }
                "pair_error" => {
                    ui.pair_busy = false;
                    ui.message = event["message"]
                        .as_str()
                        .unwrap_or("Could not connect.")
                        .into();
                }
                "permission_granted" => {
                    if let Some(s) = &mut ui.session {
                        s.start();
                    }
                }
                "audio_error" => {
                    ui.action("pause");
                    ui.message = event["message"]
                        .as_str()
                        .unwrap_or("Microphone unavailable")
                        .into();
                }
                "pause" => ui.action("pause"),
                "error" => ui.message = event["message"].as_str().unwrap_or("").into(),
                "import" => {
                    ui.import(
                        event["name"].as_str().unwrap_or("script.txt"),
                        event["text"].as_str().unwrap_or(""),
                    )
                }
                _ => {}
            }
        }
        if state.peek().session.as_ref().is_some_and(|s| s.phase == Phase::Countdown || (s.phase.running() && elapsed > 1500)) {
            let mut ui = state.write();
            if elapsed > 1500 {
                ui.action("pause");
            } else if let Some(session) = &mut ui.session {
                session.tick(elapsed);
            }
            ui.apply_pending();
        }
        let desired = state.peek().native && state.peek().voice_ready
            && state.peek().session.as_ref().is_some_and(|s| s.phase == Phase::Reading);
        let slide_key = state
            .peek()
            .session
            .as_ref()
            .map(|s| format!("{}:{}:{}", s.script.id, s.script.revision, s.slide_index))
            .unwrap_or_default();
        if desired != audio || slide_key != current_slide {
            if audio {
                native_call("stop", "");
            }
            audio = desired;
            current_slide = slide_key;
            if desired {
                native_call("start", "");
            }
            shown_word = None;
        }
        let follow = state.peek().follow;
        if follow && !was_following {
            shown_word = None;
        }
        was_following = follow;
        set_following(follow);
        if let Some(session) = &state.peek().session {
            let position = session.tracker.position.unwrap_or(0);
            if follow && shown_word != Some(position) {
                follow_word(position, state.peek().settings.guide, shown_word.is_none());
                shown_word = Some(position);
            }
        }
        if time - last_persist > 1000.0 {
            last_persist = time;
            if let Some(session) = &state.peek().session {
                let saved = serde_json::to_string(&session.position()).unwrap();
                local_set("rusty:position", &saved);
                native_call("position", &saved);
                native_call(
                    "status",
                    &json!(
                        { "phase" : format!("{:?}", session.phase), "script" : session
                        .script.title, "pending" : state.peek().pending.is_some() }
                    )
                        .to_string(),
                );
            } else if state.peek().native {
                native_call(
                    "status",
                    &json!({ "phase" : "Library", "script" : "", "pending" : false })
                        .to_string(),
                );
            }
        }
        if !state.peek().native && !state.peek().tablet {
            let save = {
                let ui = state.peek();
                if ui.dirty && !ui.blocked && time - ui.last_edit >= 500.0 {
                    ui.script().cloned().map(|s| (s, ui.change))
                } else {
                    None
                }
            };
            if let Some((script, change)) = save {
                match fetch_json(
                        &format!("/api/scripts/{}", script.id),
                        "PUT",
                        Some(&script),
                    )
                    .await
                {
                    Ok(value) => {
                        let mut ui = state.write();
                        if let Some(current) = ui
                            .library
                            .scripts
                            .iter_mut()
                            .find(|s| s.id == script.id)
                        {
                            current.revision = value["revision"]
                                .as_u64()
                                .unwrap_or(current.revision);
                        }
                        if ui.change == change {
                            ui.dirty = false;
                            ui.save_status = "All changes saved".into();
                            ui.message.clear();
                            local_set("rusty:draft", "");
                            set_dirty(false);
                        }
                    }
                    Err(error) => {
                        let mut ui = state.write();
                        ui.blocked = error.contains("newer version")
                            || error.contains("unique slide");
                        ui.save_status = "Unsaved changes".into();
                        ui.message = error;
                        ui.last_edit = time + 1000.0;
                    }
                }
            }
            if time - last_poll >= 2000.0 {
                last_poll = time;
                match fetch_json("/api/state", "GET", None).await {
                    Ok(value) => {
                        let mut ui = state.write();
                        ui.addresses = value["addresses"]
                            .as_array()
                            .map(|v| {
                                v.iter()
                                    .filter_map(|s| s.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        ui.code = value["pairingCode"].as_str().unwrap_or("").into();
                        ui.tablet_status = if let Some(t) = value["tablets"]
                            .as_array()
                            .and_then(|v| v.first())
                        {
                            format!(
                                "Tablet connected · {}{}",
                                t["phase"].as_str().unwrap_or("Ready"),
                                if t["pending"].as_bool().unwrap_or(false) {
                                    " · Update waiting"
                                } else {
                                    ""
                                },
                            )
                        } else {
                            "Waiting for tablet".into()
                        };
                        if !ui.dirty && !ui.blocked {
                            if let Ok(lib) = serde_json::from_value::<Library>(value) {
                                ui.receive_library(lib);
                                if ui.script().is_none() {
                                    ui.selected = ui
                                        .library
                                        .scripts
                                        .first()
                                        .map(|s| s.id.clone())
                                        .unwrap_or_default();
                                    ui.selected_slide = 0;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let mut ui = state.write();
                        ui.tablet_status = "PC companion unavailable".into();
                        ui.message = e;
                    }
                }
            }
        }
    }
}
#[component]
fn Brand() -> Element {
    rsx! {
        div { class: "brand",
            span { class: "brand-icon", "r" }
            span {
                "Rusty"
                b { "Prompter" }
            }
        }
    }
}
#[component]
fn Studio(mut state: Signal<Ui>) -> Element {
    let ui = state.read().clone();
    let selected = ui.script().cloned();
    let script = selected
        .clone()
        .unwrap_or(Script {
            id: String::new(),
            title: String::new(),
            revision: 0,
            slides: vec![],
        });
    let sl = script.slides.get(ui.selected_slide).cloned();
    let index = ui.selected_slide;
    let slide = sl
        .clone()
        .unwrap_or(Slide {
            id: String::new(),
            title: String::new(),
            text: String::new(),
        });
    let count = words(&slide.text).len();
    let duration = ((count as f64) / 140.0 * 60.0).ceil() as usize;
    let addresses = ui.addresses.iter().map(|a| ip_only(a)).collect::<Vec<_>>().join(" or ");
    rsx! {
        div { class: "studio-shell",
            aside { class: "sidebar",
                Brand {}
                div { class: "side-caption", "YOUR SCRIPT STUDIO" }
                button {
                    class: "new-script",
                    onclick: move |_| state.write().new_script(false),
                    span { "+" }
                    " New script"
                }
                div { class: "side-section",
                    "SCRIPTS"
                    span { "{ui.library.scripts.len()}" }
                }
                nav { class: "script-nav", aria_label: "Scripts",
                    {
                        ui.library
                            .scripts
                            .clone()
                            .into_iter()
                            .map(|item| {
                                let id = item.id.clone();
                                let selected = item.id == ui.selected;
                                rsx! {
                                    button {
                                        key: "{item.id}",
                                        class: if selected { "script-item selected" } else { "script-item" },
                                        onclick: move | _ | state
                                                .write().select(id.clone()),
                                        span { class: "script-symbol", "▤" }
                                        div {
                                            strong { "{item.title}" }
                                            small { "{item.slides.len()} slides" }
                                        }
                                    }
                                }
                            })
                    }
                }
                div { class: "sidebar-bottom",
                    div { class: "local-label",
                        span { class: "local-dot" }
                        "ALL YOURS. ALWAYS."
                    }
                    p { "Local scripts. Offline voice following. Zero subscriptions." }
                    button {
                        class: "sidebar-help",
                        onclick: move |_| state.write().help = true,
                        "Setup & help ↗"
                    }
                }
            }
            main { class: "studio-main",
                header { class: "studio-top",
                    div { class: "breadcrumb",
                        "Script studio"
                        span { "/" }
                        strong {
                            if selected.is_some() {
                                "{script.title}"
                            } else {
                                "Welcome"
                            }
                        }
                    }
                    div { class: "save-state",
                        span { class: if ui.dirty { "save-dot busy" } else { "save-dot" } }
                        "{ui.save_status}"
                    }
                }
                if !ui.message.is_empty() {
                    div { class: "message", role: "alert",
                        span { "{ui.message}" }
                        button {
                            aria_label: "Dismiss message",
                            onclick: move |_| state.write().message.clear(),
                            "×"
                        }
                    }
                }
                section { class: "connection-card",
                    div { class: "connection-icon", "↗" }
                    div { class: "connection-copy",
                        span { class: "eyebrow", "FROM YOUR DESK TO YOUR TABLET" }
                        strong { "{ui.tablet_status}" }
                        small { "Both devices on the same Wi-Fi. Changes sync automatically." }
                    }
                    div { class: "connection-details",
                        span { class: "address", "{addresses}" }
                        div {
                            span { "PAIRING CODE" }
                            b { "{ui.code}" }
                        }
                    }
                }
                if selected.is_none() {
                    section { class: "welcome",
                        span { class: "welcome-tag", "WELCOME TO RUSTYPROMPTER" }
                        h1 {
                            "A good take starts"
                            br {}
                            "with your words."
                        }
                        p {
                            "Write your script one slide at a time. Your tablet listens,"
                            br {}
                            "follows your voice, and gives you a breath between slides."
                        }
                        div { class: "welcome-actions",
                            button {
                                class: "primary",
                                onclick: move | _ | state.write()
                                        .new_script(false),
                                "Create a script  +"
                            }
                            button {
                                class: "secondary",
                                onclick: move |_| state.write().new_script(true),
                                "Try the example"
                            }
                        }
                        div { class: "welcome-steps",
                            span { "01  Write on your PC" }
                            span { "02  Read on your tablet" }
                            span { "03  Find your flow" }
                        }
                    }
                } else {
                    div { class: "document-header",
                        div {
                            span { class: "eyebrow", "YOUR SCRIPT" }
                            input {
                                class: "script-title",
                                aria_label: "Script title",
                                maxlength: 200,
                                value: "{script.title}",
                                oninput: move |e| {
                                    let value = e.value();
                                    state
                                        .write()
                                        .edit(|s| {
                                            s.title = if value.trim().is_empty() {
                                                "Untitled script".into()
                                            } else {
                                                value
                                            };
                                        });
                                },
                            }
                        }
                        div { class: "document-actions",
                            button {
                                class: "secondary",
                                onclick: move | _ |
                                        request_import(),
                                "Import"
                            }
                            button {
                                class: "secondary",
                                onclick: move | _ |
                                        state.read().export(false),
                                "Backup"
                            }
                            button {
                                class: "secondary",
                                onclick: move |_| state.read().export(true),
                                "Export text"
                            }
                            button {
                                class: "secondary",
                                onclick: move |_| state.write().duplicate_script(),
                                "Duplicate"
                            }
                            button {
                                class: "icon-button danger",
                                aria_label: "Delete script",
                                title: "Delete script",
                                onclick: move | _ | state.write().confirm = Some("script"
                                        .into()),
                                "×"
                            }
                            button {
                                class: "preview-button",
                                onclick: move |_| {
                                    let mut u = state.write();
                                    if u.can_switch() {
                                        let id = u.selected.clone();
                                        u.open(id);
                                        u.tablet = true;
                                        u.preview = true;
                                    }
                                },
                                "Preview ↗"
                            }
                        }
                    }
                    div { class: "writing-grid",
                        nav { class: "slides-rail", aria_label: "Script slides",
                            div { class: "rail-heading",
                                span { class: "eyebrow", "SLIDES" }
                                span { class: "count-badge", "{script.slides.len()}" }
                            }
                            {
                                script
                                    .slides
                                    .iter()
                                    .enumerate()
                                    .map(|(i, sl)| {
                                        let title = sl.title.clone();
                                        let word_count = words(&sl.text).len();
                                        let active = i == index;
                                        rsx! {
                                            button {
                                                key: "{sl.id}",
                                                class: if active { "slide-card active" } else { "slide-card" },
                                                onclick: move |_| state.write().selected_slide = i,
                                                span { class: "slide-number", "{i+1:02}" }
                                                div {
                                                    strong { "{title}" }
                                                    small { "{word_count} words" }
                                                }
                                            }
                                        }
                                    })
                            }
                            button {
                                class: "add-slide",
                                onclick: move |
                                        _ | state.write().add_slide(),
                                "+ Add slide"
                            }
                            div { class: "rail-tip",
                                span { "5" }
                                p {
                                    "A little breathing room."
                                    small { "Five seconds between slides." }
                                }
                            }
                        }
                        article { class: "script-editor",
                            div { class: "editor-bar",
                                span { class: "eyebrow",
                                    if sl.is_some() {
                                        "SLIDE {index+1:02}"
                                    } else {
                                        "ADD A SLIDE TO BEGIN"
                                    }
                                }
                                div {
                                    button {
                                        class: "icon-button",
                                        aria_label: "Move slide up",
                                        disabled: index == 0 || sl.is_none(),
                                        onclick: move |_| state.write().move_slide(-1),
                                        "↑"
                                    }
                                    button {
                                        class: "icon-button",
                                        aria_label: "Move slide down",
                                        disabled: sl.is_none() || index + 1 >= script.slides.len(),
                                        onclick: move | _ | state
                                                .write().move_slide(1),
                                        "↓"
                                    }
                                    button {
                                        class: "text-button",
                                        disabled: sl
                                                .is_none(),
                                        onclick: move |_| state.write().duplicate_slide(),
                                        "Duplicate"
                                    }
                                    button {
                                        class: "text-button danger",
                                        disabled: sl.is_none(),
                                        onclick: move |
                                                _ | state.write().confirm = Some("slide".into()),
                                        "Delete"
                                    }
                                }
                            }
                            input {
                                class: "slide-title",
                                aria_label: "Slide title",
                                maxlength: 200,
                                disabled: sl
                                        .is_none(),
                                value: "{slide.title}",
                                placeholder: "Give this slide a title",
                                oninput: move |e| {
                                    let value = e.value();
                                    state
                                        .write()
                                        .edit(|s| {
                                            if let Some(sl) = s.slides.get_mut(index) {
                                                sl.title = value;
                                            }
                                        });
                                },
                            }
                            textarea {
                                class: "slide-text",
                                aria_label: "Slide script",
                                disabled: sl.is_none(),
                                maxlength: 100000,
                                value: "{slide.text}",
                                placeholder: "Type what you want to say…\n\nA pause, an extra word, a little personality. Read it your way.",
                                oninput: move |e| {
                                    let value = e.value();
                                    state
                                        .write()
                                        .edit(|s| {
                                            if let Some(sl) = s.slides.get_mut(index) {
                                                sl.text = value;
                                            }
                                        });
                                },
                            }
                            footer { class: "editor-footer",
                                span {
                                    strong { "{count}" }
                                    " words"
                                }
                                span { "About {duration}s" }
                                span { class: "cue-hint", "[Directions in brackets aren’t read aloud]" }
                            }
                        }
                        aside { class: "tablet-preview",
                            div { class: "rail-heading",
                                span { class: "eyebrow", "THE TABLET VIEW" }
                                span { "Aa" }
                            }
                            div { class: "mini-screen",
                                div { class: "mini-guide" }
                                div { class: "mini-text",
                                    if slide.text.is_empty() {
                                        "Your next great take starts right here."
                                    } else {
                                        "{slide.text}"
                                    }
                                }
                            }
                            div { class: "next-card",
                                span { "5" }
                                div {
                                    strong { "Read. Breathe. Next slide." }
                                    p { "Reach the end. A five-second countdown handles the rest." }
                                }
                            }
                            p { class: "preview-note",
                                "Edits made during a take wait until you pause. Your place stays yours."
                            }
                        }
                    }
                }
            }
            input {
                r#type: "file",
                id: "import-file",
                accept: ".txt,.json",
                hidden: true,
            }
        }
    }
}
#[component]
fn TabletApp(mut state: Signal<Ui>) -> Element {
    let ui = state.read().clone();
    if ui.session.is_some() {
        return rsx! {
            Prompter { state }
        };
    }
    let paired = ui.connection.starts_with("Connected");
    let count = ui.library.scripts.len();
    rsx! {
        main { class: "phone-home",
            header { class: "phone-bar",
                Brand {}
                button {
                    class: "round-button",
                    aria_label: "Help",
                    onclick: move |_| state.write().help = true,
                    "?"
                }
            }
            section { class: "status-panel",
                div { class: "status-row",
                    span { class: if paired { "dot ok" } else { "dot" } }
                    div {
                        strong { "{ui.connection}" }
                    }
                }
                div { class: "status-row",
                    span { class: if ui.voice_ready { "dot ok" } else { "dot wait" } }
                    div {
                        strong { "{ui.voice_status}" }
                    }
                }
                button {
                    class: if paired { "ghost-button wide" } else { "primary wide" },
                    onclick: move |_| {
                        let mut u = state.write();
                        u.message.clear();
                        u.pairing = true;
                    },
                    if paired {
                        "Connect to a different PC"
                    } else {
                        "Connect to PC"
                    }
                }
            }
            if !ui.message.is_empty() {
                div { class: "message", role: "alert",
                    span { "{ui.message}" }
                    button {
                        aria_label: "Dismiss",
                        onclick: move |_| state.write().message.clear(),
                        "×"
                    }
                }
            }
            div { class: "list-heading",
                h1 { "Scripts" }
                span { "{count}" }
            }
            div { class: "script-list",
                {
                    ui.library
                        .scripts
                        .into_iter()
                        .map(|script| {
                            let id = script.id.clone();
                            let words_total: usize = script
                                .slides
                                .iter()
                                .map(|s| words(&s.text).len())
                                .sum();
                            let slides = script.slides.len();
                            let minutes = (words_total as f64 / 140.0).ceil().max(1.0) as usize;
                            rsx! {
                                button {
                                    class: "script-row",
                                    key: "{script.id}",
                                    onclick: move |_| state.write().open(id.clone()),
                                    div {
                                        strong { "{script.title}" }
                                        small { "{slides} slides · {words_total} words · about {minutes} min" }
                                    }
                                    span { class: "chevron", "›" }
                                }
                            }
                        })
                }
                if count == 0 {
                    div { class: "empty-state",
                        strong { "No scripts yet" }
                        p { "Write a script in the PC studio, then connect this phone. It will show up here." }
                    }
                }
            }
        }
    }
}
#[component]
fn Prompter(mut state: Signal<Ui>) -> Element {
    let ui = state.read().clone();
    let session = ui.session.as_ref().unwrap();
    let slide = session
        .slide()
        .cloned()
        .unwrap_or(Slide {
            id: String::new(),
            title: String::new(),
            text: String::new(),
        });
    let running = session.phase.running();
    let current = session.tracker.position;
    let parts = session.tracker.words.clone();
    let raw = slide.text.clone();
    let tail_start = parts.last().map(|w| w.end).unwrap_or(0);
    let tail = raw[tail_start..].to_string();
    let x = if ui.settings.mirror_x { -1 } else { 1 };
    let y = if ui.settings.mirror_y { -1 } else { 1 };
    let transform = format!("transform:scale({x},{y});");
    let text_style = format!(
        "font-size:{}px;line-height:{};padding-left:{}%;padding-right:{}%;padding-top:{}vh;",
        ui.settings.font_size,
        ui.settings.line_spacing,
        ui.settings.margins,
        ui.settings.margins,
        ui.settings.guide * 0.74,
    );
    let countdown = if session.phase == Phase::Countdown {
        Some(session.remaining_ms.div_ceil(1000))
    } else {
        None
    };
    let total = session.script.slides.len();
    let number = session.slide_index + 1;
    let progress = if parts.is_empty() {
        0.0
    } else {
        current.map(|p| (p + 1) as f64 / parts.len() as f64 * 100.0).unwrap_or(0.0)
    };
    let status = if ui.preview {
        "Preview · use Next to change slides"
    } else if session.phase == Phase::Reading {
        "Listening"
    } else if session.phase == Phase::Countdown {
        "Next slide soon"
    } else if session.phase == Phase::Finished {
        "Take complete"
    } else if session.phase == Phase::Paused {
        "Paused"
    } else {
        "Ready"
    };
    rsx! {
        main { class: "prompter",
            header { class: "prompter-bar",
                button {
                    class: "round-button",
                    aria_label: "Back to scripts",
                    onclick: move |_| {
                        let mut u = state.write();
                        u.action("pause");
                        u.session = None;
                        if u.preview {
                            u.tablet = false;
                            u.preview = false;
                        }
                    },
                    "‹"
                }
                div { class: "prompter-title",
                    strong { "{session.script.title}" }
                    small {
                        span { class: if session.phase == Phase::Reading { "dot ok pulse" } else { "dot" } }
                        "{status} · Slide {number}/{total}"
                    }
                }
                button {
                    class: "round-button",
                    aria_label: "Reading settings",
                    onclick: move |_| {
                        let mut u = state.write();
                        u.action("pause");
                        u.settings_open = true;
                    },
                    "Aa"
                }
            }
            div { class: "progress", div { style: "width:{progress}%" } }
            if ui.pending.is_some() {
                div { class: "notice", "PC edits waiting · they apply when you pause" }
            } else if ui.removed {
                div { class: "notice", "Script removed on PC · this take is kept" }
            }
            if !ui.message.is_empty() {
                div { class: "message dark", role: "alert",
                    span { "{ui.message}" }
                    button {
                        aria_label: "Dismiss",
                        onclick: move |_| state.write().message.clear(),
                        "×"
                    }
                }
            }
            div { class: "prompt-mirror", style: "{transform}",
                if ui.settings.show_guide {
                    div {
                        class: "reading-guide",
                        style: format!(
                            "top:{}%;height:{}px;",
                            ui.settings.guide,
                            ui.settings.font_size * ui.settings.line_spacing,
                        ),
                    }
                }
                div {
                    class: "script-viewport",
                    id: "script-viewport",
                    onwheel: move |_| state.write().follow = false,
                    ontouchmove: move |_| state.write().follow = false,
                    div { class: "prompt-text", style: "{text_style}",
                        {
                            parts
                                .iter()
                                .enumerate()
                                .map(|(i, word)| {
                                    let previous = if i == 0 { 0 } else { parts[i - 1].end };
                                    let before = raw[previous..word.start].to_string();
                                    let spoken = raw[word.start..word.end].to_string();
                                    let class = match current {
                                        Some(c) if c == i => "spoken-word active-word",
                                        Some(c) if i < c => "spoken-word read-word",
                                        _ => "spoken-word",
                                    };
                                    rsx! {
                                        span { class: "cue-text", "{before}" }
                                        span {
                                            id: "word-{i}",
                                            class,
                                            onclick: move |_| {
                                                let mut u = state.write();
                                                if let Some(s) = &mut u.session {
                                                    s.seek(i);
                                                }
                                                u.follow = true;
                                            },
                                            "{spoken}"
                                        }
                                    }
                                })
                        }
                        span { class: "cue-text", "{tail}" }
                    }
                }
                if !ui.follow && running {
                    button {
                        class: "follow-pill",
                        onclick: move |_| state.write().follow = true,
                        "↓ Follow my voice"
                    }
                }
                if let Some(number) = countdown {
                    div { class: "overlay",
                        span { class: "overlay-label", "Next slide in" }
                        strong { "{number}" }
                        button {
                            class: "ghost-button",
                            onclick: move |_| state.write().action("next"),
                            "Go now"
                        }
                    }
                }
                if session.phase == Phase::Finished {
                    div { class: "overlay",
                        span { class: "overlay-label", "That’s a take" }
                        strong { class: "overlay-word", "Done" }
                        button {
                            class: "primary",
                            onclick: move |_| state.write().action("toggle"),
                            "Read again"
                        }
                    }
                }
                if session.phase == Phase::Paused && session.remaining_ms < 5000 {
                    div { class: "overlay",
                        span { class: "overlay-label", "Countdown paused" }
                        strong { "{session.remaining_ms.div_ceil(1000)}" }
                        span { class: "overlay-note", "Tap Resume when ready" }
                    }
                }
            }
            footer { class: "prompt-controls",
                button {
                    class: "control",
                    aria_label: "Restart slide set",
                    onclick: move |_| state.write().action("restart"),
                    span { class: "control-icon", "↺" }
                    span { class: "control-label", "Restart" }
                }
                button {
                    class: "control",
                    aria_label: "Previous slide",
                    disabled: number <= 1,
                    onclick: move |_| state.write().action("previous"),
                    span { class: "control-icon", "‹" }
                    span { class: "control-label", "Back" }
                }
                button {
                    class: if running { "play paused" } else { "play" },
                    onclick: move |_| state.write().action("toggle"),
                    if running {
                        "Pause"
                    } else if session.phase == Phase::Finished {
                        "Again"
                    } else if session.phase == Phase::Ready {
                        "Start"
                    } else {
                        "Resume"
                    }
                }
                button {
                    class: "control",
                    aria_label: "Next slide",
                    disabled: session.phase == Phase::Finished,
                    onclick: move |_| state.write().action("next"),
                    span { class: "control-icon", "›" }
                    span { class: "control-label", "Next" }
                }
            }
        }
    }
}
#[component]
fn SettingSlider(
    mut state: Signal<Ui>,
    label: String,
    field: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
) -> Element {
    let shown = if step >= 1.0 { format!("{value:.0}") } else { format!("{value:.1}") };
    rsx! {
        label { class: "setting-slider",
            span {
                "{label}"
                b { "{shown}" }
            }
            input {
                r#type: "range",
                aria_label: "{label}",
                min: "{min}",
                max: "{max}",
                step: "{step}",
                value: "{value}",
                oninput: move |e| {
                    if let Ok(v) = e.value().parse() {
                        let mut ui = state.write();
                        match field.as_str() {
                            "font" => ui.settings.font_size = v,
                            "margins" => ui.settings.margins = v,
                            "spacing" => ui.settings.line_spacing = v,
                            "guide" => ui.settings.guide = v,
                            _ => {}
                        }
                        ui.save_settings();
                        ui.follow = true;
                    }
                },
            }
        }
    }
}
#[component]
fn SettingSwitch(mut state: Signal<Ui>, label: String, field: String, on: bool) -> Element {
    rsx! {
        button {
            class: "switch-row",
            role: "switch",
            aria_checked: "{on}",
            onclick: move |_| {
                let mut u = state.write();
                match field.as_str() {
                    "guide" => u.settings.show_guide = !u.settings.show_guide,
                    "mirror_x" => u.settings.mirror_x = !u.settings.mirror_x,
                    "mirror_y" => u.settings.mirror_y = !u.settings.mirror_y,
                    _ => {}
                }
                u.save_settings();
            },
            span { "{label}" }
            span { class: if on { "switch on" } else { "switch" }, span {} }
        }
    }
}
#[component]
fn Overlays(mut state: Signal<Ui>) -> Element {
    let ui = state.read().clone();
    rsx! {
        if ui.help {
            div { class: "modal-backdrop",
                div {
                    class: "modal",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Setup and help",
                    div { class: "modal-heading",
                        span { class: "eyebrow", "MAKE YOURSELF AT HOME" }
                        button {
                            class: "icon-button",
                            aria_label: "Close help",
                            onclick: move | _ | state
                                    .write().help = false,
                            "×"
                        }
                    }
                    h2 {
                        "From your desk"
                        br {}
                        "to your next take."
                    }
                    ol {
                        li {
                            "Keep RustyPrompter open on your PC. Allow Private networks if Windows Firewall asks."
                        }
                        li { "Connect your PC and tablet to the same private Wi-Fi network." }
                        li {
                            "On your tablet, choose Connect to PC. Enter the address and six-digit code shown in the studio."
                        }
                        li { "Choose a script and tap Start. Your voice sets the pace." }
                    }
                    div { class: "help-note",
                        strong { "A little room to be yourself." }
                        p {
                            "Pauses and short ad-libs are fine. Reaching the end of the spoken text starts a five-second countdown. If you skip the entire ending, tap Next."
                        }
                    }
                    p {
                        "Put directions in [brackets]. Edits made during a take wait until Pause or completion. Cached scripts and voice following work without Wi-Fi or the PC."
                    }
                    p {
                        "Backup keeps your slides in an editable JSON file. Export text gives you a plain-text copy. Import accepts both. The Windows database lives in %LOCALAPPDATA%\\RustyPrompter."
                    }
                    p {
                        "Built with Rust and Dioxus. Offline recognition by Vosk. License notices are included with your downloads. Audio is not recorded or uploaded. Sync is intended for a trusted private LAN."
                    }
                    if !ui.native {
                        button {
                            class: "danger secondary",
                            onclick: move |_| {
                                state.write().confirm = Some("unpair".into());
                                state.write().help = false;
                            },
                            "Disconnect paired tablets"
                        }
                    }
                }
            }
        }
        if ui.pairing {
            div { class: "sheet-backdrop",
                div {
                    class: "sheet connect-sheet",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Connect to PC",
                    div { class: "sheet-bar",
                        button {
                            class: "round-button",
                            aria_label: "Close",
                            onclick: move |_| state.write().pairing = false,
                            "‹"
                        }
                        h2 { "Connect to PC" }
                        span {}
                    }
                    ol { class: "steps",
                        li { "Open RustyPrompter on your PC." }
                        li { "Keep this phone on the same Wi-Fi." }
                        li { "Type the IP and code shown on the PC. No http needed." }
                    }
                    label { class: "field",
                        span { "PC IP address" }
                        input {
                            aria_label: "PC IP address",
                            r#type: "text",
                            inputmode: "decimal",
                            autocomplete: "off",
                            autocapitalize: "off",
                            spellcheck: "false",
                            maxlength: 21,
                            placeholder: "192.168.1.5",
                            value: "{ui.address}",
                            oninput: move |e| state.write().address = ip_only(&e.value()),
                        }
                    }
                    label { class: "field",
                        span { "6-digit code" }
                        input {
                            class: "code-input",
                            aria_label: "Pairing code",
                            inputmode: "numeric",
                            autocomplete: "one-time-code",
                            maxlength: 6,
                            placeholder: "••••••",
                            value: "{ui.pair_code}",
                            oninput: move |e| {
                                let digits: String = e.value().chars().filter(|c| c.is_ascii_digit()).take(6).collect();
                                state.write().pair_code = digits;
                            },
                        }
                    }
                    if !ui.message.is_empty() {
                        p { class: "form-error", role: "alert", "{ui.message}" }
                    }
                    button {
                        class: "primary wide big",
                        disabled: ui.pair_busy || ui.address.trim().is_empty() || ui.pair_code.len() != 6,
                        onclick: move |_| {
                            let mut u = state.write();
                            u.pair_busy = true;
                            u.message.clear();
                            native_call(
                                "pair",
                                &json!({ "address" : u.address, "code" : u.pair_code }).to_string(),
                            );
                        },
                        if ui.pair_busy {
                            "Connecting…"
                        } else {
                            "Connect"
                        }
                    }
                    p { class: "hint",
                        "Won’t connect? Allow RustyPrompter in Windows Firewall (Private networks), and turn off VPN or guest Wi-Fi on either device."
                    }
                }
            }
        }
        if ui.settings_open {
            div {
                class: "sheet-backdrop",
                onclick: move |_| state.write().settings_open = false,
                div {
                    class: "sheet settings-sheet",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Reading settings",
                    onclick: move |e| e.stop_propagation(),
                    div { class: "sheet-bar",
                        span {}
                        h2 { "Reading settings" }
                        button {
                            class: "round-button",
                            aria_label: "Close settings",
                            onclick: move |_| state.write().settings_open = false,
                            "×"
                        }
                    }
                    SettingSlider {
                        state,
                        label: "Text size",
                        field: "font",
                        value: ui.settings.font_size,
                        min: 22.0,
                        max: 90.0,
                        step: 1.0,
                    }
                    SettingSlider {
                        state,
                        label: "Line spacing",
                        field: "spacing",
                        value: ui.settings.line_spacing,
                        min: 1.0,
                        max: 2.4,
                        step: 0.1,
                    }
                    SettingSlider {
                        state,
                        label: "Side margins",
                        field: "margins",
                        value: ui.settings.margins,
                        min: 2.0,
                        max: 20.0,
                        step: 1.0,
                    }
                    SettingSlider {
                        state,
                        label: "Reading line position",
                        field: "guide",
                        value: ui.settings.guide,
                        min: 15.0,
                        max: 60.0,
                        step: 1.0,
                    }
                    div { class: "setting-group",
                        span { class: "setting-name", "Screen rotation" }
                        div { class: "segmented", role: "radiogroup",
                            {
                                [("auto", "Auto"), ("portrait", "Portrait"), ("landscape", "Landscape")]
                                    .into_iter()
                                    .map(|(value, label)| {
                                        let on = ui.settings.orientation == value;
                                        rsx! {
                                            button {
                                                key: "{value}",
                                                class: if on { "on" } else { "" },
                                                role: "radio",
                                                aria_checked: "{on}",
                                                onclick: move |_| {
                                                    let mut u = state.write();
                                                    u.settings.orientation = value.into();
                                                    u.save_settings();
                                                },
                                                "{label}"
                                            }
                                        }
                                    })
                            }
                        }
                    }
                    div { class: "setting-group switches",
                        SettingSwitch { state, label: "Show reading line", field: "guide", on: ui.settings.show_guide }
                        SettingSwitch { state, label: "Mirror left–right", field: "mirror_x", on: ui.settings.mirror_x }
                        SettingSwitch { state, label: "Flip upside down", field: "mirror_y", on: ui.settings.mirror_y }
                    }
                    button {
                        class: "primary wide big",
                        onclick: move |_| state.write().settings_open = false,
                        "Done"
                    }
                }
            }
        }
        if let Some(kind) = ui.confirm {
            div { class: "modal-backdrop",
                div {
                    class: "modal",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Confirm action",
                    h2 {
                        if kind == "unpair" {
                            "Disconnect paired tablets?"
                        } else if kind == "script" {
                            "Delete this script?"
                        } else {
                            "Delete this slide?"
                        }
                    }
                    p {
                        if kind == "unpair" {
                            "Cached scripts stay on the tablet. It will need the new code to sync again."
                        } else {
                            "This removes the selected content. Export a backup first if you want to keep it."
                        }
                    }
                    div { class: "modal-actions",
                        button {
                            class: "secondary",
                            onclick: move |
                                    _ | state.write().confirm = None,
                            "Keep it"
                        }
                        button {
                            class: "primary destructive",
                            onclick: move |_| {
                                let action = kind.clone();
                                spawn(async move {
                                    if action == "slide" {
                                        let mut u = state.write();
                                        let i = u.selected_slide;
                                        u.edit(|s| {
                                            if i < s.slides.len() {
                                                s.slides.remove(i);
                                            }
                                        });
                                        u.selected_slide = u
                                            .selected_slide
                                            .min(
                                                u.script().map(|s| s.slides.len().saturating_sub(1)).unwrap_or(0),
                                            );
                                        u.confirm = None;
                                    } else {
                                        let request = {
                                            let u = state.read();
                                            if u.dirty {
                                                None
                                            } else if action == "unpair" {
                                                Some(("/api/unpair".into(), "POST"))
                                            } else {
                                                u.script()
                                                    .map(|s| (
                                                        format!("/api/scripts/{}?revision={}", s.id, s.revision),
                                                        "DELETE",
                                                    ))
                                            }
                                        };
                                        if let Some((path, method)) = request {
                                            match fetch_json(&path, method, None).await {
                                                Ok(_) => {
                                                    let mut u = state.write();
                                                    if action == "script" {
                                                        let id = u.selected.clone();
                                                        u.library.scripts.retain(|s| s.id != id);
                                                        u.selected = u
                                                            .library
                                                            .scripts
                                                            .first()
                                                            .map(|s| s.id.clone())
                                                            .unwrap_or_default();
                                                        u.selected_slide = 0;
                                                    }
                                                    u.confirm = None;
                                                }
                                                Err(e) => state.write().message = e,
                                            }
                                        } else {
                                            state.write().message = "Wait for your changes to finish saving first."
                                                .into();
                                            state.write().confirm = None;
                                        }
                                    }
                                });
                            },
                            if kind == "unpair" {
                                "Disconnect"
                            } else {
                                "Delete"
                            }
                        }
                    }
                }
            }
        }
        if let Some(recovery) = ui.recovery {
            div { class: "modal-backdrop",
                div {
                    class: "modal",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Recover unsaved draft",
                    span { class: "eyebrow", "NOTHING LOST" }
                    h2 { "Pick up where you left off?" }
                    p {
                        "There’s an unsaved draft of “{recovery.title}” from your previous session."
                    }
                    div { class: "modal-actions",
                        button {
                            class: "secondary",
                            onclick: move |_| {
                                local_set("rusty:draft", "");
                                state.write().recovery = None;
                            },
                            "Discard draft"
                        }
                        button {
                            class: "primary",
                            disabled: ui.code.is_empty(),
                            onclick: move |_| {
                                let mut u = state.write();
                                if let Some(mut s) = u.recovery.take() {
                                    s.revision = u
                                        .library
                                        .scripts
                                        .iter()
                                        .find(|x| x.id == s.id)
                                        .map(|x| x.revision)
                                        .unwrap_or(0);
                                    u.library.scripts.retain(|x| x.id != s.id);
                                    u.selected = s.id.clone();
                                    u.selected_slide = 0;
                                    u.library.scripts.push(s);
                                    u.changed();
                                }
                            },
                            "Restore draft"
                        }
                    }
                }
            }
        }
    }
}
