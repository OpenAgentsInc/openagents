// Production-path readiness: muted headless Chrome, real sockets and the
// Coax module's read-only probe. No golden fixture, publishing or keys.
import {mkdtempSync,readFileSync,existsSync,rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {startChrome} from '../chrome'
const base=process.argv[2]?.replace(/\/$/,'');if(!base)throw Error('usage: home_live_browser.ts ORIGIN')
const {proc,profile,port}=await startChrome('coax-home-live-',['--window-size=1280,800'])
let ws:WebSocket|undefined
try{
 const targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json(),target=targets.find((t:any)=>t.type==='page')
 ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws!.addEventListener('open',r,{once:true}))
 let serial=0,connections=0,received=0,sent=0;const pending=new Map<number,(v:any)=>void>(),errors:string[]=[]
 ws.addEventListener('message',e=>{const m=JSON.parse(String(e.data));pending.get(m.id)?.(m);pending.delete(m.id)
  if(m.method==='Network.webSocketCreated')connections++
  if(m.method==='Network.webSocketFrameReceived')received++
  if(m.method==='Network.webSocketFrameSent')sent++
  if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails.exception?.description??m.params.exceptionDetails.text)
 })
 const send=(method:string,params:any={})=>new Promise<any>(r=>{pending.set(++serial,r);ws!.send(JSON.stringify({id:serial,method,params}))})
 await send('Network.enable');await send('Runtime.enable');await send('Page.navigate',{url:base+'/'})
 const start=Date.now();let value=0,ready=false
 while(Date.now()-start<90000){
  const result=await send('Runtime.evaluate',{expression:'({ready:globalThis.coaxial?.ready===true,probe:globalThis.coaxial?.probe?.()??0})',returnByValue:true})
  const state=result.result?.result?.value;value=Number(state?.probe??0);ready=state?.ready===true
  if(errors.length)throw Error(errors.join('\n'))
  if(ready&&(value&255)===255)break
  await Bun.sleep(250)
 }
 if(!ready||(value&255)!==255)throw Error('homepage did not complete real catch-up: '+JSON.stringify({ready,value,connections,received,sent}))
 console.log(JSON.stringify({base,ready_projects:value&63,watch_connected:!!(value&64),beacon_history_ended:!!(value&128),beacon_observations:(value>>>8)&255,project_refreshes:(value>>>16)&255,connections,received_frames:received,sent_frames:sent,errors,elapsed_ms:Date.now()-start}))
}finally{ws?.close();proc.kill();rmSync(profile,{recursive:true,force:true})}
