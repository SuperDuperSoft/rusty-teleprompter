"""Compile Dioxus with Cargo and package with upstream wasm-bindgen.

Using explicit assets avoids the Dioxus CLI's Windows bundle path failure.
The resulting UI is the same Rust/Dioxus web renderer, fully offline.
"""
from pathlib import Path
import json
import shutil
import subprocess
import tarfile
import tomllib
from task import ROOT, TOOLS, env

def run(command):
    command=[str(x) for x in command]
    command[0]=shutil.which(command[0],path=env['PATH'])or command[0]
    subprocess.run(command,cwd=ROOT,env=env,check=True)

def bundle():
    run(['cargo','build','-p','rusty-ui','--target','wasm32-unknown-unknown','--profile','wasm-release','--locked'])
    lock=tomllib.loads((ROOT/'Cargo.lock').read_text())
    version=next(p['version']for p in lock['package']if p['name']=='wasm-bindgen')
    bindgen=TOOLS/f'wasm-bindgen-{version}/wasm-bindgen.exe'
    if not bindgen.exists():
        cached=Path.home()/f'.dx/tools/wasm-bindgen-{version}/wasm-bindgen.exe'
        if cached.exists():
            bindgen.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(cached,bindgen)
        else:
            archive=TOOLS/f'wasm-bindgen-{version}.tar.gz'
            url=f'https://github.com/wasm-bindgen/wasm-bindgen/releases/download/{version}/wasm-bindgen-{version}-x86_64-pc-windows-msvc.tar.gz'
            run(['curl.exe','-fLsS','--retry','2',url,'-o',archive])
            with tarfile.open(archive)as source:
                member=next(m for m in source.getmembers()if m.name.endswith('/wasm-bindgen.exe'))
                bindgen.parent.mkdir(parents=True,exist_ok=True)
                with source.extractfile(member)as input,bindgen.open('wb')as output:shutil.copyfileobj(input,output)
    output=ROOT/'web-dist';assets=output/'assets';assets.mkdir(parents=True,exist_ok=True)
    run([bindgen,'--target','web','--out-dir',assets,'--out-name','rusty-ui','--no-typescript',ROOT/'target/wasm32-unknown-unknown/wasm-release/rusty-ui.wasm'])
    shutil.copy2(ROOT/'crates/ui/assets/main.css',assets/'main.css')
    (output/'index.html').write_text('''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark"><title>RustyPrompter · Script studio</title><link rel="stylesheet" href="/assets/main.css"></head><body><div id="main"></div><script type="module">import init from '/assets/rusty-ui.js';init().catch(error=>{document.getElementById('main').textContent='RustyPrompter could not load. Please update Android System WebView or reopen the PC companion. '+error;});</script></body></html>''',encoding='utf-8')
    print('Offline Dioxus bundle: '+str(output),flush=True)

if __name__=='__main__':bundle()
