use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, DefaultBodyLimit, Path, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use redb::{Database, ReadableTable, TableDefinition};
use rusty_core::{Library, Script};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use subtle::ConstantTimeEq;
pub mod paths;
pub mod voice;
use voice::Voice;

const DATA: TableDefinition<&str, &str> = TableDefinition::new("library");
const DEVICES: TableDefinition<&str, bool> = TableDefinition::new("devices");
/// Largest single recording chunk the browser may upload.
pub const RECORDING_CHUNK_LIMIT: usize = 32_000_000;

pub struct Store {
    db: Database,
}
impl Store {
    pub fn open(folder: &std::path::Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(folder)?;
        let db = Database::create(folder.join("rustyprompter.redb"))?;
        let write = db.begin_write()?;
        {
            let mut table = write.open_table(DATA)?;
            if table.get("snapshot")?.is_none() {
                table.insert(
                    "snapshot",
                    serde_json::to_string(&Library::default())?.as_str(),
                )?;
            }
        }
        write.open_table(DEVICES)?;
        write.commit()?;
        Ok(Self { db })
    }
    pub fn library(&self) -> anyhow::Result<Library> {
        let read = self.db.begin_read()?;
        let table = read.open_table(DATA)?;
        let value = table.get("snapshot")?.unwrap();
        Ok(serde_json::from_str(value.value())?)
    }
    pub fn save(&self, mut script: Script) -> Result<Script, ApiError> {
        validate(&script)?;
        let write = self.db.begin_write().map_err(internal)?;
        {
            let mut table = write.open_table(DATA).map_err(internal)?;
            let mut library: Library =
                serde_json::from_str(table.get("snapshot").map_err(internal)?.unwrap().value())
                    .map_err(internal)?;
            let current = library
                .scripts
                .iter()
                .find(|s| s.id == script.id)
                .map(|s| s.revision)
                .unwrap_or(0);
            if script.revision != current {
                return Err(ApiError(
                    StatusCode::CONFLICT,
                    "A newer version exists. Export your draft, then reload.".into(),
                ));
            }
            script.revision = current + 1;
            if let Some(old) = library.scripts.iter_mut().find(|s| s.id == script.id) {
                *old = script.clone();
            } else {
                library.scripts.push(script.clone());
            }
            library.generation += 1;
            table
                .insert(
                    "snapshot",
                    serde_json::to_string(&library).map_err(internal)?.as_str(),
                )
                .map_err(internal)?;
        }
        write.commit().map_err(internal)?;
        Ok(script)
    }
    pub fn delete(&self, id: &str, revision: u64) -> Result<(), ApiError> {
        let write = self.db.begin_write().map_err(internal)?;
        {
            let mut table = write.open_table(DATA).map_err(internal)?;
            let mut library: Library =
                serde_json::from_str(table.get("snapshot").map_err(internal)?.unwrap().value())
                    .map_err(internal)?;
            if let Some(s) = library.scripts.iter().find(|s| s.id == id) {
                if s.revision != revision {
                    return Err(ApiError(
                        StatusCode::CONFLICT,
                        "A newer version exists. Reload before deleting.".into(),
                    ));
                }
            }
            library.scripts.retain(|s| s.id != id);
            library.generation += 1;
            table
                .insert(
                    "snapshot",
                    serde_json::to_string(&library).map_err(internal)?.as_str(),
                )
                .map_err(internal)?;
        }
        write.commit().map_err(internal)?;
        Ok(())
    }
    pub fn pair(&self, token: &str) -> anyhow::Result<()> {
        let write = self.db.begin_write()?;
        {
            write
                .open_table(DEVICES)?
                .insert(digest(token).as_str(), true)?;
        }
        write.commit()?;
        Ok(())
    }
    pub fn is_device(&self, token: &str) -> bool {
        self.db
            .begin_read()
            .ok()
            .and_then(|r| r.open_table(DEVICES).ok())
            .and_then(|t| t.get(digest(token).as_str()).ok().flatten().map(|_| true))
            .unwrap_or(false)
    }
    pub fn unpair(&self) -> anyhow::Result<()> {
        let write = self.db.begin_write()?;
        {
            let mut table = write.open_table(DEVICES)?;
            table.retain(|_, _| false)?;
        }
        write.commit()?;
        Ok(())
    }
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub fn secret() -> String {
    (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect()
}
fn equal(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
pub fn validate(script: &Script) -> Result<(), ApiError> {
    let invalid = script.id.is_empty()
        || script.id.len() > 80
        || script.title.trim().is_empty()
        || script.title.chars().count() > 200
        || script.slides.len() > 500
        || script.slides.iter().any(|s| {
            s.id.is_empty()
                || s.id.len() > 80
                || s.title.chars().count() > 200
                || s.text.chars().count() > 100_000
        })
        || script.slides.iter().map(|s| s.text.len()).sum::<usize>() > 1_000_000;
    let unique: std::collections::HashSet<_> = script.slides.iter().map(|s| &s.id).collect();
    if invalid || unique.len() != script.slides.len() {
        return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "Use unique slide IDs, a script title under 200 characters, and at most 100,000 characters per slide.".into()));
    }
    Ok(())
}
pub struct ApiError(pub StatusCode, pub String);
impl std::fmt::Debug for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"detail": self.1}))).into_response()
    }
}
fn internal(e: impl std::fmt::Display) -> ApiError {
    eprintln!("RustyPrompter storage error: {e}");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Could not access the local script database.".into(),
    )
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Tablet {
    pub phase: String,
    pub script: String,
    pub pending: bool,
}
pub struct Pairing {
    pub code: String,
    attempts: HashMap<String, Vec<Instant>>,
}
pub struct AppState {
    pub store: Store,
    pub editor_key: String,
    pub pairing: Mutex<Pairing>,
    pub tablets: Mutex<HashMap<u64, Tablet>>,
    pub web: PathBuf,
    pub port: u16,
    /// Offline voice for reading on this PC.
    pub voice: Arc<Voice>,
    /// Camera recordings made while reading on this PC go here.
    pub recordings: PathBuf,
    /// Next chunk number each in-progress recording expects.
    recording_parts: Mutex<HashMap<PathBuf, u32>>,
}
impl AppState {
    /// Voice files are looked for in a `voice` folder next to `web`.
    pub fn new(folder: &std::path::Path, web: PathBuf, port: u16) -> anyhow::Result<Arc<Self>> {
        let voice = web.parent().map(|p| p.join("voice")).unwrap_or_default();
        Self::with_voice(folder, web, voice, port)
    }
    pub fn with_voice(
        folder: &std::path::Path,
        web: PathBuf,
        voice: PathBuf,
        port: u16,
    ) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            store: Store::open(folder)?,
            editor_key: secret(),
            pairing: Mutex::new(Pairing {
                code: format!("{:06}", rand::random::<u32>() % 1_000_000),
                attempts: HashMap::new(),
            }),
            tablets: Mutex::new(HashMap::new()),
            web,
            port,
            voice: Voice::new(voice),
            recordings: paths::videos_folder().unwrap_or_else(|_| folder.join("Recordings")),
            recording_parts: Mutex::new(HashMap::new()),
        }))
    }
    pub fn addresses(&self) -> Vec<String> {
        let mut addresses = local_ip_address::list_afinet_netifas()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, ip)| {
                match ip {
                    std::net::IpAddr::V4(v) if !v.is_loopback() && !v.is_unspecified() && !v.is_link_local() => Some(v),
                    _ => None,
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        addresses.sort_by_key(|ip| (!ip.is_private(), *ip));
        addresses.into_iter().map(|ip| format!("http://{ip}:{}", self.port)).collect()
    }
    pub fn launch_url(&self) -> String {
        format!("http://127.0.0.1:{}/?key={}", self.port, self.editor_key)
    }
}
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(editor_state))
        .route("/api/scripts/{id}", put(save_script).delete(delete_script))
        .route("/api/pair", post(pair))
        .route("/api/library", get(library))
        .route("/api/unpair", post(unpair))
        .route("/api/device/scripts/{id}", put(device_save).delete(device_delete))
        .route("/api/voice", get(voice_socket))
        // Video arrives in ~1 s chunks; this route alone allows big bodies.
        .route(
            "/api/recordings/{stem}",
            post(save_recording).layer(DefaultBodyLimit::max(RECORDING_CHUNK_LIMIT)),
        )
        .route("/ws", get(ws))
        .route("/assets/{*path}", get(asset))
        .route("/favicon.ico", get(asset_icon))
        .layer(DefaultBodyLimit::max(2_000_000))
        .layer(middleware::from_fn(safety_headers))
        .with_state(state)
}
async fn safety_headers(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("Cache-Control", HeaderValue::from_static("no-store"));
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("Content-Security-Policy", HeaderValue::from_static("default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'"));
    response
}
fn editor(peer: SocketAddr, headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let cookie = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .find_map(|s| s.trim().strip_prefix("rusty_editor="));
    if !peer.ip().is_loopback() || !cookie.is_some_and(|v| equal(v, &state.editor_key)) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Open the editor using RustyPrompter.exe on this PC.".into(),
        ));
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let host = headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if origin != format!("http://{host}") {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Wrong editor origin".into(),
            ));
        }
    }
    Ok(())
}
fn device(headers: &HeaderMap, state: &AppState) -> Result<String, ApiError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if token.is_empty() || !state.store.is_device(token) {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Connect this tablet to the PC again.".into(),
        ));
    }
    Ok(token.into())
}
async fn index(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    if let Some(key) = query.get("key") {
        if !peer.ip().is_loopback() || !equal(key, &state.editor_key) {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Invalid editor link".into(),
            ));
        }
        let mut response = StatusCode::SEE_OTHER.into_response();
        response
            .headers_mut()
            .insert("Location", HeaderValue::from_static("/"));
        response.headers_mut().insert(
            "Set-Cookie",
            HeaderValue::from_str(&format!(
                "rusty_editor={}; HttpOnly; SameSite=Strict; Path=/",
                state.editor_key
            ))
            .unwrap(),
        );
        return Ok(response);
    }
    editor(peer, &headers, &state)?;
    let html = std::fs::read(state.web.join("index.html")).map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "The bundled interface is missing. Extract all files from the Windows ZIP together."
                .into(),
        )
    })?;
    Ok(([("Content-Type", "text/html; charset=utf-8")], html).into_response())
}
async fn asset(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(path): Path<String>,
) -> Result<Response, ApiError> {
    if !peer.ip().is_loopback() {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Editor assets are local to this PC".into(),
        ));
    }
    if path.split('/').any(|s| s == ".." || s.contains('\\')) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Invalid asset path".into(),
        ));
    }
    let file = state.web.join("assets").join(&path);
    let bytes = std::fs::read(&file)
        .map_err(|_| ApiError(StatusCode::NOT_FOUND, "Asset not found".into()))?;
    let mime = match file.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "wasm" => "application/wasm",
        "js" => "application/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };
    Ok(([("Content-Type", mime)], bytes).into_response())
}
async fn asset_icon() -> StatusCode {
    StatusCode::NO_CONTENT
}
async fn editor_state(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    editor(peer, &headers, &state)?;
    let library = state.store.library().map_err(internal)?;
    Ok(Json(
        serde_json::json!({"generation":library.generation,"scripts":library.scripts,"pairingCode":state.pairing.lock().unwrap().code,"addresses":state.addresses(),"tablets":state.tablets.lock().unwrap().values().cloned().collect::<Vec<_>>()}),
    ))
}
async fn save_script(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(script): Json<Script>,
) -> Result<Json<Script>, ApiError> {
    editor(peer, &headers, &state)?;
    if id != script.id {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Script ID mismatch".into(),
        ));
    }
    Ok(Json(state.store.save(script)?))
}
async fn delete_script(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    editor(peer, &headers, &state)?;
    let revision = query
        .get("revision")
        .and_then(|v| v.parse().ok())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing revision".into()))?;
    state.store.delete(&id, revision)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
/// Saves one chunk of a camera recording made while reading on this PC.
/// `part=0` creates the file (never overwriting one); later parts append,
/// so everything recorded so far is on disk even if the browser closes.
async fn save_recording(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(stem): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    editor(peer, &headers, &state)?;
    let bad = |text: &str| ApiError(StatusCode::BAD_REQUEST, text.into());
    if rusty_core::recording::safe_stem(&stem).as_deref() != Some(stem.as_str()) {
        return Err(bad("Invalid recording name"));
    }
    let ext = query.get("ext").map(String::as_str).unwrap_or("");
    if !matches!(ext, "mp4" | "webm") {
        return Err(bad("Recordings are saved as mp4 or webm"));
    }
    let part: u32 = query
        .get("part")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| bad("Missing recording part"))?;
    let folder = state.recordings.clone();
    let name = format!("{stem}.{ext}");
    let file = folder.join(&name);
    if part > 0 {
        // Only the next chunk of a recording started by this run, so a retry,
        // a lost chunk or an old file can never scramble a video.
        let expected = state.recording_parts.lock().unwrap().get(&file).copied();
        if expected != Some(part) {
            return Err(if expected.is_none() && !file.exists() {
                ApiError(StatusCode::NOT_FOUND, "Recording not started.".into())
            } else {
                ApiError(StatusCode::CONFLICT, "Recording chunks arrived out of order.".into())
            });
        }
    }
    let written = file.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ApiError> {
        use std::io::{ErrorKind, Write};
        let opened = if part == 0 {
            std::fs::create_dir_all(&folder).map_err(|_| {
                ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Could not create the Videos folder.".into())
            })?;
            std::fs::OpenOptions::new().write(true).create_new(true).open(&file)
        } else {
            std::fs::OpenOptions::new().append(true).open(&file)
        };
        let mut handle = opened.map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => ApiError(StatusCode::CONFLICT, "A recording with this name already exists.".into()),
            ErrorKind::NotFound => ApiError(StatusCode::NOT_FOUND, "Recording not started.".into()),
            _ => ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Could not save the recording.".into()),
        })?;
        handle
            .write_all(&body)
            .and_then(|_| handle.flush())
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Could not save the recording. Is the disk full?".into()))
    })
    .await
    .map_err(internal)??;
    state.recording_parts.lock().unwrap().insert(written, part + 1);
    Ok(Json(serde_json::json!({
        "ok": true,
        "file": name,
        "folder": state.recordings.display().to_string(),
    })))
}
/// A paired tablet saves a script it edited. The same revision check as the
/// editor applies, so a tablet can never overwrite newer PC edits.
async fn device_save(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(script): Json<Script>,
) -> Result<Json<Script>, ApiError> {
    device(&headers, &state)?;
    if id != script.id {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Script ID mismatch".into(),
        ));
    }
    Ok(Json(state.store.save(script)?))
}
async fn device_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    device(&headers, &state)?;
    let revision = query
        .get("revision")
        .and_then(|v| v.parse().ok())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "Missing revision".into()))?;
    state.store.delete(&id, revision)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
#[derive(Deserialize)]
struct PairInput {
    code: String,
}
async fn pair(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<PairInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    {
        let mut pairing = state.pairing.lock().unwrap();
        pairing
            .attempts
            .retain(|_, v| v.last().is_some_and(|t| t.elapsed().as_secs() < 60));
        let history = pairing.attempts.entry(peer.ip().to_string()).or_default();
        history.retain(|t| t.elapsed().as_secs() < 60);
        if history.len() >= 6 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Wait a minute before trying again.".into(),
            ));
        }
        history.push(Instant::now());
        if body.code.len() > 20 || !equal(body.code.trim(), &pairing.code) {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Pairing code does not match.".into(),
            ));
        }
    }
    let token = secret();
    state.store.pair(&token).map_err(internal)?;
    let lib = state.store.library().map_err(internal)?;
    Ok(Json(
        serde_json::json!({"token":token,"generation":lib.generation,"scripts":lib.scripts}),
    ))
}
async fn library(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Library>, ApiError> {
    device(&headers, &state)?;
    Ok(Json(state.store.library().map_err(internal)?))
}
async fn unpair(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    editor(peer, &headers, &state)?;
    state.store.unpair().map_err(internal)?;
    state.pairing.lock().unwrap().code = format!("{:06}", rand::random::<u32>() % 1_000_000);
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn ws(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let token = device(&headers, &state)?;
    Ok(upgrade
        .max_message_size(4096)
        .on_upgrade(move |socket| websocket(socket, state, token)))
}
async fn websocket(mut socket: WebSocket, state: Arc<AppState>, token: String) {
    let id = rand::random::<u64>();
    state.tablets.lock().unwrap().insert(id, Tablet::default());
    let mut generation = None;
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(750));
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if !state.store.is_device(&token) { let _ = socket.send(Message::Close(Some(axum::extract::ws::CloseFrame { code: 4401, reason: "Pairing revoked".into() }))).await; break; }
                let Ok(library) = state.store.library() else { break; };
                let message = if generation != Some(library.generation) { generation = Some(library.generation); serde_json::json!({"type":"library","generation":library.generation,"scripts":library.scripts}) } else { serde_json::json!({"type":"heartbeat"}) };
                if !matches!(tokio::time::timeout(std::time::Duration::from_secs(5), socket.send(Message::Text(message.to_string().into()))).await, Ok(Ok(_))) { break; }
            }
            value = socket.recv() => {
                match value {
                    Some(Ok(Message::Text(text))) => if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) { if value["type"] == "status" { state.tablets.lock().unwrap().insert(id, Tablet { phase:value["phase"].as_str().unwrap_or("").chars().take(80).collect(), script:value["script"].as_str().unwrap_or("").chars().take(200).collect(), pending:value["pending"].as_bool().unwrap_or(false) }); } },
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    state.tablets.lock().unwrap().remove(&id);
}
/// The PC reader's microphone bridge: the browser sends the same start, stop,
/// reset and grammar commands the Android host gets, and receives `voice`,
/// `speech`, `audio_error` and `grammar_error` events.
async fn voice_socket(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
) -> Result<Response, ApiError> {
    // Authenticate before looking at the upgrade, so strangers always get 403.
    editor(peer, &headers, &state)?;
    if headers.get("origin").is_none() {
        return Err(ApiError(StatusCode::FORBIDDEN, "Wrong editor origin".into()));
    }
    let upgrade = match upgrade {
        Ok(upgrade) => upgrade,
        Err(rejection) => return Ok(rejection.into_response()),
    };
    state.voice.ensure_loaded();
    Ok(upgrade
        .max_message_size(1_000_000)
        .on_upgrade(move |socket| voice_session(socket, state)))
}
async fn voice_session(mut socket: WebSocket, state: Arc<AppState>) {
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let take = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut listener: Option<voice::Listener> = None;
    let mut grammar: Option<String> = None;
    let mut wanted = false;
    let mut status = None;
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(400));
    loop {
        let event = tokio::select! {
            _ = ticker.tick() => {
                let now = state.voice.status_event();
                if status.as_ref() == Some(&now) { continue; }
                status = Some(now.clone());
                now
            }
            Some(event) = received.recv() => event,
            value = socket.recv() => {
                let Some(Ok(Message::Text(text))) = value else {
                    if matches!(value, Some(Ok(_))) { continue; }
                    break;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let payload = value["payload"].as_str().unwrap_or("");
                if let Ok(tag) = payload.trim().parse() { take.store(tag, std::sync::atomic::Ordering::SeqCst); }
                match value["command"].as_str().unwrap_or("") {
                    "start" => wanted = true,
                    "stop" => { wanted = false; close_listener(listener.take()); }
                    "reset" => if let Some(l) = &listener { l.reset(); },
                    "grammar" => {
                        grammar = Some(payload.to_string()).filter(|g| !g.trim().is_empty());
                        close_listener(listener.take());
                    }
                    _ => {}
                }
                // A microphone that failed for good stays off until the next start.
                if value["command"] == "start" && listener.as_ref().is_some_and(|l| !l.alive()) {
                    listener = None;
                }
                if wanted && listener.is_none() && value["command"] != "reset" {
                    let voice = state.voice.clone();
                    let (g, t, e) = (grammar.clone(), take.clone(), events.clone());
                    listener = tokio::task::spawn_blocking(move || voice.listen(g, t, e)).await.ok().flatten();
                }
                continue;
            }
        };
        if event["type"] == "audio_error" {
            wanted = false;
        }
        if socket.send(Message::Text(event.to_string().into())).await.is_err() {
            break;
        }
    }
    // Closing the studio tab closes the microphone.
    close_listener(listener);
}
/// Closing the microphone joins its thread, so it runs off the async workers.
fn close_listener(listener: Option<voice::Listener>) {
    if listener.is_some() {
        tokio::task::spawn_blocking(move || drop(listener));
    }
}
