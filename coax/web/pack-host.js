// Firm reason: 1 (browser SDK: measured stream copies, cancellation, WebCrypto and decompression; Coax owns admission).
const wasmURL=new URL('../build/host-packs.wasm',import.meta.url);
let compiled,opened,moduleBytes=0,moduleSourceCleared=false;
async function packModule(){compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error(`pack reactor:${r.status}`);const bytes=await r.arrayBuffer();moduleBytes=bytes.byteLength;try{return await WebAssembly.compile(bytes);}finally{new Uint8Array(bytes).fill(0);moduleSourceCleared=true;}})().catch(e=>{compiled=null;throw e;});return compiled;}
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
  const info=()=>({compiler:{schema:'aethyr.pack-compiler-input.v1',sourceBytes:moduleBytes,sourceCleared:moduleSourceCleared,compiledStorage:'opaque SDK program outside pack byte storage'},storage:[0,1,2,3].map(f=>Number(master.x.pack_storage_info(f))),owners:ops.size,loading:[...ops].filter(o=>o.loading).length,published:[...ops].filter(o=>o.published).length,readers:[...ops].reduce((n,o)=>n+o.readers.size,0),sdkPending:[...ops].reduce((n,o)=>n+o.sdkPending,0),copied:[...ops].reduce((n,o)=>n+o.copied,0)});
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
   if(/\.vpk3$/.test(pack.url))return loadPaged(pack,{owner,current,progress,observe});
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
  // A range-addressable VPK3 pack (aethyr/forge/page-pack.coax): the header and manifest in one
  // range, then runs of pages, roots first, a few ranges at once, each retried with back-off. A
  // server that ignores Range sends the file once and the ranges are cut from it. Each page is
  // inflated here and written at its place in the image arena; Coax admits it by size and SHA-256
  // (aethyr/host/page-stream.coax). The roots held, the image is admitted as a whole pack is, and
  // the result is published; pages that are not roots follow, and result.complete waits for them.
  // With image:true the image is published once its roots are held, without the VPK1 reader: for
  // a caller that reads the image itself (host.js's whole-pack path, geometry pages).
  function loadPaged(pack,{owner,current=()=>true,progress,observe,image=false}={}){
   const op=operation(owner,current,progress,undefined,false,observe);let trailing=null;
   op.finished=(async()=>{try{
    const url=pack.url,signal=op.controller.signal,init=pack.quiet?{cache:'no-store'}:{},sleep=ms=>new Promise(r=>setTimeout(r,ms));
    let whole=null,total=0,etag=null;
    const range=async(from,to)=>{
     for(let attempt=0;;attempt++){
      op.check();
      try{
       if(whole)return whole.subarray(from,Math.min(to,whole.length));
       const r=await fetch(url,{...init,signal,headers:{Range:`bytes=${from}-${to-1}`}});op.check();
       const tag=r.headers.get('etag');if(etag===null)etag=tag;else if(tag!==etag){await r.body?.cancel();throw Object.assign(Error('pack changed while loading'),{final:true});}
       if(r.status===206){
        const m=/^bytes (\d+)-(\d+)\/(\d+)$/.exec(r.headers.get('content-range')||''),body=new Uint8Array(await r.arrayBuffer());op.check();
        if(!m||Number(m[1])!==from||body.length!==Number(m[2])-from+1)throw Error('pack range answered with other bytes');total=Number(m[3]);return body;
       }
       if(r.status===200){const body=new Uint8Array(await r.arrayBuffer());op.check();op.reserve(body.length);op.buffers.push(body);whole=body;total=body.length;return whole.subarray(from,Math.min(to,total));}
       await r.body?.cancel();throw Object.assign(Error(`pack range: ${r.status}`),{final:r.status===404||r.status===416});
      }catch(error){if(error?.name==='AbortError'||op.retired||error.final||attempt>=3)throw error;await sleep(250*2**attempt);}
     }
    };
    const x=op.context.x,cap=x.pack_paged_capacity();
    const put=bytes=>new Uint8Array(op.context.memory.buffer,x.pack_paged_ptr(),bytes.length).set(bytes);
    let head=await range(0,cap);
    if(head.length<96)throw Error('not a VPK3 pack');
    const need=96+new DataView(head.buffer,head.byteOffset,head.length).getUint32(16,true)*64;
    if(need>cap)throw Error('VPK3 manifest is larger than the reader holds');
    if(head.length<need)head=await range(0,need);
    put(head.subarray(0,need));
    if(!x.pack_paged_open(need))throw Error('Coax refused the VPK3 manifest');
    const pages=Number(x.pack_page_info(-1,0)),imageBytes=Number(x.pack_page_info(-1,1)),fileBytes=Number(x.pack_page_info(-1,2));
    if(total&&total!==fileBytes)throw Error('VPK3 file is not the size its header says');
    const info=[];for(let k=0;k<pages;k++){const f=[0,1,2,3,4,5].map(i=>Number(x.pack_page_info(k,i)));info.push({at:f[0],stored:f[1],image:f[2],decoded:f[3],codec:f[4],root:f[5]===1});}
    const address=arena(op,imageBytes),held=new Uint8Array(pages);let loaded=need;op.progress?.(loaded,fileBytes);
    // Runs: consecutive pages of one kind (root or not), up to 2 MiB of stored bytes each.
    const runs=root=>{const out=[];let run=null;for(let k=0;k<pages;k++){if(info[k].root!==root){run=null;continue;}
     if(run&&k===run.at(-1)+1&&info[k].at+info[k].stored-info[run[0]].at<=2097152)run.push(k);else out.push(run=[k]);}return out;};
    const inflate=async bytes=>new Uint8Array(await new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('deflate-raw'))).arrayBuffer());
    const fetchRun=async run=>{
     const first=info[run[0]],last=info[run.at(-1)];
     for(let attempt=0;;attempt++){
      const bytes=await range(first.at,last.at+last.stored);let ok=bytes.length===last.at+last.stored-first.at;
      for(const k of run){
       if(!ok)break;if(held[k])continue;const g=info[k],stored=bytes.subarray(g.at-first.at,g.at-first.at+g.stored);
       let decoded;op.sdkPending++;try{decoded=g.codec===1?await inflate(stored):stored;}catch{ok=false;break;}finally{op.sdkPending--;}
       op.check();if(decoded.length!==g.decoded){ok=false;break;}
       new Uint8Array(op.context.memory.buffer,address+g.image,g.decoded).set(decoded);
       if(!x.pack_page_admit(k)){ok=false;break;}held[k]=1;loaded+=g.stored;op.copied+=g.decoded;op.progress?.(loaded,fileBytes);
      }
      if(ok)return;if(attempt>=2)throw Error('Coax refused a VPK3 page');await sleep(250*2**attempt);
     }
    };
    const workers=async list=>{const queue=[...list],width=Math.max(1,Math.min(16,pack.ranges||4));await Promise.all(Array.from({length:Math.min(width,queue.length)},async()=>{while(queue.length)await fetchRun(queue.shift());}));};
    await workers(runs(true));op.check();
    if(!x.pack_paged_finish()||(!image&&!x.pack_admit(0)))throw Error('Coax refused complete pack identity, layout or catalog');
    const memory=op.context.memory,source=new Uint8Array(memory.buffer,address,imageBytes);
    const entries=[];if(!image)for(let i=0;i<x.pack_entry_count();i++){const fields=[0,1,2,3,4,5].map(f=>Number(x.pack_entry(i,f)));entries.push(Object.freeze({kind:fields[0],licence:fields[1],name:new TextDecoder().decode(new Uint8Array(memory.buffer,fields[4],fields[5])),bytes:source.subarray(fields[2],fields[2]+fields[3]),at:fields[2]}));}
    const rest=runs(false);trailing=rest.length?workers(rest):null;const complete=(trailing||Promise.resolve()).then(()=>{op.check();});complete.catch(()=>{});
    op.published=true;const result=Object.freeze({entries:Object.freeze(entries),source,bytes:imageBytes,stored:fileBytes,pages,owner,complete,
     ready:entry=>!!op.context&&!!op.context.x.pack_paged_ready(entry.at,entry.bytes.length),close:()=>op.abort(),
     async use(fn){op.check();op.borrowers++;try{const answer=await fn(result);op.check();return answer;}finally{op.borrowers--;op.dispose();}}});return result;
   }catch(error){op.retired=true;op.controller.abort();throw error;}finally{
    if(trailing){op.loading=true;trailing.catch(()=>{}).finally(()=>{op.loading=false;op.dispose();});}else{op.loading=false;op.dispose();}
   }})();return op.finished;
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
  return Object.freeze({load,loadPaged,loadBytes,retainBytes,info,abortReads(owner){for(const op of ops)if(op.owner===owner&&!op.published)op.abort();},async retire(owner){const selected=[...ops].filter(op=>op.owner===owner);for(const op of selected)op.abort();await Promise.allSettled(selected.map(op=>op.finished));await Promise.all(selected.map(op=>op.closed));},masterBytes:master.memory.buffer.byteLength});
 })().catch(error=>{opened=null;throw error;});return opened;
}
