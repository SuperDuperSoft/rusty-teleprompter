#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus::launch(app::App);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("Build the shared Dioxus interface using dx build --platform web.");
}
