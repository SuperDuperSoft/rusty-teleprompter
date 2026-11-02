from pathlib import Path
import json,shutil,tomllib
from task import ROOT,TOOLS
licenses=ROOT/'licenses';licenses.mkdir(exist_ok=True)
for name in ['Apache-2.0.txt','Java-WebSocket-MIT.txt','SLF4J-MIT.txt']:
    old=ROOT.parent/'licenses'/name
    if old.exists():shutil.copy2(old,licenses/name)
registry=TOOLS/'cargo/registry/src'
lock=tomllib.loads((ROOT/'Cargo.lock').read_text())
index=[]
for package in lock['package']:
    if not package.get('source','').startswith('registry+'):continue
    sources=list(registry.glob('*/'+package['name']+'-'+package['version']))
    if not sources:continue
    source=sources[0];manifest=tomllib.loads((source/'Cargo.toml').read_text(encoding='utf-8'))
    copied=[]
    for path in source.iterdir():
        if path.is_file()and path.name.lower().startswith(('license','licence','copying','copyright','notice')):
            target=licenses/'rust'/source.name/path.name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target);copied.append(path.name)
    custom=manifest.get('package',{}).get('license-file')
    if custom and (source/custom).is_file() and custom not in copied:
        target=licenses/'rust'/source.name/Path(custom).name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source/custom,target);copied.append(target.name)
    index.append({'crate':package['name'],'version':package['version'],'license':manifest.get('package',{}).get('license','upstream'),'files':copied,'repository':manifest.get('package',{}).get('repository','')})
(licenses/'rust').mkdir(exist_ok=True)
(licenses/'rust/INDEX.json').write_text(json.dumps(index,indent=2),encoding='utf-8')
print('Collected notices for '+str(len(index))+' Rust packages.')
