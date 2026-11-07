// Firm reason: 1 (browser SDK: measured stream copies, cancellation, WebCrypto and decompression; Coax owns admission).
const wasmURL=new URL('../build/host-packs.wasm',import.meta.url);
let compiled,opened;
async function packModule(){compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error(`pack reactor:${r.status}`);return WebAssembly.compile(await r.arrayBuffer());})().catch(e=>{compiled=null;throw e;});return compiled;}
function instance(module,onMemory){
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 try{
 onMemory?.(memory.buffer.byteLength);
 const wasi={proc_exit:code=>{throw Error(`pack reactor exited:${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  clock_time_get(id,p,at){new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor((id===0?Date.now():performance.now())*1e6)),true);return 0;},
  fd_write(fd,iov,count,written){const d=new DataView(memory.buffer);let bytes=0;for(let i=0;i<count;i++)bytes+=d.getUint32(iov+8*i+4,true);d.setUint32(written,bytes,true);return 0;}};
 const x=new WebAssembly.Instance(module,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();return {x,memory};
 }catch(error){new Uint8Array(memory.buffer).fill(0);memory.grow(0);throw error;}
}
export async function openPackHost(){
 opened??=(async()=>{
  const module=await packModule(),master=instance(module),ops=new Set();master.x.pack_storage_open();
  function reserve(bytes){if(master.x.pack_storage_reserve(BigInt(bytes))!==0)throw Error('Coax refused retained pack storage');return [0,1,2].map(f=>master.x.pack_storage_ticket(f));}
  function release(ticket){if(!master.x.pack_storage_release(...ticket))throw Error('Coax refused pack storage release');}
  const masterTicket=reserve(master.memory.buffer.byteLength);
  const info=()=>({storage:[0,1,2,3].map(f=>Number(master.x.pack_storage_info(f))),owners:ops.size,loading:[...ops].filter(o=>o.loading).length,published:[...ops].filter(o=>o.published).length,readers:[...ops].reduce((n,o)=>n+o.readers.size,0),sdkPending:[...ops].reduce((n,o)=>n+o.sdkPending,0),copied:[...ops].reduce((n,o)=>n+o.copied,0)});
  function operation(owner,current,progress,expected,raw=false,observe){
   const tickets=[];let plan=null,context,constructingBytes=0;
   if(expected!==undefined){
    if(!Number.isSafeInteger(expected)||master.x.pack_storage_plan(BigInt(expected),Number(raw))!==0)throw Error('Coax refused complete transport plan');
    plan=Object.freeze({reactor:Number(master.x.pack_plan_info(1)),external:Number(master.x.pack_plan_info(2)),reserved:Number(master.x.pack_plan_info(3)),grow:!!master.x.pack_plan_info(4),heapBase:Number(master.x.pack_plan_info(5)),initial:master.memory.buffer.byteLength});
    for(let i=0;i<master.x.pack_plan_info(0);i++)tickets.push([0,1,2].map(f=>master.x.pack_plan_ticket(i,f)));
   }else tickets.push(reserve(master.memory.buffer.byteLength));
   const leases=()=>tickets.map(t=>t.map(String));
   try{if(observe){if(!plan)throw Error('transport observation needs a complete plan');observe({kind:'plan',plan,tickets:leases(),storage:info().storage});}context=instance(module,bytes=>{constructingBytes=bytes;observe?.({kind:'allocate',reactor:bytes,external:0});});}catch(error){for(const ticket of tickets)release(ticket);observe?.({kind:'constructor-refused',reactor:constructingBytes,external:0,tickets:leases(),storage:info().storage});throw error;}
   const op={owner,context,current,progress,plan,tickets,buffers:[],readers:new Map(),controller:new AbortController(),loading:true,published:false,retired:false,borrowers:0,sdkPending:0,copied:0};
   op.observe=observe;
   op.closed=new Promise(resolve=>{op.closedResolve=resolve;});ops.add(op);
   op.check=()=>{if(op.retired||!op.current()){op.abort();throw new DOMException('pack owner retired','AbortError');}};
   op.cancel=reader=>{let pending=op.readers.get(reader);if(!pending){pending=Promise.resolve().then(()=>reader.cancel()).catch(()=>{});op.readers.set(reader,pending);}return pending;};
   op.dispose=()=>{if(op.loading||op.borrowers||!op.retired||!op.context)return;const {x,memory}=op.context,reactor=memory.buffer.byteLength,external=op.buffers.reduce((n,b)=>n+b.byteLength,0),original=leases();x.pack_close();new Uint8Array(memory.buffer).fill(0);memory.grow(0);op.context=null;for(const buffer of op.buffers)buffer.fill(0);op.buffers.length=0;for(const ticket of op.tickets)release(ticket);op.tickets.length=0;ops.delete(op);op.closedResolve();observe?.({kind:'release',reactor,external,tickets:original,storage:info().storage});};
   op.abort=()=>{if(!op.retired){op.retired=true;op.controller.abort();for(const reader of op.readers.keys())op.cancel(reader);}op.dispose();};
   op.reserve=bytes=>{const ticket=reserve(bytes);op.tickets.push(ticket);return ticket;};
   op.free=ticket=>{release(ticket);op.tickets.splice(op.tickets.indexOf(ticket),1);};
   return op;
  }
  async function consume(op,body,chunk){
   op.check();if(!body)throw Error('pack response has no stream');const reader=body.getReader();op.readers.set(reader,null);let finished=false;
   try{for(;;){const value=await reader.read();op.check();if(value.done){finished=true;break;}chunk(value.value);}}
   finally{if(!finished)await op.cancel(reader);else if(op.readers.get(reader))await op.readers.get(reader);reader.releaseLock();op.readers.delete(reader);}
  }
  function arena(op,bytes){const {x,memory}=op.context;if(!x.pack_arena(BigInt(bytes)))throw Error('Coax refused the complete pack arena');const required=Number(x.pack_arena_info(2));if(op.plan&&required>op.plan.reactor)throw Error('pack arena exceeds admitted transport plan');if(required>memory.buffer.byteLength){const delta=required-memory.buffer.byteLength,ticket=op.plan?null:op.reserve(delta);try{memory.grow(delta/65536);}catch(error){if(ticket)op.free(ticket);throw error;}op.observe?.({kind:'allocate',reactor:delta,external:0});}return Number(x.pack_arena_info(0));}
  async function metadata(op,response){
   const {x,memory}=op.context;let loaded=0;await consume(op,response.body,value=>{if(!x.pack_metadata_offer(BigInt(loaded),BigInt(value.byteLength)))throw Error('Coax refused broker metadata extent');new Uint8Array(memory.buffer,x.pack_metadata_ptr()+loaded,value.byteLength).set(value);loaded+=value.byteLength;});
   if(!x.pack_private_parse(loaded))throw Error('Coax refused broker metadata');return {source:Number(x.pack_private_info(0)),plain:Number(x.pack_private_info(1)),url:new TextDecoder().decode(new Uint8Array(memory.buffer,Number(x.pack_private_info(3)),Number(x.pack_private_info(2))))};
  }
  async function read(op,body,expected,encoded,report=true){
   const {x,memory}=op.context;if(!x.pack_stream_open(BigInt(expected),Number(encoded)))throw Error('Coax refused pack stream extent');let target=null,address=0,ticket=null;
   await consume(op,body,value=>{
    const prefix=value.subarray(0,x.pack_prefix_capacity());new Uint8Array(memory.buffer,x.pack_prefix_ptr(),prefix.byteLength).set(prefix);
    if(!x.pack_stream_offer(BigInt(value.byteLength),prefix.byteLength))throw Error('Coax refused pack stream chunk');const at=Number(x.pack_stream_chunk(0)),total=Number(x.pack_stream_chunk(2));
    if(total&&!target){if(encoded){if(op.plan&&total!==op.plan.external)throw Error('encoded source differs from admitted transport plan');ticket=op.plan?null:op.reserve(total);try{target=new Uint8Array(total);op.buffers.push(target);}catch(error){if(ticket)op.free(ticket);throw error;}op.observe?.({kind:'allocate',reactor:0,external:total});}
     else{address=arena(op,total);target=new Uint8Array(memory.buffer,address,total);if(at)target.set(new Uint8Array(memory.buffer,x.pack_stream_head_ptr(),at));}}
    if(target)target.set(value,at);op.copied+=value.byteLength;if(report)op.progress?.(at+value.byteLength,total);
   });
   if(!x.pack_stream_finish())throw Error('Coax refused incomplete pack stream');return {target,address,ticket};
  }
  function load(pack,{owner,current=()=>true,progress,observe}={}){
   if(pack.broker&&pack.bytes!==undefined)throw Error('private transport needs its separately admitted crypto plan');
   const op=operation(owner,current,progress,pack.bytes,false,observe);
   op.finished=(async()=>{try{
    let response=null,session=null;
    for(const url of pack.urls||[pack.url]){
     op.check();if(pack.broker){const broker=await fetch(url,{method:'POST',cache:'no-store',credentials:'same-origin',signal:op.controller.signal});op.check();if(!broker.ok){await broker.body?.cancel();continue;}session=await metadata(op,broker);op.check();}
     response=await fetch(session?new URL(session.url,location.origin):url,{...(pack.quiet?{cache:'no-store'}:{}),signal:op.controller.signal});op.check();if(response.ok&&response.body)break;await response.body?.cancel();response=null;
    }
    if(!response)throw Error('pack source unavailable');let plain;
    if(session){
     const cipher=await read(op,response.body,session.source,true),{x,memory}=op.context;op.check();const gzTicket=op.reserve(session.source);let gzip;
     op.sdkPending++;try{const key=await crypto.subtle.importKey('raw',new Uint8Array(memory.buffer,Number(x.pack_private_info(4)),32),'AES-CTR',false,['decrypt']);op.check();gzip=new Uint8Array(await crypto.subtle.decrypt({name:'AES-CTR',counter:new Uint8Array(memory.buffer,Number(x.pack_private_info(5)),16),length:64},key,cipher.target));op.buffers.push(gzip);op.check();}finally{op.sdkPending--;}
     cipher.target.fill(0);op.buffers.splice(op.buffers.indexOf(cipher.target),1);op.free(cipher.ticket);
     if(!x.pack_stream_open(BigInt(session.source),1)||!x.pack_stream_offer(BigInt(gzip.byteLength),0)||!x.pack_stream_finish())throw Error('Coax refused SDK decrypt output extent');
     const source=new ReadableStream({start(c){c.enqueue(gzip);c.close();}});plain=await read(op,source.pipeThrough(new DecompressionStream('gzip')),session.plain,false,false);gzip.fill(0);op.buffers.splice(op.buffers.indexOf(gzip),1);op.free(gzTicket);
    }else plain=await read(op,response.body,pack.bytes??0,false);
    op.check();const {x,memory}=op.context;if(!x.pack_admit(Number(!!session)))throw Error('Coax refused complete pack identity, layout or catalog');
    const entries=[];for(let i=0;i<x.pack_entry_count();i++){const fields=[0,1,2,3,4,5].map(f=>Number(x.pack_entry(i,f)));entries.push(Object.freeze({kind:fields[0],licence:fields[1],name:new TextDecoder().decode(new Uint8Array(memory.buffer,fields[4],fields[5])),bytes:plain.target.subarray(fields[2],fields[2]+fields[3])}));}
    op.published=true;const result=Object.freeze({entries:Object.freeze(entries),source:plain.target,bytes:plain.target.byteLength,owner,close:()=>op.abort(),async use(fn){op.check();op.borrowers++;try{const answer=await fn(result);op.check();return answer;}finally{op.borrowers--;op.dispose();}}});return result;
   }catch(error){op.retired=true;op.controller.abort();throw error;}finally{op.loading=false;op.dispose();}})();return op.finished;
  }
  // A byte transfer is not an admitted asset. The consumer must authenticate
  // and decode the complete result through its Coax content authority.
  function loadBytes(url,expected,{owner,current=()=>true,observe}={}){
   const op=operation(owner,current,undefined,expected,true,observe);
   op.finished=(async()=>{let response;try{
    if(!Number.isSafeInteger(expected)||!op.context.x.pack_stream_open(BigInt(expected),1))throw Error('Coax refused exact byte transfer extent');
    response=await fetch(url,{cache:'no-store',signal:op.controller.signal});op.check();
    if(!response.ok||!response.body){await response.body?.cancel();throw Error('byte source unavailable');}
    const result=await read(op,response.body,expected,true,false);op.check();op.published=true;
    const transfer=Object.freeze({bytes:result.target,owner,close:()=>op.abort(),async use(fn){op.check();op.borrowers++;try{const value=await fn(transfer);op.check();return value;}finally{op.borrowers--;op.dispose();}}});return transfer;
   }catch(error){op.retired=true;op.controller.abort();await response?.body?.cancel().catch(()=>{});throw error;}finally{op.loading=false;op.dispose();}})();return op.finished;
  }
  function retainBytes(bytes){let ticket=reserve(bytes);return Object.freeze({bytes,release(){if(!ticket)throw Error('retained storage already released');release(ticket);ticket=null;}});}
  return Object.freeze({load,loadBytes,retainBytes,info,abortReads(owner){for(const op of ops)if(op.owner===owner&&!op.published)op.abort();},async retire(owner){const selected=[...ops].filter(op=>op.owner===owner);for(const op of selected)op.abort();await Promise.allSettled(selected.map(op=>op.finished));await Promise.all(selected.map(op=>op.closed));},masterBytes:master.memory.buffer.byteLength});
 })().catch(error=>{opened=null;throw error;});return opened;
}
