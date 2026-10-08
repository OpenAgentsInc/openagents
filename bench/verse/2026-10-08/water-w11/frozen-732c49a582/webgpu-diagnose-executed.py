"""Finite observation of the frozen renderer; no source or GPU writes changed."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import base64, hashlib, json, os, subprocess, sys, threading, time
BASE=Path(__file__).resolve().parent
ROOT=BASE/'water-w11'
SITE=BASE/'w11-wasm-732c49a582474fa6e669716637d1e0b194d5e8ba/site'
OUT=BASE/'w11-webgpu-diagnosis-732c49a582'
OUT.mkdir(exist_ok=True)
PROBE=r'''(() => {
 const d=window.__waterDiagnosis={errors:[],lost:[],scopes:[],shaders:[],configured:[],presented:[],submitted:0,maps:{started:0,done:0,failed:0},readbacks:[],passes:[]};
 const ids=new WeakMap();let next=0;const id=x=>{if(!ids.has(x))ids.set(x,++next);return ids.get(x)};
 const meta=new WeakMap();const writes=new WeakMap();
 const request=GPUAdapter.prototype.requestDevice;
 GPUAdapter.prototype.requestDevice=function(...args){return request.apply(this,args).then(device=>{
  d.adapter={features:[...this.features],info:this.info};d.deviceFeatures=[...device.features];
  device.addEventListener('uncapturederror',e=>d.errors.push({type:e.error.constructor.name,message:e.error.message}));
  device.lost.then(v=>d.lost.push({reason:v.reason,message:v.message}));return device;
 })};
 const pop=GPUDevice.prototype.popErrorScope;
 GPUDevice.prototype.popErrorScope=function(...args){const promise=pop.apply(this,args);promise.then(e=>{if(e)d.scopes.push({type:e.constructor.name,message:e.message})});return promise;};
 const shader=GPUDevice.prototype.createShaderModule;
 GPUDevice.prototype.createShaderModule=function(...args){const m=shader.apply(this,args);m.getCompilationInfo().then(info=>{if(info.messages.length)d.shaders.push({label:m.label,messages:info.messages.map(v=>({type:v.type,message:v.message,line:v.lineNum,column:v.linePos}))})});return m;};
 const configure=GPUCanvasContext.prototype.configure;
 GPUCanvasContext.prototype.configure=function(...args){let v=args[0];d.configured.push({format:v.format,usage:v.usage,alphaMode:v.alphaMode,viewFormats:v.viewFormats});return configure.apply(this,args)};
 const texture=GPUCanvasContext.prototype.getCurrentTexture;
 GPUCanvasContext.prototype.getCurrentTexture=function(...args){let t=texture.apply(this,args);if(d.presented.length<8)d.presented.push({width:t.width,height:t.height,format:t.format,usage:t.usage});return t;};
 const submit=GPUQueue.prototype.submit;
 GPUQueue.prototype.submit=function(...args){d.submitted++;return submit.apply(this,args)};
 const pass=GPUCommandEncoder.prototype.beginRenderPass;
 GPUCommandEncoder.prototype.beginRenderPass=function(...args){let v=args[0],w=v.timestampWrites;if(w){let rows=writes.get(this)||[];rows.push({label:v.label,query:id(w.querySet),begin:w.beginningOfPassWriteIndex,end:w.endOfPassWriteIndex});writes.set(this,rows);if(d.passes.length<32)d.passes.push(rows[rows.length-1]);}return pass.apply(this,args)};
 const resolve=GPUCommandEncoder.prototype.resolveQuerySet;
 GPUCommandEncoder.prototype.resolveQuerySet=function(q,first,count,dest,offset){meta.set(dest,{query:id(q),first,count,offset,passes:(writes.get(this)||[]).filter(v=>v.query===id(q))});return resolve.apply(this,arguments)};
 const copy=GPUCommandEncoder.prototype.copyBufferToBuffer;
 GPUCommandEncoder.prototype.copyBufferToBuffer=function(src,srcOffset,dest,destOffset,size){if(meta.has(src))meta.set(dest,meta.get(src));return copy.apply(this,arguments)};
 const map=GPUBuffer.prototype.mapAsync;
 GPUBuffer.prototype.mapAsync=function(...args){d.maps.started++;const p=map.apply(this,args);p.then(()=>d.maps.done++,e=>{d.maps.failed++;d.errors.push({type:'mapAsync',message:String(e)})});return p;};
 const range=GPUBuffer.prototype.getMappedRange;
 GPUBuffer.prototype.getMappedRange=function(...args){const b=range.apply(this,args);if(b.byteLength===64 && this.label.includes('water')){let r={label:this.label,meta:meta.get(this),ticks:[...new BigUint64Array(b)].map(x=>x.toString())};if(d.readbacks.length<64)d.readbacks.push(r);else d.readbacks[d.readbacks.length-1]=r;}return b;};
})();'''
if len(sys.argv)>1 and sys.argv[1]=='child':
 sys.path.insert(0,str(ROOT/'bench/verse/2026-10-08/water-w11'))
 from cdp import Cdp
 import urllib.request
 port=os.environ['OPENAGENTS_CHROME_PORT'];origin=sys.argv[2]
 results=[]
 for dry in [True,False]:
  tag='dry' if dry else 'wet';page=None;row={'case':tag,'app_source':'732c49a582474fa6e669716637d1e0b194d5e8ba','fence_wrapper':False}
  try:
   target=json.load(urllib.request.urlopen(urllib.request.Request(f'http://127.0.0.1:{port}/json/new?about:blank',method='PUT')));page=Cdp(target['webSocketDebuggerUrl'])
   for domain in ['Page','Runtime','Log','Network']:page.call(domain+'.enable')
   page.call('Emulation.setDeviceMetricsOverride',{'width':1920,'height':1080,'deviceScaleFactor':1,'mobile':False})
   page.call('Page.addScriptToEvaluateOnNewDocument',{'source':PROBE})
   page.call('Page.navigate',{'url':origin+'/index.html?frames&zone=water&town-clock=off'+('&water-dry' if dry else '')})
   t=time.monotonic();ready=False
   while time.monotonic()-t<120:
    time.sleep(1);probe=page.eval('({status:document.querySelector("#everglade-status")?.textContent,ready:document.querySelector("#everglade-status")&&getComputedStyle(document.querySelector("#everglade-status")).display==="none"})')['result'].get('value',{})
    if probe.get('ready'):ready=True;break
    if 'could not start' in probe.get('status','').lower():break
   row['startup']=probe;row['ready']=ready
   time.sleep(3)
   for capture in ['original','front','view']:
    if capture=='front':page.call('Page.bringToFront');time.sleep(1)
    image=OUT/(tag+'-'+capture+'.png');image.write_bytes(base64.b64decode(page.call('Page.captureScreenshot',{'format':'png','fromSurface':capture!='view'})['data']))
    row[capture+'_capture']={'file':image.name,'sha256':hashlib.sha256(image.read_bytes()).hexdigest()}
   row['diagnosis']=page.eval('({d:window.__waterDiagnosis,hidden:document.hidden,visibility:document.visibilityState,canvas:[document.querySelector("canvas").width,document.querySelector("canvas").height]})')['result'].get('value')
  except Exception as error:row['failure']=repr(error)
  finally:
   if page:
    row['events']=page.events
    try:page.call('Page.close')
    except Exception:pass
    page.s.close()
   results.append(row);(OUT/'diagnosis.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({'case':tag,'ready':row.get('ready'),'failure':row.get('failure'),'device_errors':row.get('diagnosis',{}).get('d',{}).get('errors'),'scope_errors':row.get('diagnosis',{}).get('d',{}).get('scopes')}),flush=True)
 sys.exit(any('failure' in r for r in results))
else:
 assert all(v in os.environ.get('OPENAGENTS_LEASES','') for v in ['quiet','gpu','browser'])
 class Handler(SimpleHTTPRequestHandler):
  def log_message(self,*_):pass
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Handler,directory=str(SITE)));server.daemon_threads=True
 thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
 command=['openagents','browser','run','--json','--','python3',str(Path(__file__).resolve()),'child','http://127.0.0.1:'+str(server.server_port)]
 started=time.time()
 try:result=subprocess.run(command,timeout=330)
 finally:server.shutdown();server.server_close();thread.join()
 (OUT/'run.json').write_text(json.dumps({'command':command,'exit':result.returncode,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'started_at_unix_ms':round(started*1000),'ended_at_unix_ms':round(time.time()*1000),'render_source':'732c49a582474fa6e669716637d1e0b194d5e8ba'},indent=2)+'\n')
 sys.exit(result.returncode)
