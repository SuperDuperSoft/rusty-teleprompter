#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use rusty_companion::{router, AppState};
use std::path::PathBuf;

#[cfg(windows)]
mod window;
fn arg(name: &str) -> Option<String> {
    std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|p| p[0] == name)
        .map(|p| p[1].clone())
}

#[tokio::main]
async fn main() {
    if let Some(report) = arg("--self-test") {
        let result = smoke().await;
        let json = match result {
            Ok(()) => {
                serde_json::json!({"ok":true,"checks":["native Rust runtime","redb persistence","editor authentication","Dioxus assets","pairing","revocation"]})
            }
            Err(e) => serde_json::json!({"ok":false,"error":format!("{e:#}")}),
        };
        let _ = std::fs::write(report, json.to_string());
        return;
    }
    if let Err(e) = run().await {
        #[cfg(windows)] window::error(&format!("RustyPrompter could not start.\n\n{e:#}\n\nIf it is already running, close the other companion window first."));
        #[cfg(not(windows))]
        eprintln!("{e:#}");
    }
}
fn web_folder() -> PathBuf {
    arg("--web").map(PathBuf::from).unwrap_or_else(|| {
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("web")
    })
}
async fn run() -> anyhow::Result<()> {
    let port = arg("--port")
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8766);
    let host = if std::env::args().any(|a| a == "--headless") {
        "127.0.0.1"
    } else {
        "0.0.0.0"
    };
    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    let folder = arg("--data").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
            .join("RustyPrompter")
    });
    let state = AppState::new(&folder, web_folder(), port)?;
    if let Some(file) = arg("--ready-file") {
        std::fs::write(
            file,
            serde_json::json!({"url":state.launch_url(),"port":port}).to_string(),
        )?;
    }
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    if std::env::args().any(|a| a == "--headless") {
        server.await??;
    } else {
        webbrowser::open(&state.launch_url())?;
        #[cfg(windows)]
        window::run(state);
        #[cfg(not(windows))]
        {
            tokio::signal::ctrl_c().await?;
        }
        server.abort();
    }
    Ok(())
}
async fn smoke() -> anyhow::Result<()> {
    let folder =
        std::env::temp_dir().join(format!("rustyprompter-test-{}", rusty_companion::secret()));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let state = AppState::new(&folder, web_folder(), port)?;
    let mut demo = rusty_core::demo();
    demo.revision = 0;
    let script = state
        .store
        .save(demo)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    // A new script must begin at revision zero; demo's portable revision is reset here.
    let app = router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let base = format!("http://127.0.0.1:{port}");
    let login = client.get(state.launch_url()).send().await?;
    anyhow::ensure!(login.status() == 303, "editor login failed");
    let cookie = login
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()?
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let html = client
        .get(&base)
        .header("Cookie", &cookie)
        .send()
        .await?
        .text()
        .await?;
    anyhow::ensure!(
        html.contains("RustyPrompter"),
        "bundled Dioxus page missing"
    );
    let saved: serde_json::Value = client
        .get(format!("{base}/api/state"))
        .header("Cookie", &cookie)
        .send()
        .await?
        .json()
        .await?;
    anyhow::ensure!(
        saved["scripts"][0]["id"] == script.id,
        "persistent script missing"
    );
    let code = state.pairing.lock().unwrap().code.clone();
    let pair: serde_json::Value = client
        .post(format!("{base}/api/pair"))
        .json(&serde_json::json!({"code":code}))
        .send()
        .await?
        .json()
        .await?;
    let token = pair["token"].as_str().unwrap();
    anyhow::ensure!(
        client
            .get(format!("{base}/api/library"))
            .bearer_auth(token)
            .send()
            .await?
            .status()
            == 200,
        "pairing failed"
    );
    client
        .post(format!("{base}/api/unpair"))
        .header("Cookie", &cookie)
        .send()
        .await?;
    anyhow::ensure!(
        client
            .get(format!("{base}/api/library"))
            .bearer_auth(token)
            .send()
            .await?
            .status()
            == 401,
        "revocation failed"
    );
    server.abort();
    drop(state);
    let _ = std::fs::remove_dir_all(folder);
    Ok(())
}
