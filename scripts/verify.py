from pathlib import Path
import json,subprocess,zipfile
from task import ROOT

dist=ROOT/'dist';shared=ROOT.parent/'.tools'
apk=dist/'RustyPrompter-Android.apk'
result=subprocess.run([str(shared/'jdk/bin/java.exe'),'-jar',str(shared/'android-sdk/build-tools/35.0.0/lib/apksigner.jar'),'verify','--verbose','--print-certs',str(apk)],check=True,capture_output=True,text=True)
(dist/'APK-SIGNATURE.txt').write_text(result.stdout)
with zipfile.ZipFile(apk)as archive:
    assert archive.testzip()is None
    names=set(archive.namelist())
    for name in ['assets/web/index.html','assets/web/assets/rusty-ui.js','assets/web/assets/rusty-ui_bg.wasm','assets/web/assets/main.css','assets/model-en-us/am/final.mdl']:
        assert name in names,name
    for abi in ['armeabi-v7a','arm64-v8a','x86_64']:
        for library in ['libvosk.so','libjnidispatch.so']:assert f'lib/{abi}/{library}'in names
    assert not any('qa-tablet' in name for name in names)
with zipfile.ZipFile(dist/'RustyPrompter-Windows.zip')as archive:
    assert archive.testzip()is None
    assert 'RustyPrompter/RustyPrompter.exe'in archive.namelist()
    assert 'RustyPrompter/web/assets/rusty-ui_bg.wasm'in archive.namelist()
    assert not any('qa-tablet'in name for name in archive.namelist())
with zipfile.ZipFile(dist/'RustyPrompter-Source.zip')as archive:
    assert archive.testzip()is None
    assert not any('/.signing/'in name or '/target/'in name or name.endswith('local.properties')for name in archive.namelist())
smoke=json.loads((shared/'rustyprompter/packaged-smoke.json').read_text());assert smoke['ok']
report={'apkSignature':'verified','offlineDioxusBundle':'verified','bundledModelAndNativeLibraries':'verified','nativeWindowsSmoke':'passed','privateSigningMaterialExcludedFromSource':True,'unshippedFixtureAdapterExcluded':True}
(dist/'VERIFICATION.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
