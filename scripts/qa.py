"""Isolated UI checks using the real Rust server and Dioxus/WASM application.

The tablet test page fakes only Android transport/audio input, exposing explicit
buttons for speech-result/lifecycle fixtures. It is not shipped with the apps.
"""
from pathlib import Path
import json,shutil,subprocess,time
from task import ROOT,TOOLS

folder=TOOLS/'qa';folder.mkdir(exist_ok=True)
web=folder/'web';shutil.copytree(ROOT/'web-dist',web,dirs_exist_ok=True)
sample={'id':'qa-tablet','title':'Meet RustyPrompter','revision':1,'slides':[
 {'id':'qa-one','title':'Your voice. Your pace.','text':'Welcome to RustyPrompter. I can take my time and read at my own pace. When I finish, we move to the next slide.'},
 {'id':'qa-two','title':'One more thing','text':'This is slide two. I can pause, skip a few words, and keep reading. That is the end of this take.'}
]}
script='''<script>
window.__rustyEvents=[];
window.qaLibrary=LIBRARY;
window.RustyNative={initialState:()=>JSON.stringify({native:true,library:window.qaLibrary,voiceReady:true,voiceStatus:'Offline voice model ready (fixture)',connection:'Connected to PC (test adapter)',settings:{}}),command:(name,payload)=>{if(name==='start'||name==='stop'||name==='reset'){const output=document.getElementById('qa-audio');if(output)output.textContent=name==='start'?'Microphone: listening (take '+payload+')':name==='reset'?'Microphone: reset (take '+payload+')':'Microphone: stopped';}if(name==='pair')window.__rustyEvents.push({type:'paired'});}};
window.qaSpeak=(i)=>window.__rustyEvents.push({type:'speech',text:window.qaLibrary.scripts[0].slides[i].text,final:true});
window.qaSpeakStale=()=>window.__rustyEvents.push({type:'speech',text:window.qaLibrary.scripts[0].slides[0].text,final:true,take:999999});
window.qaUpdate=()=>{window.qaLibrary=JSON.parse(JSON.stringify(window.qaLibrary));window.qaLibrary.generation++;window.qaLibrary.scripts[0].revision++;window.qaLibrary.scripts[0].slides[1].text='Updated while you read. '+window.qaLibrary.scripts[0].slides[1].text;window.__rustyEvents.push({type:'library',...window.qaLibrary});};
</script>'''.replace('LIBRARY',json.dumps({'generation':1,'scripts':[sample]}))
controls='''<div style="position:fixed;bottom:4px;right:4px;z-index:999;background:#fff4d9;color:#342b1d;padding:5px;border:1px solid #ae8d50;font:10px Arial;display:flex;gap:3px" id="qa-controls"><button onclick="qaSpeak(0)">Speak slide one</button><button onclick="qaSpeak(1)">Speak slide two</button><button onclick="qaSpeakStale()">Speak stale take</button><button onclick="qaUpdate()">Queue PC update</button><button onclick="window.__rustyEvents.push({type:'pause'})">Background app</button><span id="qa-audio">Microphone: stopped</span><button onclick="document.getElementById('qa-controls').hidden=true">Hide test controls</button></div>'''
index=(web/'index.html').read_text(encoding='utf-8').replace('<head>','<head>'+script).replace('</body>',controls+'</body>')
(web/'assets/qa-tablet.html').write_text(index,encoding='utf-8')
exe=folder/'RustyPrompter-QA.exe';shutil.copy2(ROOT/'target/release/rusty-companion.exe',exe)
ready=folder/'ready.json'
if ready.exists():ready.unlink()
process=subprocess.Popen([str(exe),'--headless','--port','8877','--data',str(folder/'data'),'--web',str(web),'--ready-file',str(ready)],creationflags=subprocess.CREATE_NO_WINDOW)
(folder/'process.json').write_text(json.dumps({'pid':process.pid,'exe':str(exe)}))
for _ in range(100):
 if ready.exists():break
 if process.poll()is not None:raise RuntimeError('Rust QA server stopped before becoming ready.')
 time.sleep(.1)
print(ready.read_text())
