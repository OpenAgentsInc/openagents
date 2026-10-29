// Headless-only actual Chrome/Wasm observer. No visible app window.
import{mkdtempSync,readFileSync,writeFileSync,existsSync,rmSync}from'node:fs';
import{tmpdir}from'node:os';import{join,resolve}from'node:path';
const directory=resolve(process.argv[2]??'coax/bin/nostr-timing-2026-10-28');
const source=resolve(import.meta.dir,'browser.html');
const headers={'Cross-Origin-Opener-Policy':'same-origin','Cross-Origin-Embedder-Policy':'require-corp'};
const server=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request){
 const path=new URL(request.url).pathname;
 if(request.method==='POST'&&/^\/raw\/(fixed|branch|index)$/.test(path)){
  const data=await request.arrayBuffer();if(data.byteLength!==9000012)return new Response('size',{status:400});
  writeFileSync(join(directory,'browser-'+path.split('/').at(-1)+'.bin'),Buffer.from(data));return new Response('ok',{headers});
 }
 if(path==='/timing.wasm')return new Response(Bun.file(join(directory,'timing.wasm')),{headers:{...headers,'content-type':'application/wasm'}});
 if(path==='/')return new Response(Bun.file(source),{headers:{...headers,'content-type':'text/html'}});
 return new Response('missing',{status:404});
}});
const chrome=process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const profile=mkdtempSync(join(tmpdir(),'coax-timing-browser-'));
// Request optimizing-only Wasm to avoid silently mixing baseline and optimized
// tiers. This is a requested engine configuration, not a JIT attestation.
const flags=['--headless=new','--mute-audio','--no-first-run','--disable-background-timer-throttling','--js-flags=--no-liftoff','--remote-debugging-port=0'];
const proc=Bun.spawn([chrome,...flags,`--user-data-dir=${profile}`,'about:blank'],{stdout:'ignore',stderr:Bun.file(join(directory,'chrome.stderr'))});
let ws:WebSocket|undefined,deadline:ReturnType<typeof setTimeout>|undefined;
try{
 const portFile=join(profile,'DevToolsActivePort');for(let i=0;i<100&&!existsSync(portFile);i++)await Bun.sleep(50);
 const port=readFileSync(portFile,'utf8').split('\n')[0];
 const info=await(await fetch(`http://127.0.0.1:${port}/json/version`)).json();
 const target=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t:any)=>t.type==='page');
 ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws!.addEventListener('open',r,{once:true}));
 let id=0;const pending=new Map<number,(value:any)=>void>();
 ws.addEventListener('message',event=>{const m=JSON.parse(String(event.data));pending.get(m.id)?.(m);pending.delete(m.id);});
 const send=(method:string,params:any={})=>new Promise<any>(r=>{pending.set(++id,r);ws!.send(JSON.stringify({id,method,params}));});
 await send('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
 for(let i=0;i<100;i++){
  const v=await send('Runtime.evaluate',{expression:'Boolean(window.completed)',returnByValue:true});if(v.result?.result?.value)break;await Bun.sleep(50);
 }
 const result=await Promise.race([send('Runtime.evaluate',{expression:'window.completed',awaitPromise:true,returnByValue:true}),new Promise((_,reject)=>{deadline=setTimeout(()=>reject(Error('browser budget90s')),90000);})])as any;
 clearTimeout(deadline);deadline=undefined;
 if(result.result?.exceptionDetails||!result.result?.result?.value)throw Error(JSON.stringify(result.result?.exceptionDetails??result));
 const report={...result.result.result.value,browser:info.Browser,engine:info['V8-Version'],flags,tier:'optimizing-only requested via --no-liftoff; tier not independently attested',isolationHeaders:headers};
 writeFileSync(join(directory,'browser-report.json'),JSON.stringify(report,null,2)+'\n');console.log(join(directory,'browser-report.json'));
}finally{if(deadline)clearTimeout(deadline);ws?.close();proc.kill();server.stop(true);rmSync(profile,{recursive:true,force:true});}
