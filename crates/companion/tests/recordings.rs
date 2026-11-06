use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use rusty_companion::*;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tower::ServiceExt;

fn setup() -> (tempfile::TempDir, Arc<AppState>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let web = dir.path().join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<html>RustyPrompter</html>").unwrap();
    let mut app = AppState::new(dir.path(), web, 8766).unwrap();
    let videos = dir.path().join("Videos").join("RustyPrompter");
    Arc::get_mut(&mut app).unwrap().recordings = videos.clone();
    (dir, app, videos)
}

async fn upload(state: Arc<AppState>, path: &str, bytes: &[u8], auth: bool, remote: bool) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("Host", "127.0.0.1:8766")
        .header("Origin", "http://127.0.0.1:8766")
        .header("Content-Type", "application/octet-stream");
    if auth {
        builder = builder.header("Cookie", format!("rusty_editor={}", state.editor_key));
    }
    let mut req = builder.body(Body::from(bytes.to_vec())).unwrap();
    let peer: SocketAddr = if remote { "192.168.1.8:1234" } else { "127.0.0.1:1234" }.parse().unwrap();
    req.extensions_mut().insert(ConnectInfo(peer));
    router(state).oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn parts_are_written_in_order_into_one_file() {
    let (_dir, app, videos) = setup();
    let base = "/api/recordings/RustyPrompter_Test_2026-11-05_19-58-03?ext=mp4";
    assert_eq!(upload(app.clone(), &format!("{base}&part=0"), b"one-", true, false).await, StatusCode::OK);
    assert_eq!(upload(app.clone(), &format!("{base}&part=1"), b"two", true, false).await, StatusCode::OK);
    let file = videos.join("RustyPrompter_Test_2026-11-05_19-58-03.mp4");
    assert_eq!(std::fs::read(file).unwrap(), b"one-two");
}

#[tokio::test]
async fn first_part_never_overwrites_an_existing_video() {
    let (_dir, app, videos) = setup();
    std::fs::create_dir_all(&videos).unwrap();
    std::fs::write(videos.join("Take.webm"), b"keep").unwrap();
    let status = upload(app, "/api/recordings/Take?ext=webm&part=0", b"new", true, false).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(std::fs::read(videos.join("Take.webm")).unwrap(), b"keep");
}

#[tokio::test]
async fn later_parts_need_the_first_part() {
    let (_dir, app, videos) = setup();
    let status = upload(app, "/api/recordings/Take?ext=mp4&part=3", b"x", true, false).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!videos.join("Take.mp4").exists());
}

#[tokio::test]
async fn unsafe_names_and_extensions_are_rejected() {
    let (_dir, app, _videos) = setup();
    for path in [
        "/api/recordings/..%2Fevil?ext=mp4&part=0",
        "/api/recordings/Take.exe?ext=mp4&part=0",
        "/api/recordings/Take?ext=exe&part=0",
        "/api/recordings/Take?ext=mp4",
    ] {
        let status = upload(app.clone(), path, b"x", true, false).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn only_the_local_editor_can_save_videos() {
    let (_dir, app, videos) = setup();
    let path = "/api/recordings/Take?ext=mp4&part=0";
    assert_eq!(upload(app.clone(), path, b"x", false, false).await, StatusCode::FORBIDDEN);
    assert_eq!(upload(app.clone(), path, b"x", true, true).await, StatusCode::FORBIDDEN);
    assert!(!videos.join("Take.mp4").exists());
}

#[tokio::test]
async fn chunks_larger_than_the_script_limit_are_accepted() {
    let (_dir, app, videos) = setup();
    let chunk = vec![7u8; 5_000_000];
    let status = upload(app, "/api/recordings/Big?ext=mp4&part=0", &chunk, true, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::metadata(videos.join("Big.mp4")).unwrap().len(), 5_000_000);
}

#[tokio::test]
async fn parts_out_of_order_or_repeated_are_refused() {
    let (_dir, app, videos) = setup();
    let base = "/api/recordings/Order?ext=mp4";
    assert_eq!(upload(app.clone(), &format!("{base}&part=0"), b"a", true, false).await, StatusCode::OK);
    assert_eq!(upload(app.clone(), &format!("{base}&part=2"), b"c", true, false).await, StatusCode::CONFLICT);
    assert_eq!(upload(app.clone(), &format!("{base}&part=1"), b"b", true, false).await, StatusCode::OK);
    assert_eq!(upload(app.clone(), &format!("{base}&part=1"), b"b", true, false).await, StatusCode::CONFLICT);
    assert_eq!(std::fs::read(videos.join("Order.mp4")).unwrap(), b"ab");
}

#[tokio::test]
async fn a_finished_file_from_before_a_restart_is_never_appended_to() {
    let (_dir, app, videos) = setup();
    std::fs::create_dir_all(&videos).unwrap();
    std::fs::write(videos.join("Old.mp4"), b"done").unwrap();
    let status = upload(app, "/api/recordings/Old?ext=mp4&part=1", b"x", true, false).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(std::fs::read(videos.join("Old.mp4")).unwrap(), b"done");
}
