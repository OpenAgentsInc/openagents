// Actual isolated Chrome/CDP fixture runner; no visible window or user profile.
import{existsSync,mkdtempSync,readFileSync,rmSync,writeFileSync,cpSync,mkdirSync}from'node:fs';
import{tmpdir}from'node:os';import{join,resolve}from'node:path';
const[normalPath,observePath,reportPath,stagePath]=process.argv.slice(2);if(!reportPath)throw Error('usage: bun browser.ts NORMAL.wasm OBSERVE.wasm REPORT.json');
const root=resolve(import.meta.dir,'../../..'),profile=mkdtempSync(join(tmpdir(),'coax-gate-browser-'));
const normal=new WebAssembly.Module(readFileSync(normalPath));if(WebAssembly.Module.exports(normal).some(x=>['test_fault','test_trap','clean'].includes(x.name)))throw Error('Fixture hooks in normal module');
const adapterSource=readFileSync(join(root,'coax/web/unlock/gate.ts'),'utf8').split("// The page's Coax code and this file talk through the Coaxial host")[0];
writeFileSync(join(profile,'legacy.ts'),adapterSource);
const adapter=await Bun.build({entrypoints:[join(profile,'legacy.ts')],target:'browser',minify:true});
const client=await Bun.build({entrypoints:[join(root,'coax/web/unlock/experimental/client.js')],target:'browser',minify:true});
if(!adapter.success||!client.success)throw Error('Experimental bundle failed');
const legacy=await Bun.build({entrypoints:[join(root,'coax/web/unlock/gate.ts')],target:'browser',minify:true});if(!legacy.success)throw Error('Legacy bundle failed');
const assets=new Map([
 ['/',{path:join(import.meta.dir,'browser.html'),type:'text/html'}],
 ['/fixture.json',{path:join(root,'coax/tests/nostr/data/phrase-gate-draft-2026-10-28.json'),type:'application/json'}],
 ['/normal/phrase-gate-worker.js',{path:join(root,'coax/lib/access/platform/phrase-gate-worker.js'),type:'application/javascript'}],
 ['/observe/phrase-gate-worker.js',{path:join(root,'coax/lib/access/platform/phrase-gate-worker.js'),type:'application/javascript'}],
 ['/init-fail/phrase-gate-worker.js',{path:join(root,'coax/lib/access/platform/phrase-gate-worker.js'),type:'application/javascript'}],
 ['/init-fail/phrase-gate-worker.wasm',{path:resolve(observePath,'../init-fail.wasm'),type:'application/wasm'}],
 ['/missing/phrase-gate-worker.js',{path:join(root,'coax/lib/access/platform/phrase-gate-worker.js'),type:'application/javascript'}],
 ['/normal/phrase-gate-worker.wasm',{path:resolve(normalPath),type:'application/wasm'}],
 ['/observe/phrase-gate-worker.wasm',{path:resolve(observePath),type:'application/wasm'}],
]);
const activePath=join(profile,'active');if(stagePath)mkdirSync(activePath);
const server=Bun.serve({hostname:'127.0.0.1',port:0,fetch(request){const url=new URL(request.url),path=url.pathname;if(path==='/__mode')return Response.json({rollback:!!stagePath});if(path==='/__switch'&&stagePath&&request.method==='POST'){const mode=url.searchParams.get('profile');if(!['baseline','experimental'].includes(mode??''))return new Response('',{status:400});rmSync(activePath,{recursive:true,force:true});cpSync(join(resolve(stagePath),mode!),activePath,{recursive:true});return Response.json({ok:true});}if(path.startsWith('/active/')&&stagePath){const name=path.slice(8)||'index.html';if(!/^[a-z0-9.-]+$/.test(name))return new Response('',{status:404});const target=join(activePath,name),type=name.endsWith('.wasm')?'application/wasm':name.endsWith('.js')?'application/javascript':name.endsWith('.json')?'application/json':'text/html';return existsSync(target)?new Response(Bun.file(target),{headers:{'content-type':type,'cache-control':'no-store'}}):new Response('',{status:404});}if(path==='/adapter.js')return new Response(adapter.outputs[0],{headers:{'content-type':'application/javascript'}});if(path==='/client.js')return new Response(client.outputs[0],{headers:{'content-type':'application/javascript'}});if(path.startsWith('/entropy/')){const mode=path.split('/')[2];return new Response(`const mode=${JSON.stringify(mode)};Object.defineProperty(self,'crypto',{value:{getRandomValues:mode==='absent'?undefined:array=>{if(mode==='partial')array[0]=255;if(mode==='wrong')return new Uint8Array(array.length);throw Error('test entropy');}}});const queued=[];onmessage=e=>queued.push(e);await import('/observe/phrase-gate-worker.js');const handler=onmessage;for(const e of queued)handler(e);`,{headers:{'content-type':'application/javascript'}});}if(path.startsWith('/fault/'))return new Response(`onmessage=e=>{const m=e.data;if(m.type==='init'){postMessage({type:'ready'});if(location.pathname.includes('after'))setTimeout(()=>{throw Error('test death')},10);}else if(m.type==='counts'){if(location.pathname.includes('death'))throw Error('test death');if(location.pathname.includes('refused'))postMessage({type:'refused',id:m.id});if(location.pathname.includes('run'))postMessage({type:'limits',id:m.id,error:0});}}`,{headers:{'content-type':'application/javascript'}});if(path==='/legacy.js')return new Response(legacy.outputs[0],{headers:{'content-type':'application/javascript'}});const asset=assets.get(path);return asset?new Response(Bun.file(asset.path),{headers:{'content-type':asset.type,'cache-control':'no-store'}}):new Response('',{status:404});}});
const chrome=Bun.spawn([process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','--headless=new',`--user-data-dir=${profile}`,'--no-first-run','--no-default-browser-check','--disable-background-networking','--disable-component-update','--disable-sync','--disable-background-timer-throttling','--disable-renderer-backgrounding','--disable-gpu','--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'});let socket:WebSocket|undefined;
try{
 const file=join(profile,'DevToolsActivePort');for(let i=0;i<100&&!existsSync(file);i++)await Bun.sleep(50);
 const port=readFileSync(file,'utf8').split('\n')[0],targets=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();socket=new WebSocket(targets.find((x:any)=>x.type==='page').webSocketDebuggerUrl);
 await new Promise(resolve=>socket!.addEventListener('open',resolve,{once:true}));let id=0;const pending=new Map<number,(x:any)=>void>();
 socket.addEventListener('message',event=>{const m=JSON.parse(String(event.data));if(m.id){pending.get(m.id)?.(m);pending.delete(m.id);}});
 const send=(method:string,params:any={})=>new Promise<any>(resolve=>{pending.set(++id,resolve);socket!.send(JSON.stringify({id,method,params}));});
 await send('Page.navigate',{url:`http://127.0.0.1:${server.port}/`});for(let i=0;i<100;i++){const r=await send('Runtime.evaluate',{expression:'typeof run'});if(r.result?.result?.value==='function')break;await Bun.sleep(20);}
 let timer:ReturnType<typeof setTimeout>;const deadline=new Promise((_r,reject)=>{timer=setTimeout(()=>reject(Error('gate browser deadline')),120000);});
 let result:any;try{result=await Promise.race([send('Runtime.evaluate',{expression:'run('+JSON.stringify(process.env.COAX_GATE_BROWSER_GROUP??'all')+')',awaitPromise:true,returnByValue:true}),deadline]);}finally{clearTimeout(timer!);}
 if(result.result?.exceptionDetails||!result.result?.result?.value)throw Error(JSON.stringify(result));
 const report=result.result.result.value;writeFileSync(reportPath,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({cases:report.cases,completed:report.completed,cancellation:report.cancellation,normalEntropyEntries:report.sealed.length}));
}finally{socket?.close();chrome.kill();server.stop(true);rmSync(profile,{recursive:true,force:true});}
