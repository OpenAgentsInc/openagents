// Firm reason: 1 (browser worker glue: scheduling and transport for Coax's wasm in a Web Worker)
// Dedicated-worker lifecycle and scheduling only. Coax owns KDF parameters,
// work budgets, counters, cancellation and output admission. Not a gate cutover.
let engine,job,initializing=false;
const channel=new MessageChannel();
channel.port1.onmessage=advance;
const publicProgress=()=>({completed:engine.completed(),total:engine.total(),maxChunkMs:job.maxChunkMs});
function wipeReceived(request){
 const buffers=new Set();
 for(const value of [request?.password,request?.salt]){
  if(value instanceof ArrayBuffer)buffers.add(value);
  else if(ArrayBuffer.isView(value)&&value.buffer instanceof ArrayBuffer)buffers.add(value.buffer);
 }
 for(const buffer of buffers)new Uint8Array(buffer).fill(0);
}
function reject(request,message){wipeReceived(request);postMessage(message);}
function dispose(){try{return engine.dispose();}catch{return 6;}}
function fail(error){const id=job?.id,cleanup=dispose();job=null;postMessage({type:'error',id,error,cleanup,clean:cleanup?undefined:engine.clean?.()});}
function advance(){try{advanceJob();}catch{fail('trap');}}
function advanceJob(){
 if(!job)return;
 if(job.cancelled){const id=job.id,progress=publicProgress(),cleanup=engine.cancel();job=null;postMessage({type:'cancelled',id,...progress,cleanup,clean:engine.clean?.()});return;}
 const start=performance.now(),error=engine.advance(job.budget);job.maxChunkMs=Math.max(job.maxChunkMs,performance.now()-start);
 if(error){fail(error);return;}
 if(engine.phase()===2){
  const error=engine.finish();if(error){fail(error);return;}
  const key=new Uint8Array(engine.memory.buffer,engine.output(),job.length).slice().buffer;
  const id=job.id,progress=publicProgress(),cleanup=dispose();job=null;
  if(cleanup){new Uint8Array(key).fill(0);postMessage({type:'error',id,error:cleanup,cleanup});return;}
  postMessage({type:'done',id,key,...progress,cleanup,clean:engine.clean?.()},[key]);return;
 }
 if(engine.completed()-job.reported>=4096){job.reported=engine.completed();postMessage({type:'progress',id:job.id,...publicProgress()});}
 channel.port2.postMessage(0);
}
onmessage=event=>{receive(event).catch(()=>{
 wipeReceived(event.data);
 if(job)fail('trap');else postMessage({type:'error',id:event.data?.id,error:'trap',cleanup:engine?dispose():undefined});
});};
async function receive(event){
 const request=event.data;
 if(!request||typeof request!=='object'){reject(request,{type:'error',error:'ABI'});return;}
 if(request.type!=='derive')wipeReceived(request);
 try{
 if(request.type==='init'){
  if(engine||job||initializing){reject(request,{type:'error',id:request.id,error:'busy'});return;}
  initializing=true;
  const denied=()=>{throw new Error('KDF host effect refused');};
  try{const{instance}=await WebAssembly.instantiateStreaming(fetch(request.url),{wasi_snapshot_preview1:{fd_fdstat_get:denied,fd_seek:denied,fd_write:denied,proc_exit:denied}});engine=instance.exports;engine._initialize?.();postMessage({type:'ready'});}
  catch{engine=undefined;reject(request,{type:'error',error:'module'});}finally{initializing=false;}return;
 }
 if(request.type==='cancel'){if(job&&request.id===job.id)job.cancelled=true;return;}
 if(request.type!=='derive'||!engine||job){reject(request,{type:'error',id:request.id,error:'busy'});return;}
 // Transfer ownership of input ArrayBuffers; no plaintext string conversion.
 if(!(request.password instanceof ArrayBuffer)||!(request.salt instanceof ArrayBuffer)){reject(request,{type:'error',id:request.id,error:'ABI'});return;}
 const password=new Uint8Array(request.password),salt=new Uint8Array(request.salt),budget=request.budget??engine.budget();
 // Exact scalar transport precedes Wasm i32 coercion; KDF bounds stay Coax.
 if(![password.length,salt.length,request.rounds,request.length,budget].every(x=>Number.isInteger(x)&&x>=-2147483648&&x<=2147483647)||(request.gate!==undefined&&typeof request.gate!=='boolean')){
  reject(request,{type:'error',id:request.id,error:'ABI'});return;}
 const status=engine.preflight(password.length,salt.length,request.rounds,request.length,budget,request.gate?1:0);
 if(status){const cleanup=dispose();reject(request,{type:'error',id:request.id,error:status,cleanup,clean:engine.clean?.()});return;}
 if(dispose()){reject(request,{type:'error',id:request.id,error:'cleanup'});return;}
 new Uint8Array(engine.memory.buffer,engine.password(),password.length).set(password);
 new Uint8Array(engine.memory.buffer,engine.salt(),salt.length).set(salt);
 const pn=password.length,sn=salt.length;password.fill(0);salt.fill(0);
 job={id:request.id,length:request.length,budget,reported:0,maxChunkMs:0,cancelled:false};
 try{const error=engine.begin(pn,sn,request.rounds,request.length,request.gate?1:0);if(error){fail(error);return;}channel.port2.postMessage(0);}
 catch{fail('trap');}
 }finally{wipeReceived(request);}
}
