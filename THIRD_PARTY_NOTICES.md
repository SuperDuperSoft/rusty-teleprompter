# RustyPrompter third-party notices

RustyPrompter is a new application using open-source components. Full upstream licenses and the exact locked versions are included in `licenses/`.

## Rust application and interface

- **Dioxus 0.7.10**, Dioxus Labs and contributors — MIT OR Apache-2.0.
- **Rust standard library** and **wasm-bindgen**, their respective contributors — MIT OR Apache-2.0.
- **Axum**, **Tokio**, **redb**, **serde**, **serde_json**, **regex**, **tower-http**, **reqwest**, and other Rust dependencies — the upstream license expressions and copied license files are listed in `licenses/rust/INDEX.json`.
- The Dioxus interface is compiled to WebAssembly and included in the APK and Windows ZIP. No Dioxus hosting product is required.

## Android platform adapter

- **Vosk Android API 0.3.75**, Alpha Cephei Inc. — Apache-2.0.
- **vosk-model-small-en-us-0.15**, Alpha Cephei Inc. — Apache-2.0, as listed at https://alphacephei.com/vosk/models.
- **JNA 5.18.1**, JNA contributors — distributed under its Apache-2.0 option.
- **Java-WebSocket 1.5.7** and **SLF4J API**, their respective contributors — MIT.
- **Kotlin standard library**, JetBrains and contributors — Apache-2.0.

Vosk and the English model are used unmodified. The phrase matcher, slide state machine, script editor and application interface are implemented independently in Rust. RustyPrompter is not affiliated with PromptSmart, Samsung, Dioxus Labs or Alpha Cephei.

## Build tools

Rustup, Cargo, Dioxus CLI, wasm-bindgen CLI, w64devkit, OpenJDK, Gradle and the Android SDK are development tools, not subscriptions or hosted runtime services. Python scripts only orchestrate development builds; no Python runtime is distributed with the app. Developer tools retain their own licenses and are not included in the application archives.
