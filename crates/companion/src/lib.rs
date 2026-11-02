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

const DATA: TableDefinition<&str, &str> = TableDefinition::new("library");
const DEVICES: TableDefinition<&str, bool> = TableDefinition::new("devices");

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
}
impl AppState {
    pub fn new(folder: &std::path::Path, web: PathBuf, port: u16) -> anyhow::Result<Arc<Self>> {
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
