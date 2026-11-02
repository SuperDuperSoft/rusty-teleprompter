"""Prepare free Android build tools in the parent workspace's .tools directory."""
from pathlib import Path
import os,shutil,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1]
TOOLS=ROOT.parent/'.tools'
DOWNLOADS=TOOLS/'rustyprompter'
DOWNLOADS.mkdir(parents=True,exist_ok=True)
def download(url,name):
    path=DOWNLOADS/name
    if not path.exists():subprocess.run(['curl.exe','-fLsS','--retry','2','--max-time','300',url,'-o',str(path)],check=True)
    return path
if not (TOOLS/'jdk/bin/java.exe').exists():
    with zipfile.ZipFile(download('https://aka.ms/download-jdk/microsoft-jdk-17.0.20.1-windows-x64.zip','jdk17.zip'))as z:
        base=z.namelist()[0].split('/')[0];z.extractall(DOWNLOADS/'jdk-extract')
    shutil.move(str(DOWNLOADS/'jdk-extract'/base),TOOLS/'jdk')
if not (TOOLS/'gradle-8.11.1/bin/gradle.bat').exists():
    with zipfile.ZipFile(download('https://services.gradle.org/distributions/gradle-8.11.1-bin.zip','gradle.zip'))as z:z.extractall(TOOLS)
sdk=TOOLS/'android-sdk'
if not (sdk/'cmdline-tools/latest/bin/sdkmanager.bat').exists():
    with zipfile.ZipFile(download('https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip','android-cli.zip'))as z:z.extractall(DOWNLOADS/'sdk-extract')
    (sdk/'cmdline-tools').mkdir(parents=True,exist_ok=True)
    shutil.move(str(DOWNLOADS/'sdk-extract/cmdline-tools'),sdk/'cmdline-tools/latest')
env=os.environ.copy();env['JAVA_HOME']=str(TOOLS/'jdk');env['PATH']=str(TOOLS/'jdk/bin')+os.pathsep+env.get('PATH','')
manager=sdk/'cmdline-tools/latest/bin/sdkmanager.bat'
subprocess.run([str(manager),'--sdk_root='+str(sdk),'--licenses'],input='y\n'*30,text=True,env=env,check=True)
subprocess.run([str(manager),'--sdk_root='+str(sdk),'platforms;android-35','build-tools;35.0.0','platform-tools'],env=env,check=True)
print('Android toolchain ready.')
