// Ordinary browser input and actual clipboard effects over the real GPU page.
// Only read-only ABI/GPU-completion observation is captured; no app state/content is injected.
// bun coax/web/tests/connect_pylon_browser.ts ORIGIN [OUT_PREFIX]
import {existsSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {chrome,gpuFlags} from '../chrome'
const base=process.argv[2]?.replace(/\/$/,''),prefix=process.argv[3]??'/tmp/connect-pylon'
if(!base?.startsWith('http'))throw Error('usage: connect_pylon_browser.ts ORIGIN [OUT_PREFIX]')
const source=readFileSync(resolve(import.meta.dir,'../../../docs/connect-a-pylon.md'),'utf8')
const command='curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh && bash /tmp/join-pylon.sh'
const profile=mkdtempSync(join(tmpdir(),'coax-guide-'))
const proc=Bun.spawn([chrome(),'--headless=new','--mute-audio',`--user-data-dir=${profile}`,'--no-first-run','--hide-scrollbars',...gpuFlags(),'--window-size=1440,900','--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'})
let ws:WebSocket|undefined;const evidence:any={base,at:new Date().toISOString(),scenarios:[]}
try{
 const portFile=join(profile,'DevToolsActivePort');let port='';for(let i=0;i<200;i++){if(existsSync(portFile)){const candidate=readFileSync(portFile,'utf8').split('\n')[0];if(/^[0-9]+$/.test(candidate)){port=candidate;break}}await Bun.sleep(50)}if(!port)throw Error('Chrome debugger port did not become ready')
 const targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()
 ws=new WebSocket(targets.find((t:any)=>t.type==='page').webSocketDebuggerUrl);await new Promise<void>((yes,no)=>{ws!.addEventListener('open',()=>yes(),{once:true});ws!.addEventListener('error',()=>no(Error('debugger connection')),{once:true})})
 let serial=0,recording:any;const pending=new Map<number,{yes:(v:any)=>void,no:(e:any)=>void,timer:ReturnType<typeof setTimeout>}>()
 ws.addEventListener('message',event=>{const m=JSON.parse(String(event.data)),p=pending.get(m.id);if(p){pending.delete(m.id);clearTimeout(p.timer);if(m.error)p.no(Error(JSON.stringify(m.error)));else p.yes(m.result)}if(recording&&m.method==='Runtime.exceptionThrown')recording.errors.push(m.params.exceptionDetails.exception?.description??m.params.exceptionDetails.text)})
 const send=(method:string,params:any={})=>new Promise<any>((yes,no)=>{const id=++serial,timer=setTimeout(()=>{pending.delete(id);no(Error('debugger timed out: '+method))},20000);pending.set(id,{yes,no,timer});ws!.send(JSON.stringify({id,method,params}))})
 async function evaluate(expression:string){const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description??r.exceptionDetails.text);return r.result?.value}
 async function until(predicate:()=>Promise<boolean>,why:string){const end=Date.now()+30000;while(Date.now()<end){if(recording.errors.length)throw Error(recording.errors.join('\n'));if(await predicate())return;await Bun.sleep(100)}throw Error('timed out: '+why)}
 const observe=()=>evaluate('globalThis.guideObservation?.()')
 await send('Runtime.enable');await send('Page.enable');await send('Emulation.setFocusEmulationEnabled',{enabled:true});await send('Browser.grantPermissions',{origin:new URL(base).origin,permissions:['clipboardReadWrite','clipboardSanitizedWrite']})
 await send('Page.addScriptToEvaluateOnNewDocument',{source:`const submit=GPUQueue.prototype.submit;GPUQueue.prototype.submit=function(commands){const r=submit.call(this,commands);this.onSubmittedWorkDone().then(()=>globalThis.guidePresented=true);return r;};const original=WebAssembly.instantiate;WebAssembly.instantiate=async function(module,imports){const r=await original.call(WebAssembly,module,imports),x=(r.instance||r).exports;if(x.coaxial_guide_control){const memory=imports.env.memory,state=Math.ceil(Math.max(Number(x.__heap_base.value),memory.buffer.byteLength)/65536)*65536;globalThis.guideObservation=()=>{if(globalThis.coaxial?.ready!==true)return {ready:false};return ({ready:true,scroll:x.coaxial_guide_scroll(state),limit:x.coaxial_guide_limit(state),action:x.coaxial_action(state),controls:Array.from({length:4},(_,c)=>Array.from({length:4},(_,a)=>x.coaxial_guide_control(state,c,a)))});};}return r;};`})
 for(const v of [{name:'desktop',w:1440,h:900,d:1,touch:false},{name:'phone',w:390,h:844,d:2,touch:true}]){
  recording={viewport:v,errors:[]};evidence.scenarios.push(recording)
  await send('Emulation.setDeviceMetricsOverride',{width:v.w,height:v.h,deviceScaleFactor:v.d,mobile:v.touch})
  await send('Emulation.setTouchEmulationEnabled',{enabled:v.touch,maxTouchPoints:2})
  await send('Page.navigate',{url:base+'/docs/connect-a-pylon'})
  // Module readiness precedes first layout/present. The host sceneReady flag
  // describes 3D draws, so observe real queue completion for this 2D GPU page.
  await until(async()=>{const o=await observe();return o?.ready===true&&o.limit>0&&o.controls[0][2]>o.controls[0][0]&&await evaluate('globalThis.guidePresented===true')},v.name+' GPU scene readiness')
  recording.initial=await observe();recording.screenshot=`${prefix}-${v.name}.png`
  await Bun.sleep(250);writeFileSync(recording.screenshot,Buffer.from((await send('Page.captureScreenshot',{format:'png'})).data,'base64'))
  const fallback=await evaluate('({width:document.querySelector("main").getBoundingClientRect().width,alternate:document.querySelector("link[type=\\"text/markdown\\"]")?.getAttribute("href")})')
  if(fallback.width!==1||fallback.alternate!=='/docs/connect-a-pylon/index.md')throw Error('visible DOM or missing agent discovery')
  async function click(id:number){const b=(await observe()).controls[id],x=(b[0]+b[2])/2,y=(b[1]+b[3])/2;if(v.touch){await send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y,id:1,radiusX:1,radiusY:1,force:1}]});await send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]})}else{await send('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',buttons:1,clickCount:1});await send('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',buttons:0,clickCount:1})}}
  await click(0);await until(async()=>await evaluate('navigator.clipboard.readText()')===source,v.name+' complete Markdown clipboard');recording.markdown_bytes=Buffer.byteLength(source)
  await click(1);await until(async()=>await evaluate('navigator.clipboard.readText()')===command,v.name+' exact join clipboard');recording.command_bytes=command.length
  const body=(await observe()).controls[3],cx=(body[0]+body[2])/2,cy=(body[1]+body[3])/2
  if(v.touch){await send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:cx,y:cy+80,id:1,radiusX:1,radiusY:1,force:1}]});await send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:cx,y:cy-80,id:1,radiusX:1,radiusY:1,force:1}]});await send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]})}
  else await send('Input.dispatchMouseEvent',{type:'mouseWheel',x:cx,y:cy,deltaY:160,deltaX:0})
  await until(async()=>(await observe()).scroll>100,v.name+' document scroll');recording.scrolled=await observe()
  if(await evaluate('navigator.clipboard.readText()')!==command)throw Error('scroll unexpectedly copied or navigated')
  const response=await fetch(base+'/docs/connect-a-pylon/index.md');if(!response.ok||await response.text()!==source||!response.headers.get('content-type')?.includes('text/markdown'))throw Error('agent Markdown HTTP parity/MIME')
  console.log(JSON.stringify({viewport:v.name,ready:true,markdown_bytes:recording.markdown_bytes,command_bytes:recording.command_bytes,scroll:recording.scrolled.scroll,screenshot:recording.screenshot}))
 }
 writeFileSync(prefix+'.json',JSON.stringify(evidence,null,2)+'\n')
}finally{ws?.close();proc.kill();rmSync(profile,{recursive:true,force:true})}
