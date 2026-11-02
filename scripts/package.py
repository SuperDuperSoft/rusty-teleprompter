"""Archive only this new app's source and verified delivery artifacts."""
from pathlib import Path
import hashlib,json,zipfile
from task import ROOT

dist=ROOT/'dist';dist.mkdir(exist_ok=True)
excluded={'target','dist','.signing','.gradle','build','web-dist','__pycache__','model-en-us','web','notices'}
with zipfile.ZipFile(dist/'RustyPrompter-Source.zip','w',zipfile.ZIP_DEFLATED,strict_timestamps=False)as archive:
    for path in ROOT.rglob('*'):
        rel=path.relative_to(ROOT)
        if path.is_file() and not(set(rel.parts)&excluded)and path.name!='local.properties':
            archive.write(path,'RustyPrompter-Source/'+rel.as_posix())
names=['RustyPrompter-Android.apk','RustyPrompter-Windows.zip','RustyPrompter-Source.zip','RustyPrompter-Signing-Backup.zip']
(dist/'SHA256SUMS.txt').write_text('\n'.join(hashlib.sha256((dist/name).read_bytes()).hexdigest()+'  '+name for name in names)+'\n')
print('Source archive and delivery checksums ready.')
