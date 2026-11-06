// Firm reason: 1 (browser SDK fixture: real streams/WebCrypto/gzip and concrete ownership; Coax admits every observation).
import {openPackHost} from '../pack-host.js';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test,label){const end=performance.now()+5000;while(!test()){if(performance.now()>end)throw Error(`pack fixture:${label}`);await sleep(5);}}
const hex=b=>Array.from(new Uint8Array(b),v=>v.toString(16).padStart(2,'0')).join('');
export async function physicalPackFixture(){
 const host=await openPackHost(),progress=[],url=path=>`http://127.0.0.1:9187/${path}`;
 const result=await host.load({url:url('split')},{owner:'physical',progress:(loaded,total)=>progress.push([loaded,total])});
 const out=[result.bytes,result.entries.length,Number(progress.some(([n,t])=>n===3&&t===0)),Number(progress.some(([n,t])=>n===11&&t===0)),Number(progress.some(([n,t])=>n===12&&t===512))];
 await host.retire('physical');let refused=0;try{await host.load({url:url('cut')},{owner:'cut'});}catch{refused=1;}out.push(refused);
 let started=false;const held=host.load({url:url('hold')},{owner:'held',progress:()=>{started=true;}}).catch(()=>null);await wait(()=>started,'physical held bytes');await host.retire('held');await held;
 let closed=false;const until=performance.now()+5000;while(!closed&&performance.now()<until){closed=await(await fetch(url('close-count'))).text()==='1';if(!closed)await sleep(10);}out.push(Number(closed));const state=host.info();out.push(state.owners,state.storage[0],state.storage[1]);return out;
}
export async function packFixture(){
 const host=await openPackHost(),bytes=new Uint8Array(await(await fetch('/fixture.pack')).arrayBuffer()),originalFetch=globalThis.fetch,originalDecrypt=crypto.subtle.decrypt.bind(crypto.subtle),routes=new Map(),out={},unblock=[];
 const totals=()=>{const i=host.info();return [i.owners,i.storage[0],i.storage[1]];},refuse=async p=>{try{await p;return 0;}catch{return 1;}},source=(path)=>({url:path}),response=b=>new Response(b,{status:200});
 globalThis.fetch=(url,options)=>routes.has(new URL(url,location.origin).pathname)?routes.get(new URL(url,location.origin).pathname)(options):originalFetch(url,options);
 async function privateInput(plain=bytes,wrongDigest=false){
  const c=new CompressionStream('gzip'),read=new Response(c.readable).arrayBuffer(),writer=c.writable.getWriter();await writer.write(plain);await writer.close();const gzip=await read,key=Uint8Array.from({length:32},(_,i)=>i),iv=Uint8Array.from({length:16},(_,i)=>i),cryptoKey=await crypto.subtle.importKey('raw',key,'AES-CTR',false,['encrypt']);
  const cipher=new Uint8Array(await crypto.subtle.encrypt({name:'AES-CTR',counter:iv,length:64},cryptoKey,gzip));const metadata={url:'/private-cipher',size:cipher.byteLength,bytes:bytes.byteLength,key:hex(key),iv:hex(iv),sha256:wrongDigest?'0'.repeat(64):hex(await crypto.subtle.digest('SHA-256',bytes))};
  routes.set('/private-session',()=>response(JSON.stringify(metadata)));routes.set('/private-cipher',()=>response(cipher));return metadata;
 }
 try{
  const progress=[];routes.set('/split',()=>response(new ReadableStream({start(c){for(const [from,to]of [[0,3],[3,11],[11,12],[12,bytes.length]])c.enqueue(bytes.subarray(from,to));c.close();}})));
  let result=await host.load(source('/split'),{owner:'public',progress:(l,t)=>progress.push(l,t)});const retained=result.entries.map(e=>e.bytes.slice());out.progress=progress;out.public=[result.bytes,result.entries.length,...result.entries.map(e=>e.bytes.byteLength)];await host.retire('public');out.public.push(...totals());
  await privateInput();result=await host.load({url:'/private-session',broker:true},{owner:'private'});out.private=[result.bytes,result.entries.length,Number(result.entries.every((e,i)=>e.bytes.length===retained[i].length&&e.bytes.every((v,k)=>v===retained[i][k])))];await host.retire('private');out.private.push(...totals());
  out.refusals=[];
  for(const data of [bytes.subarray(0,11),bytes.subarray(0,511),new Uint8Array(513),(()=>{const b=bytes.slice();b[0]=0;return b;})(),(()=>{const b=bytes.slice();b[32+4]=0;return b;})()]){routes.set('/bad',()=>response(data));out.refusals.push(await refuse(host.load(source('/bad'),{owner:'bad'})));}
  await privateInput(bytes,true);out.refusals.push(await refuse(host.load({url:'/private-session',broker:true},{owner:'bad-digest'})));
  const bomb=new Uint8Array(513);bomb.set(bytes);await privateInput(bomb);out.refusals.push(await refuse(host.load({url:'/private-session',broker:true},{owner:'bad-gzip'})));out.refusals.push(...totals());
  let cancelCount=0,releaseCancel;const heldCancel=new Promise(r=>{releaseCancel=r;});unblock.push(()=>releaseCancel());routes.set('/cancel',()=>response(new ReadableStream({start(c){c.enqueue(bytes.subarray(0,3));},cancel(){cancelCount++;return heldCancel;}})));
  const loading=host.load(source('/cancel'),{owner:'cancel'}).catch(()=>null);await wait(()=>host.info().readers===1,'active reader');await sleep(10);const before=host.info();let retired=false;const retiring=host.retire('cancel').then(()=>{retired=true;});await wait(()=>cancelCount===1,'actual reader cancellation');await sleep(5);out.cancel=[before.readers,before.storage[0],before.storage[1],cancelCount,Number(!retired)];releaseCancel();await retiring;await loading;out.cancel.push(...totals());
  await privateInput();const decrypt=originalDecrypt;let releaseCrypto,entered=false;const heldCrypto=new Promise(r=>{releaseCrypto=r;});unblock.push(()=>releaseCrypto());crypto.subtle.decrypt=async(...args)=>{const answer=await decrypt(...args);entered=true;await heldCrypto;return answer;};
  const pending=host.load({url:'/private-session',broker:true},{owner:'crypto'}).catch(()=>null);await wait(()=>entered,'actual decrypt completed behind held delivery');const state=host.info();let closed=false;const closing=host.retire('crypto').then(()=>{closed=true;});await sleep(10);out.crypto=[state.sdkPending,state.storage[0],state.storage[1],Number(!closed)];releaseCrypto();await closing;await pending;crypto.subtle.decrypt=decrypt;out.crypto.push(...totals());
  routes.set('/whole',()=>response(bytes));result=await host.load(source('/whole'),{owner:'borrow'});const borrowed=result.entries[0].bytes;let releaseBorrow;const heldBorrow=new Promise(r=>{releaseBorrow=r;});unblock.push(()=>releaseBorrow());const using=refuse(result.use(async()=>{await heldBorrow;}));let finished=false;const closeBorrow=host.retire('borrow').then(()=>{finished=true;});await sleep(5);out.borrow=[Number(!finished),borrowed.byteLength];releaseBorrow();const rejected=await using;await closeBorrow;out.borrow.push(borrowed.byteLength,...totals(),rejected);
  for(let i=0;i<31;i++)await host.load(source('/whole'),{owner:'full'});const full=host.info(),Instance=WebAssembly.Instance;let constructs=0;WebAssembly.Instance=new Proxy(Instance,{construct(target,args){constructs++;return Reflect.construct(target,args);}});let refused;try{refused=await refuse(Promise.resolve().then(()=>host.load(source('/whole'),{owner:'overflow'})));}finally{WebAssembly.Instance=Instance;}
  out.full=[full.owners,full.storage[0],full.storage[1],refused,constructs,host.info().owners];await host.retire('full');out.full.push(...totals());return out;
 }finally{globalThis.fetch=originalFetch;crypto.subtle.decrypt=originalDecrypt;for(const release of unblock)release();for(const owner of ['public','private','bad','bad-digest','bad-gzip','cancel','crypto','borrow','full','overflow'])await host.retire(owner);}
}
