use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=assets/rustyprompter.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    let (mut compiler, resource) = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let resource = out_dir.join("icon.res");
        let mut compiler = Command::new("rc.exe");
        compiler.arg("/nologo").arg("/fo").arg(&resource).arg("assets/icon.rc");
        (compiler, resource)
    } else {
        let resource = out_dir.join("icon.o");
        let mut compiler = Command::new("windres");
        compiler.args(["-i", "assets/icon.rc", "-O", "coff", "-o"]).arg(&resource);
        (compiler, resource)
    };
    let status = compiler.status().expect("Run the Windows resource compiler");
    assert!(status.success(), "Compile the RustyPrompter icon");
    println!("cargo:rustc-link-arg-bin=rusty-companion={}", resource.display());
}
