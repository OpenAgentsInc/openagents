// Headless Chrome/CDP only; never opens a visible window or takes focus.
import {existsSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const [modulePath,reportPath]=process.argv.slice(2);
if(!modulePath||!reportPath)throw Error('usage: bun browser.ts MODULE.wasm REPORT.json');
const root=resolve(import.meta.dir,'../../..'),profile=mkdtempSync(join(tmpdir(),'coax-pbkdf2-browser-'));
const assets=new Map([
 ['/',{path:join(import.meta.dir,'browser.html'),type:'text/html'}],
 ['/worker.js',{path:join(root,'coax/lib/crypto/platform/pbkdf2-worker.js'),type:'application/javascript'}],
 ['/rejection-worker.js',{path:join(import.meta.dir,'rejection-worker.js'),type:'application/javascript'}],
 ['/worker.wasm',{path:resolve(modulePath),type:'application/wasm'}],
]);
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(request){const asset=assets.get(new URL(request.url).pathname);return asset?new Response(Bun.file(asset.path),{headers:{'content-type':asset.type}}):new Response('',{status:404});}});
const chrome=Bun.spawn([process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','--headless=new',`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--disable-background-networking','--disable-component-update','--disable-sync','--disable-background-timer-throttling','--disable-renderer-backgrounding','--enable-unsafe-webgpu','--use-angle=metal','--window-size=800,600','--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'});
let socket:WebSocket|undefined;
try{
 const portFile=join(profile,'DevToolsActivePort');for(let i=0;i<100&&!existsSync(portFile);i++)await Bun.sleep(50);
 const port=readFileSync(portFile,'utf8').split('\n')[0],targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
 socket=new WebSocket(targets.find((x:any)=>x.type==='page').webSocketDebuggerUrl);
 await new Promise(resolve=>socket!.addEventListener('open',resolve,{once:true}));
 let id=0;const pending=new Map<number,(result:any)=>void>();
 socket.addEventListener('message',event=>{const m=JSON.parse(String(event.data));if(m.id){pending.get(m.id)?.(m);pending.delete(m.id);}});
 const send=(method:string,params:any={})=>new Promise<any>(resolve=>{pending.set(++id,resolve);socket!.send(JSON.stringify({id,method,params}));});
 await send('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});
 for(let i=0;i<100;i++){const ready=await send('Runtime.evaluate',{expression:'typeof run'});if(ready.result?.result?.value==='function')break;await Bun.sleep(20);}
 let timer:ReturnType<typeof setTimeout>;
 const deadline=new Promise((_resolve,reject)=>{timer=setTimeout(()=>reject(Error('browser worker deadline')),120000);});
 let result:any;try{result=await Promise.race([send('Runtime.evaluate',{expression:'run()',awaitPromise:true,returnByValue:true}),deadline]);}finally{clearTimeout(timer!);}
 if(result.result?.exceptionDetails||!result.result?.result?.value)throw Error(JSON.stringify(result));
 const report=result.result.result.value;writeFileSync(reportPath,JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({gate:report.gate.map((x:any)=>({budget:x.budget,elapsedMs:x.elapsedMs,frames:x.frames,maxChunkMs:x.maxChunkMs})),cancelled:report.cancelled.completed}));
}finally{socket?.close();chrome.kill();server.stop(true);rmSync(profile,{recursive:true,force:true});}
