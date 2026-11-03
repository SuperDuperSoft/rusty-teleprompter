"""Build RustyPrompter's Rust companion and offline Android host."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import secrets
import shutil
import subprocess
import sys
import zipfile
from task import ROOT, TOOLS, env

parser=argparse.ArgumentParser()
parser.add_argument('--skip-ui',action='store_true')
parser.add_argument('--android-only',action='store_true')
parser.add_argument('--windows-only',action='store_true')
parser.add_argument('--skip-tests',action='store_true')
args=parser.parse_args()
DIST=ROOT/'dist';DIST.mkdir(exist_ok=True)

def run(command,cwd=ROOT):
    command=[str(x) for x in command]
    command[0]=shutil.which(command[0],path=env['PATH']) or command[0]
    subprocess.run(command,cwd=cwd,env=env,check=True)

def zip_folder(source,target):
    with zipfile.ZipFile(target,'w',zipfile.ZIP_DEFLATED,strict_timestamps=False)as archive:
        for path in source.rglob('*'):
            if path.is_file():archive.write(path,source.name+'/'+path.relative_to(source).as_posix())

if not args.skip_tests:run(['cargo','test','-p','rusty-core','-p','rusty-companion'])
if not args.skip_ui:
    from bundle_ui import bundle
    bundle()
web=ROOT/'web-dist'
if not (web/'index.html').exists():raise RuntimeError('Build the Dioxus UI first; web-dist/index.html is missing.')
if not args.android_only:
    run(['cargo','build','-p','rusty-companion','--release'])
    windows=DIST/'RustyPrompter';windows.mkdir(exist_ok=True)
    shutil.copy2(ROOT/'target/release/rusty-companion.exe',windows/'RustyPrompter.exe')
    shutil.copytree(web,windows/'web',dirs_exist_ok=True)
    for name in ['INSTALL.md','README.md','THIRD_PARTY_NOTICES.md','VALIDATION.md','LICENSE','NOTICE']:
        if (ROOT/name).exists():shutil.copy2(ROOT/name,windows/name)
    if (ROOT/'licenses').exists():shutil.copytree(ROOT/'licenses',windows/'licenses',dirs_exist_ok=True)
    zip_folder(windows,DIST/'RustyPrompter-Windows.zip')

if not args.windows_only:
    shared=ROOT.parent/'.tools'
    for path in ['jdk/bin/java.exe','android-sdk/build-tools/35.0.0','gradle-8.11.1/bin/gradle.bat']:
        if not (shared/path).exists():raise RuntimeError('Prepare Android SDK/JDK/Gradle first. See INSTALL.md development instructions.')
    env['JAVA_HOME']=str(shared/'jdk');env['ANDROID_HOME']=str(shared/'android-sdk');env['GRADLE_USER_HOME']=str(shared/'gradle-cache')
    env['PATH']=str(shared/'jdk/bin')+os.pathsep+env['PATH']
    (ROOT/'android/local.properties').write_text('sdk.dir='+str(shared/'android-sdk').replace('\\','/')+'\n')
    assets=ROOT/'android/app/src/main/assets';assets.mkdir(parents=True,exist_ok=True)
    destination=assets/'web'
    if destination.exists():
        resolved=destination.resolve()
        assert resolved.is_relative_to(ROOT.resolve()) and resolved.name=='web'
        shutil.rmtree(resolved)
    shutil.copytree(web,destination)
    model=assets/'model-en-us'
    if not (model/'am/final.mdl').exists():
        old=ROOT.parent/'android/app/src/main/assets/model-en-us'
        if old.exists():shutil.copytree(old,model,dirs_exist_ok=True)
        else:
            archive=TOOLS/'vosk-model.zip'
            run(['curl.exe','-fLsS','--retry','2','https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip','-o',archive])
            with zipfile.ZipFile(archive) as z:z.extractall(TOOLS/'model')
            shutil.copytree(TOOLS/'model/vosk-model-small-en-us-0.15',model)
    notices=assets/'notices';notices.mkdir(exist_ok=True)
    for name in ['THIRD_PARTY_NOTICES.md','LICENSE','NOTICE']:
        if (ROOT/name).exists():shutil.copy2(ROOT/name,notices/name)
    if (ROOT/'licenses').exists():shutil.copytree(ROOT/'licenses',notices/'licenses',dirs_exist_ok=True)
    signing=ROOT/'.signing';signing.mkdir(exist_ok=True);config=signing/'config.json'
    if not config.exists():config.write_text(json.dumps({'password':secrets.token_urlsafe(32)},indent=2))
    env['RUSTYPROMPTER_STORE_PASSWORD']=json.loads(config.read_text())['password']
    if not (signing/'rustyprompter.jks').exists():run([shared/'jdk/bin/keytool.exe','-genkeypair','-keystore',signing/'rustyprompter.jks','-storepass:env','RUSTYPROMPTER_STORE_PASSWORD','-keypass:env','RUSTYPROMPTER_STORE_PASSWORD','-alias','rustyprompter','-keyalg','RSA','-keysize','3072','-validity','10000','-dname','CN=RustyPrompter local application'])
    run([shared/'gradle-8.11.1/bin/gradle.bat','--no-daemon','--console=plain','lintRelease','assembleRelease'],ROOT/'android')
    shutil.copy2(ROOT/'android/app/build/outputs/apk/release/app-release.apk',DIST/'RustyPrompter-Android.apk')
    with zipfile.ZipFile(DIST/'RustyPrompter-Signing-Backup.zip','w',zipfile.ZIP_DEFLATED)as z:
        for path in signing.iterdir():
            if path.is_file():z.write(path,path.name)
print('RustyPrompter packages built in '+str(DIST),flush=True)
