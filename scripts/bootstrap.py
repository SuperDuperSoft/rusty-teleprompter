"""Project-local Rust/Dioxus build tools. No global PATH or Rust install changes."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import zipfile
import hashlib

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT.parent / '.tools/rustyprompter'
TOOLS.mkdir(parents=True, exist_ok=True)

def download(url, name):
    target = TOOLS / name
    if not target.exists():
        print('Downloading ' + name, flush=True)
        partial = target.with_suffix(target.suffix + '.part')
        subprocess.run(['curl.exe', '-fLsS', '--retry', '2', '--max-time', '300', url, '-o', str(partial)], check=True)
        partial.replace(target)
    return target

env = os.environ.copy()
env['CARGO_HOME'] = str(TOOLS / 'cargo')
env['RUSTUP_HOME'] = str(TOOLS / 'rustup')
env['PATH'] = str(TOOLS / 'cargo/bin') + os.pathsep + env.get('PATH', '')
rustup = TOOLS / 'cargo/bin/rustup.exe'
if not rustup.exists():
    installer = download('https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe', 'rustup-init.exe')
    subprocess.run([str(installer), '-y', '--no-modify-path', '--profile', 'minimal', '--default-host', 'x86_64-pc-windows-gnu', '--default-toolchain', 'stable'], env=env, check=True)
subprocess.run([str(rustup), 'target', 'add', 'wasm32-unknown-unknown'], env=env, check=True)
subprocess.run([str(rustup), 'component', 'add', 'rustfmt', 'clippy'], env=env, check=True)

if not (TOOLS / 'w64devkit/bin/gcc.exe').exists():
    archive = download('https://github.com/skeeto/w64devkit/releases/download/v2.11.0/w64devkit-x64-2.11.0.7z.exe', 'w64devkit.7z.exe')
    subprocess.run([str(archive), '-y', '-o' + str(TOOLS)], check=True)

if not (TOOLS / 'dx/dx.exe').exists():
    url = 'https://github.com/DioxusLabs/dioxus/releases/download/v0.7.10/dx-x86_64-pc-windows-msvc'
    archive = download(url + '.zip', 'dioxus-cli.zip')
    expected = download(url + '.sha256', 'dioxus-cli.sha256').read_text().split()[0]
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected, 'Dioxus CLI checksum mismatch'
    with zipfile.ZipFile(archive) as z:
        z.extractall(TOOLS / 'dx')
print('Rust and Dioxus build tools ready.', flush=True)
