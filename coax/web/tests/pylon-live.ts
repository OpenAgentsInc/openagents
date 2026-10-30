// Open the real page before the operator sends ephemeral jobs, then observe
// its actual Coax state. Headless/muted, isolated profile, no focus or audio.
// bun coax/web/tests/pylon-live.ts URL --expect slug1,slug2 --answers 2 --out /tmp/pylon.png
// Prints READY before waiting for answers. Run the buyer in another terminal.
import {mkdtempSync,readFileSync,existsSync,writeFileSync,rmSync} from "node:fs"
import {tmpdir} from "node:os"
import {join} from "node:path"
import {chrome,gpuFlags} from "../chrome"
const args=process.argv.slice(2),url=args[0]
const value=(n:string,d:string)=>{const i=args.indexOf(`--${n}`);return i<0?d:args[i+1]}
if(!url?.startsWith("http"))throw Error("usage: pylon-live.ts URL --expect SLUG,SLUG --answers N --out FILE")
const expected=value("expect","").split(",").filter(Boolean),answers=Number(value("answers","1")),seconds=Number(value("timeout","120")),out=value("out","/tmp/pylon-live.png"),perProvider=Number(value("per-provider",answers>0?"1":"0")),identity=value("identity","")
if(!Number.isInteger(answers)||answers<0||!Number.isInteger(perProvider)||perProvider<0||!Number.isFinite(seconds)||seconds<1||seconds>600)throw Error("invalid bounded live proof options")
const profile=mkdtempSync(join(tmpdir(),"coax-pylon-live-"))
const proc=Bun.spawn([chrome(),"--headless=new","--mute-audio",`--user-data-dir=${profile}`,"--no-first-run","--hide-scrollbars",...gpuFlags(),"--window-size=1280,800","--remote-debugging-port=0","about:blank"],{stdout:"ignore",stderr:"ignore"})
let ws:WebSocket|undefined
try{
  const portFile=join(profile,"DevToolsActivePort")
  for(let i=0;i<400&&!existsSync(portFile);i++)await Bun.sleep(50)
  const port=readFileSync(portFile,"utf8").split("\n")[0]
  const target=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t:any)=>t.type==="page")
  ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws!.addEventListener("open",r,{once:true}))
  let id=0;const pending=new Map<number,(m:any)=>void>(),errors:any[]=[]
  ws.addEventListener("message",e=>{const m=JSON.parse(String(e.data));if(m.id){pending.get(m.id)?.(m);pending.delete(m.id)}else if(m.method==="Runtime.exceptionThrown")errors.push(m.params)})
  const send=(method:string,params:any={})=>new Promise<any>(r=>{pending.set(++id,r);ws!.send(JSON.stringify({id,method,params}))})
  const evaluate=async(expression:string)=>(await send("Runtime.evaluate",{expression,returnByValue:true,awaitPromise:true})).result?.result?.value
  await send("Runtime.enable");await send("Page.enable")
  await send("Emulation.setDeviceMetricsOverride",{width:1280,height:800,deviceScaleFactor:1,mobile:false})
  // Capture the original module's observation ABI, never alter its bytes,
  // event stream, clock, results, exports or rendering. The state arena is
  // the host's documented fresh-arena allocation above memory/__heap_base.
  await send("Page.addScriptToEvaluateOnNewDocument",{source:`
    const originalInstantiate=WebAssembly.instantiate;
    WebAssembly.instantiate=async function(bytes,imports){
      const result=await originalInstantiate.call(WebAssembly,bytes,imports);
      const x=(result.instance||result).exports;
      if(x.coaxial_pylon_count){
        const memory=imports.env.memory;
        const state=Math.ceil(Math.max(Number(x.__heap_base.value),memory.buffer.byteLength)/65536)*65536;
        globalThis.pylonObservation=()=>{
          const count=x.coaxial_pylon_count(state),decode=new TextDecoder();
          const slugs=Array.from({length:count},(_,i)=>decode.decode(new Uint8Array(memory.buffer,x.coaxial_pylon_slug(state,i),x.coaxial_pylon_slug_len(state,i))));
          const rows=slugs.map((slug,i)=>({slug,identity:x.coaxial_pylon_identity?decode.decode(new Uint8Array(memory.buffer,x.coaxial_pylon_identity(state,i),x.coaxial_pylon_identity_len(state,i))):"",answers:x.coaxial_pylon_row_answers(state,i),statuses:x.coaxial_pylon_row_statuses(state,i)}));
          return {ready:globalThis.coaxial?.ready===true,count,slugs,rows,answers:x.coaxial_pylon_answers(state),statuses:x.coaxial_pylon_statuses(state),refused:x.coaxial_pylon_refused(state)};
        };
      }
      return result;
    };`})
  await send("Page.navigate",{url})
  const until=Date.now()+seconds*1000;let ready=false,snapshot:any
  while(Date.now()<until){
    snapshot=await evaluate("globalThis.pylonObservation?.()")
    if(snapshot?.ready&&expected.every(s=>snapshot.slugs.includes(s))){
      if(!ready){ready=true;console.log("READY "+JSON.stringify(snapshot))}
      if(snapshot.answers>=answers&&expected.every(s=>snapshot.rows.some((r:any)=>r.slug===s&&r.answers>=perProvider&&(!identity||r.identity===identity)))){
        if(errors.length)throw Error("page exceptions: "+JSON.stringify(errors))
        await Bun.sleep(500)
        const shot=await send("Page.captureScreenshot",{format:"png"});writeFileSync(out,Buffer.from(shot.result.data,"base64"))
        const evidence={url,at:new Date().toISOString(),expected,minimum_answers:answers,per_provider:perProvider,identity,...snapshot,screenshot:out}
        writeFileSync(out+".json",JSON.stringify(evidence,null,2)+"\n")
        console.log("PASS "+JSON.stringify(evidence));process.exitCode=0;break;
      }
    }
    await Bun.sleep(500)
  }
  if(!snapshot?.ready||!expected.every(s=>snapshot.slugs.includes(s))||snapshot.answers<answers||!expected.every(s=>snapshot.rows.some((r:any)=>r.slug===s&&r.answers>=perProvider&&(!identity||r.identity===identity))))throw Error("live page proof timed out: "+JSON.stringify({expected,answers,snapshot,errors}))
}finally{ws?.close();proc.kill();rmSync(profile,{recursive:true,force:true})}
