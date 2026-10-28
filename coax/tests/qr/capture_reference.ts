// Optional fixture refresh: the primary live demo runs only in a fresh
// headless browser security sandbox. No downloaded code runs as a native tool.
import{mkdtempSync,readFileSync,writeFileSync,existsSync,rmSync}from'node:fs';
import{join}from'node:path';import{tmpdir}from'node:os';
const lengths=[0,14,15,27,42,43,62,70,84,106,122,152,180,212,240,254,300,350,366,412];
const strings=lengths.map(n=>Array.from({length:n},(_,i)=>'Coax qr0123456789-._~'[i%20]).join(''));
strings[7]='https://openagents.com/signin?challenge='+'0'.repeat(70-'https://openagents.com/signin?challenge='.length);
const connect='nostrconnect://'+'0'.repeat(64)+'?relay=wss%3A%2F%2Frelay.openagents.com&metadata=%7B%22name%22%3A%22Coax%22%7D&test=';
strings[16]=connect+'0'.repeat(300-connect.length);
const profile=mkdtempSync(join(tmpdir(),'coax-qr-reference-'));
const chrome=process.env.CHROME??'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const proc=Bun.spawn([chrome,'--headless=new','--no-first-run',`--user-data-dir=${profile}`,'--remote-debugging-port=0','about:blank'],{stdout:'ignore',stderr:'ignore'});
let ws:WebSocket|undefined,deadline:ReturnType<typeof setTimeout>|undefined;
try{
 const portFile=join(profile,'DevToolsActivePort');for(let i=0;i<100&&!existsSync(portFile);i++)await Bun.sleep(50);
 const port=readFileSync(portFile,'utf8').split('\n')[0],target=(await(await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((v:any)=>v.type==='page');
 ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws!.addEventListener('open',r,{once:true}));
 let id=0;const pending=new Map<number,(v:any)=>void>();
 ws.addEventListener('message',e=>{const m=JSON.parse(String(e.data));pending.get(m.id)?.(m);pending.delete(m.id);});
 const send=(method:string,params:any={})=>new Promise<any>(r=>{pending.set(++id,r);ws!.send(JSON.stringify({id,method,params}));});
 await send('Network.enable');await send('Network.setBlockedURLs',{urls:['*googlesyndication*','*googletagmanager*','*google-analytics*']});
 await send('Page.navigate',{url:'https://www.nayuki.io/page/qr-code-generator-library'});
 let ready=false;for(let i=0;i<200;i++){
  const v=await send('Runtime.evaluate',{expression:'typeof qrcodegen!=="undefined" && typeof qrcodegen.QrCode!=="undefined"',returnByValue:true});if(v.result?.result?.value){ready=true;break;}await Bun.sleep(100);
 }if(!ready)throw Error('Primary reference unavailable; keep existing frozen fixtures');
 const expression=`(async()=>{
 const sourceUrl='https://www.nayuki.io/res/qr-code-generator-library/qrcodegen.js';
 const source=await(await fetch(sourceUrl)).arrayBuffer();
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',source)),b=>b.toString(16).padStart(2,'0')).join('');
 const rows=${JSON.stringify(strings)}.map(text=>{
  const bytes=new TextEncoder().encode(text),code=qrcodegen.QrCode.encodeSegments([qrcodegen.QrSegment.makeBytes(bytes)],qrcodegen.QrCode.Ecc.MEDIUM,1,15,-1,false);
  return{text,bytes:Array.from(bytes),version:code.version,size:code.size,mask:code.mask,penalty:code.getPenaltyScore(),rows:Array.from({length:code.size},(_,y)=>Array.from({length:code.size},(_,x)=>code.getModule(x,y)?'1':'0').join(''))};
 });return{schema:'coax.qr-reference.v1',sourceUrl,sourceSha256:digest,reference:'Project Nayuki primary live demo, byte mode/M/no ECC boost, automatic mask, versions1..15',rows};})()`;
 const result=await Promise.race([send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true}),new Promise((_,reject)=>{deadline=setTimeout(()=>reject(Error('reference budget30s')),30000);})])as any;
 if(result.result?.exceptionDetails||!result.result?.result?.value)throw Error(JSON.stringify(result.result));
 writeFileSync(join(import.meta.dir,'reference-2026-10-28.json'),JSON.stringify(result.result.result.value,null,2)+'\n');console.log('Frozen20 reference module grids');
}finally{if(deadline)clearTimeout(deadline);ws?.close();proc.kill();rmSync(profile,{recursive:true,force:true});}
