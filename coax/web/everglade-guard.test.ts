// Actual Everglade Wasm + shipped host, with a local synthetic SystemOne edge.
// Build just this realm first; no real account, Pylon, keys or model required.
// EVERGLADE_GUARD_WASM=/tmp/everglade.wasm bun test coax/web/everglade-guard.test.ts
import {afterAll, beforeAll, describe, expect, test} from 'bun:test'
import {existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs'
import {join} from 'node:path'
import {tmpdir} from 'node:os'
import {chrome as chromePath, gpuFlags} from './chrome'

const root = join(import.meta.dir, '../..'), wasm = process.env.EVERGLADE_GUARD_WASM
if (wasm && !existsSync(wasm)) throw Error('EVERGLADE_GUARD_WASM does not exist')
const profile = wasm ? mkdtempSync(join(tmpdir(), 'everglade-guard-')) : ''
const identity = 'elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba'
const response = {model:'elephant-1', service:{version:'psionic-pylon-v1',identity, provider:'a'.repeat(64), door:'pylon:'+'b'.repeat(64)},
  answers:{ambient:{type:'choice', choice:'greeting', confidence:0.8, probabilities:{quiet:0.1,greeting:0.8,weather:0.1}}}}
let server: ReturnType<typeof Bun.serve>, chrome: ReturnType<typeof Bun.spawn>, ws: WebSocket
let serial=0, mode='good', status=200, calls:any[]=[]
let held: ((r:Response)=>void) | undefined
const replies=new Map<number,(r:any)=>void>(), errors:string[]=[]
function cdp(method:string,params:any={}) {
  return new Promise<any>((resolve,reject)=>{
    const id=++serial, timer=setTimeout(()=>{replies.delete(id);reject(Error('CDP timeout '+method))},30000)
    replies.set(id,r=>{clearTimeout(timer);r.error?reject(Error(JSON.stringify(r.error))):resolve(r.result)})
    ws.send(JSON.stringify({id,method,params}))
  })
}
async function evaluate(expression:string) {
  const r=await cdp('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true})
  if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails))
  return r.result.value
}
async function until(predicate:()=>Promise<boolean>,why:string) {
  const end=Date.now()+45000
  while(Date.now()<end){if(await predicate())return;await Bun.sleep(60)}
  throw Error(why+' '+JSON.stringify(await evaluate('({ready:coaxial?.ready,scene:coaxial?.sceneReady,p:Array.from({length:7},(_,i)=>coaxial?.guardProbe(i))})'))+' '+errors.slice(-3).join('\n'))
}
const probe=()=>evaluate('Array.from({length:7},(_,i)=>coaxial.guardProbe(i))')
async function visit(query:string,next='good',code=200) {
  mode=next;status=code;calls=[];errors.length=0
  await cdp('Page.navigate',{url:`http://127.0.0.1:${server.port}/realms/everglade${query}`})
  await until(async()=>{if(query.includes('clip=1'))await evaluate('globalThis.coaxialClipStep?.(2)');return evaluate('!!globalThis.coaxial?.sceneReady')},'Everglade scene did not become ready')
}
describe.skipIf(!wasm)('actual Everglade guard consumer (set EVERGLADE_GUARD_WASM)',()=>{
beforeAll(async()=>{
  server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(req){
    const url=new URL(req.url),path=url.pathname
    if(path==='/api/v1/systemone'){
      calls.push({body:await req.json(),player:req.headers.get('X-Verse-Player'),authorization:req.headers.get('Authorization')})
      if(mode==='hold')return await new Promise<Response>(r=>{held=r})
      return new Response(mode==='bad'?'{}':mode==='large'?'x'.repeat(20000):JSON.stringify(response),{status,headers:{'content-type':'application/json'}})
    }
    if(path.startsWith('/api/'))return new Response('',{status:404})
    if(path==='/everglade.wasm')return new Response(Bun.file(wasm!),{headers:{'content-type':'application/wasm'}})
    if(path==='/realms/everglade')return new Response(`<!doctype html><html><style>html,body{margin:0;background:black}canvas{width:100vw;height:100vh}</style><canvas id="coaxial"></canvas><script>
      Date.now=()=>1793361600000;
      if(location.search.includes('clip=1'))globalThis.coaxialClip=true;
      localStorage.clear();
      localStorage.setItem('coaxial.everglade.place','verse-place 1\\nrealm=everglade\\nat=1000\\nx=0\\ny=0\\nz=-67000\\nface=0\\nyaw=0\\npitch=300\\nzoom=6000\\nview=0\\nroom=-1\\n');
      </script><script type="module" src="/coaxial/host.js" data-app="everglade" data-slot="everglade" data-ruleset="srd5" data-wasm="/everglade.wasm" data-assets="pack:/realms/everglade/assets/everglade.pack particles:/everglade-fx.wgsl"></script></html>`,{headers:{'content-type':'text/html'}})
    const file=join(root,path.startsWith('/coaxial/')?'coax/web/'+path.slice(9):'apps/openagents-web/'+path.slice(1))
    return existsSync(file)?new Response(Bun.file(file)):new Response('',{status:404})
  }})
  chrome=Bun.spawn([chromePath(),'--headless=new','--mute-audio',`--user-data-dir=${profile}`,'--no-first-run',
    '--hide-scrollbars',...gpuFlags(),'--window-size=640,480','--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'})
  const portFile=join(profile,'DevToolsActivePort')
  for(let i=0;i<100&&!existsSync(portFile);i++)await Bun.sleep(50)
  const port=readFileSync(portFile,'utf8').split('\n')[0]
  const target=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t:any)=>t.type==='page')
  ws=new WebSocket(target.webSocketDebuggerUrl)
  await new Promise(r=>ws.addEventListener('open',r,{once:true}))
  ws.addEventListener('message',e=>{const r=JSON.parse(String(e.data));if(r.method==='Runtime.exceptionThrown')errors.push(JSON.stringify(r.params));if(r.method==='Runtime.consoleAPICalled'&&r.params.type==='error')errors.push(r.params.args.map((a:any)=>a.value??a.description).join(' '));replies.get(r.id)?.(r);replies.delete(r.id)})
  await cdp('Page.enable');await cdp('Runtime.enable');await cdp('Emulation.setFocusEmulationEnabled',{enabled:true})
},60000)
afterAll(()=>{held?.(new Response('{}'));ws?.close();chrome?.kill();server?.stop(true);rmSync(profile,{recursive:true,force:true})})

test('ordinary and unqualified ambient visits make no guard request',async()=>{
  for(const query of ['','?guard-ai=ambient']){
    await visit(query);await Bun.sleep(700)
    expect((await probe()).slice(0,3)).toEqual([0,0,0]);expect(calls.length).toBe(0)
  }
},120000)
test('shadow uses exact bounded public request and admits only at a later game boundary',async()=>{
  await visit('?guard-ai=shadow');await until(async()=> (await probe())[3]===1,'shadow reply not admitted')
  const p=await probe();expect(p[0]).toBe(1);expect(p[1]).toBe(1);expect(p[2]).toBe(0);expect(p[5]).toBe(1)
  expect(calls.length).toBe(1);expect(calls[0].body.model).toBe('elephant-1')
  expect(Object.keys(calls[0].body.questions.ambient.criteria)).toEqual(['quiet','greeting','weather'])
  expect(calls[0].player).toBeNull();expect(calls[0].authorization).toBeNull()
  expect(JSON.stringify(calls[0].body)).not.toContain('verse.player')
  if(process.env.EVERGLADE_GUARD_SCREENSHOT){const shot=await cdp('Page.captureScreenshot',{format:'png'});writeFileSync(process.env.EVERGLADE_GUARD_SCREENSHOT,Buffer.from(shot.data,'base64'))}
},60000)
test('malformed and oversized successful bodies preserve fallback',async()=>{
  for(const next of ['bad','large']){
    await visit('?guard-ai=shadow',next);await until(async()=> (await probe())[4]>0,'invalid body not refused')
    const p=await probe();expect(p[3]).toBe(0);expect(p[2]).toBe(0);expect(p[1]).toBe(1)
  }
},120000)
test('guest and quota refusal disable further attempts for this visit',async()=>{
  for(const code of [401,429]){
    await visit('?guard-ai=shadow','good',code);await until(async()=> (await probe())[6]===1,'session refusal did not disable guard requests')
    expect((await probe())[3]).toBe(0);expect(calls.length).toBe(1)
  }
},120000)
test('blur revokes a pending request before its delayed reply can apply',async()=>{
  await visit('?guard-ai=shadow','hold');await until(async()=>calls.length===1&&!!held,'request not held')
  await cdp('Emulation.setFocusEmulationEnabled',{enabled:false});await evaluate("dispatchEvent(new Event('blur'))")
  await until(async()=> (await probe())[2]===0,'blur did not revoke pending owner')
  const reply=held!;held=undefined;reply(new Response(JSON.stringify(response),{headers:{'content-type':'application/json'}}))
  await Bun.sleep(300);expect((await probe())[3]).toBe(0)
  await cdp('Emulation.setFocusEmulationEnabled',{enabled:true})
},60000)
test('golden and clip views make no guard request',async()=>{
  for(const query of ['?guard-ai=shadow&golden=1','?guard-ai=shadow&clip=1']){
    await visit(query);await Bun.sleep(500);expect((await probe())[1]).toBe(0);expect(calls.length).toBe(0)
  }
},120000)

})
