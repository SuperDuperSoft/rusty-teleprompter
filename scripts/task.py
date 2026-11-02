"""Run a build tool with isolated Cargo/Rust/linker paths."""
from pathlib import Path
import os
import subprocess
import sys
import shutil
ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT.parent / '.tools/rustyprompter'
env = os.environ.copy()
env['CARGO_HOME'] = str(TOOLS / 'cargo')
env['RUSTUP_HOME'] = str(TOOLS / 'rustup')
env['PATH'] = os.pathsep.join([str(TOOLS / 'cargo/bin'), str(TOOLS / 'w64devkit/bin'), str(TOOLS / 'dx'), env.get('PATH', '')])
env['CARGO_BUILD_JOBS'] = '2'
env['CARGO_TERM_COLOR'] = 'never'
env['CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER'] = str(TOOLS / 'w64devkit/bin/gcc.exe')
env['CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS'] = '-C link-self-contained=yes'
runtime = next((TOOLS / 'rustup/toolchains').rglob('libgcc_eh.a')).parent
env['LIBRARY_PATH'] = str(runtime)
if __name__ == '__main__':
    command = sys.argv[1:]
    command[0] = shutil.which(command[0], path=env['PATH']) or command[0]
    sys.exit(subprocess.call(command, cwd=ROOT, env=env))
