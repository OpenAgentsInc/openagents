// The shipped host, a real C/Wasm inbox, and real IndexedDB. Fault injections
// below exercise browser refusal paths; they are not storage durability claims.
import { afterAll, beforeAll, expect, test } from 'bun:test'
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { chrome as chromePath, gpuFlags } from "./chrome"

const root = join(import.meta.dir, '../..')
const temp = mkdtempSync(join(tmpdir(), 'coax-storage-'))
let server: ReturnType<typeof Bun.serve>, chrome: ReturnType<typeof Bun.spawn>, ws: WebSocket
let serial = 0
const pending = new Map<number, (v: any) => void>()
function cdp(method: string, params: any = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++serial, timeout = setTimeout(() => { pending.delete(id); reject(Error('CDP timeout ' + method)) }, 15000)
    pending.set(id, reply => { clearTimeout(timeout); reply.error ? reject(Error(JSON.stringify(reply.error))) : resolve(reply.result) })
    ws.send(JSON.stringify({ id, method, params }))
  })
}
async function evaluate(source: string) {
  const result = await cdp('Runtime.evaluate', { expression: source, awaitPromise: true, returnByValue: true })
  if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails))
  return result.result.value
}
async function ready() {
  for (let i = 0; i < 150; i++) { if (await evaluate('globalThis.coaxial?.ready === true')) return; await Bun.sleep(25) }
  throw Error('Wasm host did not become ready: ' + await evaluate('document.body.textContent'))
}
const face = `#include <stdint.h>
#define E(n) __attribute__((export_name(n)))
#define I(n) __attribute__((import_module("env"),import_name(n)))
I("coaxial_storage_put") int put(int,int,int,int,int,int,int,int,int);
I("coaxial_storage_get") int get(int,int,int,int,int,int,int);
I("coaxial_storage_delete") int del(int,int,int,int,int,int,int);
I("coaxial_storage_cancel") int cancel(int,int);
typedef struct {uint8_t inbox[4096];uint32_t replies,kind,length;} State;
static uint8_t scratch[1049600];static int capacity=4096;
E("coaxial_layout") int layout(void){return 1;}
E("coaxial_state_size") int size(void){return sizeof(State);}
E("coaxial_vertex_floats") int vf(void){return 4;}
E("coaxial_max_vertices") int mv(void){return 6;}
E("coaxial_uniform_floats") int uf(void){return 4;}
E("coaxial_frame") int frame(int a,int b,int c,int w,int h,float p,float dt,int s){return 0;}
E("coaxial_inbox") uintptr_t inbox(State*s){return(uintptr_t)s->inbox;}
E("coaxial_inbox_cap") int cap(void){return capacity;}
E("coaxial_message") void message(State*s,int kind,int n){if(kind<768 || kind>=800)return;s->replies++;s->kind=kind;s->length=n;}
E("probe_count") int count(State*s){return s->replies;}
E("probe_kind") int kind(State*s){return s->kind;}
E("probe_length") int length(State*s){return s->length;}
E("probe_clear") void clear(State*s){s->replies=0;s->length=0;}
E("probe_scratch") uintptr_t input(void){return(uintptr_t)scratch;}
E("probe_cap") void setcap(int n){capacity=n;}
E("probe_request") int request(int op,int slot,int token,int np,int nn,int version,int kp,int kn,int vp,int vn){
 if(op==0)return put(slot,token,np,nn,version,kp,kn,vp,vn);
 if(op==1)return get(slot,token,np,nn,version,kp,kn);
 return del(slot,token,np,nn,version,kp,kn);}
E("probe_cancel") int cancelled(int slot,int token){return cancel(slot,token);}
`
const shader = `@vertex fn bg_vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f {let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));return vec4f(p[i],0,1);}
@fragment fn bg_fs()->@location(0) vec4f{return vec4f(0,0,0,1);}
@vertex fn fx_vs(@location(0) p:vec4f)->@builtin(position) vec4f{return p;}
@fragment fn fx_fs()->@location(0) vec4f{return vec4f(1);}`
// Helpers call the actual imported C face, and decode the raw inbox header.
const helpers = `
globalThis.request = (op,slot,token,ns='fixture',version=1,key='record',bytes=[])=>{
 const x=engine.exports,p=x.probe_scratch(),u=new Uint8Array(memory.buffer),enc=new TextEncoder();
 const n=enc.encode(ns),k=enc.encode(key);u.set(n,p);u.set(k,p+128);u.set(bytes,p+512);
 return x.probe_request(op,slot,token,p,n.length,version,p+128,k.length,p+512,bytes.length);
};
globalThis.reply = ()=>{const x=engine.exports,s=arena.state,n=x.probe_length(s),p=x.coaxial_inbox(s);
 const d=new DataView(memory.buffer,p,Math.max(16,n));return {count:x.probe_count(s),kind:x.probe_kind(s),token:d.getUint32(0,true),status:d.getUint32(4,true),length:d.getUint32(8,true),needed:d.getUint32(12,true),bytes:Array.from(new Uint8Array(memory.buffer,p+16,Math.max(0,n-16)))};};
globalThis.clear = ()=>engine.exports.probe_clear(arena.state);
globalThis.waitReply = async()=>{for(let i=0;i<200;i++){if(reply().count)return reply();await new Promise(r=>setTimeout(r,5));}throw Error('storage reply timeout');};
globalThis.operation=async(...args)=>{clear();const accepted=request(...args);if(accepted!==0)throw Error('request refused '+accepted);return waitReply();};
`
beforeAll(async () => {
  writeFileSync(join(temp, 'face.c'), face)
  const compile = Bun.spawnSync(['/opt/homebrew/opt/llvm/bin/clang', '--target=wasm32', '-nostdlib', '-std=c11', '-O1', join(temp, 'face.c'), '-o', join(temp, 'probe.wasm'), '-Wl,--no-entry,--import-memory,--max-memory=67108864,--export=__heap_base,--allow-undefined'], { stdout: 'pipe', stderr: 'pipe' })
  if (compile.exitCode) throw Error(new TextDecoder().decode(compile.stderr))
  server = Bun.serve({ port: 0, fetch(req) {
    const path = new URL(req.url).pathname
    // Module-local observations are appended only in this test server. Every effect
    // and import is the shipped implementation; production exports no test hooks.
    if (path === '/host.js') return new Response(readFileSync(join(root, 'coax/web/host.js'), 'utf8') + `
      Object.defineProperties(globalThis,{engine:{get:()=>engine},arena:{get:()=>arena},memory:{get:()=>memory}});
      globalThis.makeByteStorage=makeByteStorage;
      globalThis.reloadEngine=bytes=>loadEngine(bytes,++wanted);
    `, { headers: { 'Content-Type': 'text/javascript' } })
    if (['/entropy.js', '/entropy-import.js', '/drop.js', '/uploads.js', '/voice.js', '/gpui.js', '/loader.js', '/http-requests.js'].includes(path)) return new Response(readFileSync(join(root, 'coax/web', path.slice(1))), { headers: { 'Content-Type': 'text/javascript' } })
    if (path === '/probe.wasm') return new Response(readFileSync(join(temp, 'probe.wasm')), { headers: { 'Content-Type': 'application/wasm' } })
    if (path === '/probe.wgsl') return new Response(shader)
    if (path === '/api/auth/viewer') return new Response('')
    return new Response('<!doctype html><html><body><canvas id="coaxial"></canvas><script type="module" src="/host.js" data-app="probe" data-wasm="/probe.wasm"></script></body></html>', { headers: { 'Content-Type': 'text/html' } })
  } })
  chrome = Bun.spawn([chromePath(), '--headless=new', '--mute-audio', `--user-data-dir=${join(temp, 'chrome')}`, '--no-first-run', '--hide-scrollbars', ...gpuFlags(), '--remote-debugging-port=0', 'about:blank'], { stdout: 'ignore', stderr: 'ignore' })
  let port = ''
  for (let i = 0; i < 100; i++) { try { port = readFileSync(join(temp, 'chrome/DevToolsActivePort'), 'utf8').split('\n')[0]; break } catch {} await Bun.sleep(50) }
  if (!port) throw Error('Chrome did not open a debugging port')
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json() as any[]
  ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl)
  await new Promise(resolve => ws.addEventListener('open', resolve, { once: true }))
  ws.addEventListener('message', e => { const r = JSON.parse(String(e.data)); pending.get(r.id)?.(r); pending.delete(r.id) })
  await cdp('Page.navigate', { url: `http://127.0.0.1:${server.port}/` }); await ready(); await evaluate(helpers)
}, 20000)
afterAll(async () => { ws?.close(); chrome?.kill(); if (chrome) await chrome.exited; server?.stop(true); rmSync(temp, { recursive: true, force: true }) })

test('C/Wasm byte storage survives reload, isolates namespaces/versions, and deletes durably', async () => {
  expect(await evaluate('operation(0,0,0xffffffff,"fixture",1,"record",[0,255,128,1,0])')).toMatchObject({ kind: 768, token: 0xffffffff, status: 0, length: 0 })
  expect(await evaluate('operation(0,1,2,"other",1,"record",[9])')).toMatchObject({ status: 0 })
  expect(await evaluate('operation(0,2,3,"fixture",2,"record",[8])')).toMatchObject({ status: 0 })
  await cdp('Page.reload'); await ready(); await evaluate(helpers)
  expect(await evaluate('operation(1,3,4)')).toMatchObject({ kind: 771, token: 4, status: 0, length: 5, needed: 5, bytes: [0,255,128,1,0] })
  expect(await evaluate('operation(1,4,5,"other")')).toMatchObject({ status: 0, bytes: [9] })
  expect(await evaluate('operation(1,5,6,"fixture",2)')).toMatchObject({ status: 0, bytes: [8] })
  expect(await evaluate('operation(2,6,7)')).toMatchObject({ status: 0 })
  await cdp('Page.reload'); await ready(); await evaluate(helpers)
  expect(await evaluate('operation(1,7,8)')).toMatchObject({ status: 1, length: 0, bytes: [] })
}, 15000)

test('put copies caller bytes before memory reuse/growth; empty bytes remain a present value', async () => {
  expect(await evaluate(`(async()=>{clear();const result=request(0,0,9,'copy',1,'k',[0,255,1]);new Uint8Array(memory.buffer).fill(7,engine.exports.probe_scratch()+512,engine.exports.probe_scratch()+515);memory.grow(1);if(result)throw Error(result);await waitReply();return operation(1,0,10,'copy',1,'k');})()`)).toMatchObject({ status: 0, bytes: [0,255,1] })
  expect(await evaluate(`(async()=>{await operation(0,0,11,'copy',1,'empty',[]);return operation(1,0,12,'copy',1,'empty');})()`)).toMatchObject({ status: 0, length: 0, bytes: [] })
})

test('malformed spans, invalid UTF-8, oversized input and insufficient inbox refuse without truncation', async () => {
  expect(await evaluate(`(()=>{clear();const x=engine.exports,p=x.probe_scratch();const results=[request(0,-1,1),request(0,8,1),request(0,0,0),request(0,0,1,'',1),request(0,0,1,'n',0)];new Uint8Array(memory.buffer)[p]=255;return [...results,x.probe_request(0,0,1,p,1,1,p,1,p,0),x.probe_request(0,0,1,-1,1,1,p,1,p,0),x.probe_request(0,0,1,p,1,1,p,1,p,1048577),reply().count];})()`)).toEqual([5,5,5,5,5,5,5,4,0])
  expect(await evaluate(`(()=>{engine.exports.probe_cap(15);const result=request(1,0,1);engine.exports.probe_cap(4096);return result;})()`)).toBe(9)
  expect(await evaluate(`(async()=>{await operation(0,0,1,'capacity',1,'k',new Array(4090).fill(43));return operation(1,0,2,'capacity',1,'k');})()`)).toMatchObject({ status: 9, needed: 4090, length: 0, bytes: [] })
  expect(await evaluate(`(async()=>{await operation(0,0,1,'capacity',1,'max',new Uint8Array(1048576).fill(51));return operation(1,0,2,'capacity',1,'max');})()`)).toMatchObject({ status: 9, needed: 1048576, length: 0, bytes: [] })
})

test('cancelled queued writes and old engine generations never reappear or deliver late replies', async () => {
  const result = await evaluate(`(async()=>{
    await operation(2,0,1,'cancel',1,'k');clear();
    // Hold a real readwrite transaction open while the host queues a put.
    const db=await new Promise((ok,no)=>{const r=indexedDB.open('coaxial-bytes',1);r.onsuccess=()=>ok(r.result);r.onerror=()=>no(r.error);});
    const tx=db.transaction('bytes','readwrite');let hold=true;const pump=()=>{const r=tx.objectStore('bytes').get(['cancel',1,'k']);r.onsuccess=()=>{if(hold)pump();};};pump();
    const accepted=request(0,0,55,'cancel',1,'k',[7]),busy=request(1,0,56,'cancel',1,'k'),wrong=engine.exports.probe_cancel(0,56),cancelled=engine.exports.probe_cancel(0,55);
    hold=false;await new Promise(r=>{tx.oncomplete=r;tx.onabort=r;});db.close();await new Promise(r=>setTimeout(r,20));const late=reply().count;
    const missing=await operation(1,0,56,'cancel',1,'k');clear();
    const fresh=await fetch('/probe.wasm').then(r=>r.arrayBuffer());
    const lockDb=await new Promise(ok=>{const r=indexedDB.open('coaxial-bytes',1);r.onsuccess=()=>ok(r.result);});
    const lock=lockDb.transaction('bytes','readwrite');let keep=true;const again=()=>{const r=lock.objectStore('bytes').get(['cancel',1,'k']);r.onsuccess=()=>{if(keep)again();};};again();
    const old=request(1,0,99,'cancel',1,'k');await reloadEngine(fresh);clear();
    const acceptedNew=request(1,0,99,'cancel',1,'k');keep=false;await new Promise(r=>{lock.oncomplete=r;lock.onabort=r;});lockDb.close();
    if(acceptedNew)throw Error('new generation refused '+acceptedNew);const current=await waitReply();await new Promise(r=>setTimeout(r,20));
    return {accepted,busy,wrong,cancelled,late,missing,current,finalCount:reply().count,old};
  })()`)
  expect(result).toMatchObject({ accepted: 0, busy: 6, wrong: 1, cancelled: 0, late: 0, missing: { status: 1 }, current: { status: 1, token: 99 }, finalCount: 1, old: 0 })
})

test('unavailable, quota, blocked open, transaction timeout, corrupt data and late success are explicit', async () => {
  const result = await evaluate(`(async()=>{
    const run=async(indexed,act)=>{const memory=new WebAssembly.Memory({initial:20}),u=new Uint8Array(memory.buffer);u.set([110],0);u.set([107],8);u.set([9],16);const replies=[];
      const host=makeByteStorage({memory:()=>memory,capacity:()=>64,emit:(slot,bytes)=>replies.push({slot,status:new DataView(bytes.buffer).getUint32(4,true)}),indexedDB:()=>indexed,timeout:20});
      const accepted=host.imports.coaxial_storage_put(0,1,0,1,1,8,1,16,1);if(act)await act(host);await new Promise(r=>setTimeout(r,50));host.reset();return {accepted,replies};};
    const unavailable=await run(undefined);
    const denied=await run({open(){throw new DOMException('refused','SecurityError');}});
    const quota=await run({open(){throw new DOMException('refused','QuotaExceededError');}});
    let blockedRequest,closed=0;
    const blocked=await run({open(){blockedRequest={};setTimeout(()=>blockedRequest.onblocked(),0);return blockedRequest;}},async()=>{await new Promise(r=>setTimeout(r,5));blockedRequest.result={close(){closed++;}};blockedRequest.onsuccess();});
    const timeout=await run({open(){return {};}});
    let transaction;
    const stalled=await run({open(){const r={result:{close(){},transaction(){transaction={objectStore(){return {put(){return {};}}},abort(){}};return transaction;}}};setTimeout(()=>r.onsuccess(),0);return r;}});
    const db=await new Promise(ok=>{const r=indexedDB.open('coaxial-bytes',1);r.onsuccess=()=>ok(r.result);});
    await new Promise((ok,no)=>{const tx=db.transaction('bytes','readwrite');tx.objectStore('bytes').put('wrong type',['corrupt',1,'k']);tx.oncomplete=ok;tx.onabort=()=>no(tx.error);});db.close();
    const corrupt=await operation(1,0,313,'corrupt',1,'k');
    const hugeDb=await new Promise(ok=>{const r=indexedDB.open('coaxial-bytes',1);r.onsuccess=()=>ok(r.result);});
    await new Promise((ok,no)=>{const tx=hugeDb.transaction('bytes','readwrite');tx.objectStore('bytes').put(new ArrayBuffer(1048577),['corrupt',1,'oversize']);tx.oncomplete=ok;tx.onabort=()=>no(tx.error);});hugeDb.close();
    const oversize=await operation(1,0,314,'corrupt',1,'oversize');
    return {unavailable,denied,quota,blocked,timeout,stalled,closed,corrupt,oversize};
  })()`)
  expect(result.unavailable).toEqual({ accepted: 0, replies: [{ slot: 0, status: 2 }] })
  expect(result.denied).toEqual({ accepted: 0, replies: [{ slot: 0, status: 2 }] })
  expect(result.quota).toEqual({ accepted: 0, replies: [{ slot: 0, status: 3 }] })
  expect(result.blocked).toEqual({ accepted: 0, replies: [{ slot: 0, status: 2 }] })
  expect(result.timeout).toEqual({ accepted: 0, replies: [{ slot: 0, status: 8 }] })
  expect(result.stalled).toEqual({ accepted: 0, replies: [{ slot: 0, status: 8 }] })
  expect(result.closed).toBe(1)
  expect(result.corrupt).toMatchObject({ status: 10, length: 0 })
  expect(result.oversize).toMatchObject({ status: 4, length: 0, needed: 1048577 })
})

test('fault-injected commit/abort and late callbacks cannot report success or retarget a reused slot', async () => {
  const result=await evaluate(`(async()=>{
    const mem=new WebAssembly.Memory({initial:1}),u=new Uint8Array(mem.buffer);u.set([110],0);u.set([107],8);u[16]=9;
    const replies=[],txs=[];let closes=0;
    const db={close(){closes++;},transaction(){const request={};const tx={db,request,objectStore(){return {put(){return request;},get(){return request;},delete(){return request;}}},abort(){}};txs.push(tx);return tx;}};
    const idb={open(){const r={result:db};queueMicrotask(()=>r.onsuccess());return r;}};
    const host=makeByteStorage({memory:()=>mem,capacity:()=>64,emit:(slot,b)=>{const h=new DataView(b.buffer);replies.push({slot,token:h.getUint32(0,true),status:h.getUint32(4,true)});},indexedDB:()=>idb});
    const put=(token)=>host.imports.coaxial_storage_put(0,token,0,1,1,8,1,16,1);
    put(1);await new Promise(r=>setTimeout(r,0));const beforeCommit=replies.length;
    txs[0].request.error=new DOMException('full','QuotaExceededError');txs[0].request.onerror();txs[0].onabort();
    put(2);await new Promise(r=>setTimeout(r,0));host.imports.coaxial_storage_cancel(0,2);
    put(2);await new Promise(r=>setTimeout(r,0));txs[1].oncomplete();const cancelledLate=replies.length;txs[2].oncomplete();
    put(3);await new Promise(r=>setTimeout(r,0));host.reset();put(3);await new Promise(r=>setTimeout(r,0));txs[3].oncomplete();const resetLate=replies.length;txs[4].oncomplete();
    put(4);await new Promise(r=>setTimeout(r,0));db.onversionchange();txs[5].oncomplete();
    return {beforeCommit,cancelledLate,resetLate,replies,closes,invalidCancel:host.imports.coaxial_storage_cancel(0,0)};
  })()`)
  expect(result).toEqual({beforeCommit:0,cancelledLate:1,resetLate:2,replies:[{slot:0,token:1,status:3},{slot:0,token:2,status:0},{slot:0,token:3,status:0},{slot:0,token:4,status:2}],closes:1,invalidCancel:5})
})
