// Firm reason: 1 (browser glue: leased immutable copies, WebSocket effects and actual queue observations).
// The Coax companion owns all counts/bytes/refusals. Browser/SDK/OS buffering
// is separately observable and is not measured by the application lease ledger.
const wasmURL=new URL('../build/host-sockets.wasm',import.meta.url);
let compiled;
export async function openSocketRetention(capacity){
 compiled??=(async()=>{const r=await fetch(wasmURL,{cache:'no-store'});if(!r.ok)throw Error(`socket queue reactor:${r.status}`);return WebAssembly.compile(await r.arrayBuffer());})().catch(e=>{compiled=null;throw e;});
 const memory=new WebAssembly.Memory({initial:128,maximum:1024});
 const wasi={proc_exit:code=>{throw Error(`socket queue reactor exited:${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
  clock_time_get(id,precision,at){new DataView(memory.buffer).setBigUint64(at,BigInt(Math.floor((id===0?Date.now():performance.now())*1e6)),true);return 0;},
  fd_write(fd,iov,count,written){const d=new DataView(memory.buffer);let n=0;for(let i=0;i<count;i++)n+=d.getUint32(iov+8*i+4,true);d.setUint32(written,n,true);return 0;}};
 const x=new WebAssembly.Instance(await compiled,{env:{memory},wasi_snapshot_preview1:wasi}).exports;x._initialize?.();
 if(!x.socket_queue_open(BigInt(capacity)))throw Error('Coax refused socket queue storage');
 return Object.freeze({
  offer(output,bytes){const accepted=!!x.socket_queue_offer(Number(output),BigInt(bytes)),fields=[0,1,2,3,4].map(f=>x.socket_queue_grant(f));return Object.freeze({accepted,lease:fields.slice(0,3),error:Number(fields[3]),close:Number(fields[4])});},
  release:(output,lease)=>!!x.socket_queue_release(Number(output),...lease),
  info:output=>[0,1,2,3,4].map(f=>Number(x.socket_queue_info(Number(output),BigInt(f)))),
  span:(address,capacity,bytes,limit)=>!!x.socket_queue_span(BigInt(address),BigInt(capacity),BigInt(bytes),BigInt(limit)),
  workers:Number(x.socket_queue_plan(0)),events:Number(x.socket_queue_plan(1)),
  failure:()=>({error:Number(x.socket_queue_failure(0)),close:Number(x.socket_queue_failure(1))}),
  continue:(n,start,now)=>!!x.socket_queue_continue(BigInt(n),BigInt(start),BigInt(now)),reset:()=>!!x.socket_queue_reset()});
}

export function makeSocketQueues(retention,{current,deliver,closed,schedule,canDeliver=()=>true,canSend=()=>true}){
 const encoder=new TextEncoder(),decoder=new TextDecoder(),inputScratch=new Uint8Array(retention.info(false)[2]),outputScratch=new Uint8Array(retention.info(true)[2]);
 const input=[],output=[],terminals=new Array(retention.workers).fill(null),failed=Symbol('socket-queue-failed');
 let retired=false,timer=0,overflow=0,allocations=0,delivering=0;
 const release=(out,row)=>{row.bytes=null;retention.release(out,row.lease);};
 function discardOutput(ws,all){for(let i=output.length-1;i>=0;i--){const row=output[i];if(row.ws===ws&&(all||!row.sent)){output.splice(i,1);release(true,row);}}}
 function terminal(ws,id,r){
  if(retired||ws[failed])return;ws[failed]=true;overflow++;
  discardOutput(ws,false);terminals[id]={ws,id,error:r.error,close:r.close,stage:0};
  try{ws.close();}catch{}schedule();pump();
 }
 function copy(out,bytes){const r=retention.offer(out,bytes.length);if(!r.accepted)return {refused:r};
  try{allocations++;return {bytes:bytes.slice(),lease:r.lease};}catch{retention.release(out,r.lease);return {refused:retention.failure()};}}
 function receive(ws,id,event,text=''){
  if(retired||ws[failed])return false;
  let encoded;try{encoded=encoder.encodeInto(text,inputScratch);}catch{terminal(ws,id,retention.failure());return false;}
  if(encoded.read!==text.length){const refused=retention.offer(false,inputScratch.length+1);if(refused.close){terminal(ws,id,refused);return false;}event=refused.error;encoded={written:0};}
  const row=copy(false,inputScratch.subarray(0,encoded.written));
  if(row.refused){terminal(ws,id,row.refused);return false;}
  Object.assign(row,{ws,id,event});input.push(row);schedule();return true;
 }
 function send(ws,id,memory,address,bytes,trace){
  if(retired||ws[failed])return false;
  if(!retention.span(address,bytes,bytes,memory.buffer.byteLength)){terminal(ws,id,retention.failure());return false;}
  const grant=retention.offer(true,bytes);if(!grant.accepted){terminal(ws,id,grant);return false;}
  let row;
  try{allocations++;row={ws,id,bytes:bytes?new Uint8Array(memory.buffer,address,bytes).slice():new Uint8Array(),lease:grant.lease,sent:false};}
  catch{retention.release(true,grant.lease);terminal(ws,id,retention.failure());return false;}
  try{
   const sample=trace?.(decoder.decode(row.bytes));
   if(sample){const encoded=encoder.encodeInto(sample,outputScratch);if(encoded.read!==sample.length){release(true,row);terminal(ws,id,retention.failure());return false;}
    const stamp=copy(true,outputScratch.subarray(0,encoded.written));if(stamp.refused){release(true,row);terminal(ws,id,stamp.refused);return false;}Object.assign(stamp,{ws,id,sent:false});output.push(stamp);}
  }catch{release(true,row);terminal(ws,id,retention.failure());return false;}
  output.push(row);pump();return true;
 }
 function pump(){
  if(retired||timer)return;
  timer=setTimeout(()=>{timer=0;if(retired)return;
   for(let i=0;i<output.length;){const row=output[i];if(row.ws.readyState===3||row.sent&&row.ws.bufferedAmount===0||!row.sent&&(!current(row.ws,row.id)||row.ws[failed])){output.splice(i,1);release(true,row);continue;}
    if(!row.sent&&canSend()&&row.ws.readyState===1){try{row.ws.send(decoder.decode(row.bytes));row.sent=true;}catch{terminal(row.ws,row.id,retention.failure());}}
    i++;
   }
   if(output.length)pump();
  },4);
 }
 function drain(){
  const start=Math.floor(performance.now());let count=0;
  while(!retired&&input.length&&canDeliver()&&retention.continue(count,start,Math.floor(performance.now()))){
   const row=input.shift();delivering++;try{if(current(row.ws,row.id)){if(row.event===2){closed(row.ws,row.id);discardOutput(row.ws,true);}deliver(row.ws,row.id,row.event,row.bytes);}}finally{release(false,row);delivering--;if(retired&&!delivering)retention.reset();}count++;
  }
  if(!input.length)for(let id=0;id<terminals.length&&canDeliver();id++){const row=terminals[id];if(!row)continue;
   if(current(row.ws,id)){if(row.stage===0&&retention.continue(count,start,Math.floor(performance.now()))){row.stage=1;if(row.error){deliver(row.ws,id,row.error,new Uint8Array());count++;}}
    if(!retired&&canDeliver()&&row.stage===1&&current(row.ws,id)&&retention.continue(count,start,Math.floor(performance.now()))){row.stage=2;if(row.close){closed(row.ws,id);deliver(row.ws,id,row.close,new Uint8Array());count++;}}}
   if(!current(row.ws,id)||row.stage===2){discardOutput(row.ws,true);if(terminals[id]===row)terminals[id]=null;}
  }
 }
 return Object.freeze({receive,send,drain,hasInput:()=>input.length>0||terminals.some(Boolean),
  info:()=>({input:retention.info(false),output:retention.info(true),events:input.length,commands:output.length,delivering,terminals:terminals.filter(Boolean).length,overflow,allocations,retired,buffered:output.map(r=>({id:r.id,sent:r.sent,bytes:r.bytes.length,bufferedAmount:r.ws.bufferedAmount,readyState:r.ws.readyState}))}),
  reset(){if(retired)return;retired=true;if(timer)clearTimeout(timer);timer=0;for(const row of input)release(false,row);input.length=0;for(const row of output)release(true,row);output.length=0;terminals.fill(null);if(!delivering)retention.reset();}});
}
