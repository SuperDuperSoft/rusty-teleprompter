use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rusty_companion::*;
use rusty_core::*;
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tower::ServiceExt;

fn setup() -> (tempfile::TempDir, Arc<AppState>) {
    let dir = tempfile::tempdir().unwrap();
    let web = dir.path().join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<html>RustyPrompter</html>").unwrap();
    std::fs::write(web.join("assets/app.wasm"), b"test-wasm").unwrap();
    let app = AppState::new(dir.path(), web, 8766).unwrap();
    (dir, app)
}
fn sample() -> Script {
    let mut s = demo();
    s.revision = 0;
    s
}
async fn request(
    state: Arc<AppState>,
    method: &str,
    path: &str,
    body: Option<Value>,
    auth: bool,
    remote: bool,
    origin: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "127.0.0.1:8766")
        .header("Content-Type", "application/json");
    if auth {
        builder = builder.header("Cookie", format!("rusty_editor={}", state.editor_key));
    }
    if let Some(origin) = origin {
        builder = builder.header("Origin", origin);
    }
    let mut req = builder
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap();
    let peer: SocketAddr = if remote {
        "192.168.1.8:1234"
    } else {
        "127.0.0.1:1234"
    }
    .parse()
    .unwrap();
    req.extensions_mut().insert(ConnectInfo(peer));
    let response = router(state).oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}
#[tokio::test]
async fn editor_needs_launch_cookie() {
    let (_d, s) = setup();
    assert_eq!(
        request(s.clone(), "GET", "/api/state", None, false, false, None)
            .await
            .0,
        403
    );
    assert_eq!(
        request(s, "GET", "/api/state", None, true, false, None)
            .await
            .0,
        200
    );
}
#[tokio::test]
async fn launch_link_local_only() {
    let (_d, s) = setup();
    let path = format!("/?key={}", s.editor_key);
    assert_eq!(
        request(s.clone(), "GET", &path, None, false, false, None)
            .await
            .0,
        303
    );
    assert_eq!(
        request(s, "GET", &path, None, false, true, None).await.0,
        403
    );
}
#[tokio::test]
async fn remote_cannot_open_editor() {
    let (_d, s) = setup();
    assert_eq!(
        request(s.clone(), "GET", "/api/state", None, true, true, None)
            .await
            .0,
        403
    );
    assert_eq!(
        request(s, "GET", "/assets/app.wasm", None, false, true, None)
            .await
            .0,
        403
    );
}
#[tokio::test]
async fn device_requires_auth() {
    let (_d, s) = setup();
    assert_eq!(
        request(s, "GET", "/api/library", None, false, false, None)
            .await
            .0,
        401
    );
}
#[tokio::test]
async fn save_and_revision_protection() {
    let (_d, s) = setup();
    let sample = sample();
    let path = format!("/api/scripts/{}", sample.id);
    let (status, value) = request(
        s.clone(),
        "PUT",
        &path,
        Some(json!(sample)),
        true,
        false,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["revision"], 1);
    assert_eq!(
        request(s, "PUT", &path, Some(json!(sample)), true, false, None)
            .await
            .0,
        409
    );
}
#[tokio::test]
async fn edits_preserve_library_order() {
    let (_d, s) = setup();
    let saved = s.store.save(sample()).unwrap();
    let mut next = sample();
    next.id = "other".into();
    s.store.save(next).unwrap();
    s.store.save(saved).unwrap();
    assert_eq!(s.store.library().unwrap().scripts[0].id, "rusty-demo");
}
#[tokio::test]
async fn duplicate_slide_ids_rejected() {
    let (_d, s) = setup();
    let mut sample = sample();
    sample.slides[1].id = sample.slides[0].id.clone();
    assert_eq!(
        request(
            s,
            "PUT",
            "/api/scripts/rusty-demo",
            Some(json!(sample)),
            true,
            false,
            None
        )
        .await
        .0,
        422
    );
}
#[tokio::test]
async fn wrong_script_id_rejected() {
    let (_d, s) = setup();
    assert_eq!(
        request(
            s,
            "PUT",
            "/api/scripts/another",
            Some(json!(sample())),
            true,
            false,
            None
        )
        .await
        .0,
        400
    );
}
#[tokio::test]
async fn cross_origin_edit_denied() {
    let (_d, s) = setup();
    assert_eq!(
        request(
            s,
            "PUT",
            "/api/scripts/rusty-demo",
            Some(json!(sample())),
            true,
            false,
            Some("https://unrelated.example")
        )
        .await
        .0,
        403
    );
}
#[tokio::test]
async fn same_origin_edit_allowed() {
    let (_d, s) = setup();
    assert_eq!(
        request(
            s,
            "PUT",
            "/api/scripts/rusty-demo",
            Some(json!(sample())),
            true,
            false,
            Some("http://127.0.0.1:8766")
        )
        .await
        .0,
        200
    );
}
#[tokio::test]
async fn deletion_requires_latest_revision() {
    let (_d, s) = setup();
    s.store.save(sample()).unwrap();
    assert_eq!(
        request(
            s.clone(),
            "DELETE",
            "/api/scripts/rusty-demo?revision=0",
            None,
            true,
            false,
            None
        )
        .await
        .0,
        409
    );
    assert_eq!(
        request(
            s.clone(),
            "DELETE",
            "/api/scripts/rusty-demo?revision=1",
            None,
            true,
            false,
            None
        )
        .await
        .0,
        200
    );
    assert!(s.store.library().unwrap().scripts.is_empty());
}
#[tokio::test]
async fn pairing_and_revocation() {
    let (_d, s) = setup();
    let code = s.pairing.lock().unwrap().code.clone();
    let (status, value) = request(
        s.clone(),
        "POST",
        "/api/pair",
        Some(json!({"code":code})),
        false,
        false,
        None,
    )
    .await;
    assert_eq!(status, 200);
    let token = value["token"].as_str().unwrap();
    assert!(s.store.is_device(token));
    assert_eq!(
        request(s.clone(), "POST", "/api/unpair", None, true, false, None)
            .await
            .0,
        200
    );
    assert!(!s.store.is_device(token));
}
#[tokio::test]
async fn pairing_attempts_are_limited() {
    let (_d, s) = setup();
    for _ in 0..6 {
        assert_eq!(
            request(
                s.clone(),
                "POST",
                "/api/pair",
                Some(json!({"code":"wrong"})),
                false,
                false,
                None
            )
            .await
            .0,
            403
        );
    }
    assert_eq!(
        request(
            s,
            "POST",
            "/api/pair",
            Some(json!({"code":"wrong"})),
            false,
            false,
            None
        )
        .await
        .0,
        429
    );
}
#[test]
fn data_and_pairing_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = Store::open(dir.path()).unwrap();
        store.save(sample()).unwrap();
        store.pair("local-device-token").unwrap();
    }
    let reopened = Store::open(dir.path()).unwrap();
    assert_eq!(reopened.library().unwrap().scripts.len(), 1);
    assert!(reopened.is_device("local-device-token"));
}
#[test]
fn concurrent_saves_do_not_overwrite() {
    let (_d, s) = setup();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let b = barrier.clone();
            let state = s.clone();
            std::thread::spawn(move || {
                b.wait();
                state.store.save(sample()).is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|t| t.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
}
#[test]
fn oversized_slide_rejected() {
    let mut s = sample();
    s.slides[0].text = "a".repeat(100001);
    assert!(validate(&s).is_err());
}
#[test]
fn unicode_and_stage_directions_persist() {
    let (_d, s) = setup();
    let mut script = sample();
    script.slides[0].text = "Hello [look at camera]. It’s a beautiful day! café".into();
    s.store.save(script.clone()).unwrap();
    assert_eq!(
        s.store.library().unwrap().scripts[0].slides[0].text,
        script.slides[0].text
    );
}

#[tokio::test]
async fn real_websocket_sync_and_revocation() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    let (_d, s) = setup();
    s.store.pair("test-token").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = router(s.clone()).into_make_service_with_connect_info::<SocketAddr>();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut req = format!("ws://{address}/ws").into_client_request().unwrap();
    req.headers_mut()
        .insert("Authorization", "Bearer test-token".parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    let message = ws.next().await.unwrap().unwrap();
    let library: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
    assert_eq!(library["type"], "library");
    assert_eq!(library["scripts"], json!([]));
    s.store.save(sample()).unwrap();
    ws.send(Message::Text(
        json!({"type":"status","phase":"Reading","script":"Studio","pending":true})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let update = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let m = ws.next().await.unwrap().unwrap();
            if let Ok(t) = m.to_text() {
                let v: Value = serde_json::from_str(t).unwrap();
                if v["type"] == "library" {
                    break v;
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(update["scripts"][0]["revision"], 1);
    assert!(s.tablets.lock().unwrap().values().any(|t| t.pending));
    s.store.unpair().unwrap();
    let close = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let m = ws.next().await.unwrap().unwrap();
            if let Message::Close(frame) = m {
                break frame;
            }
        }
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(u16::from(close.code), 4401);
    drop(ws);
    server.abort();
    let _ = server.await;
}
