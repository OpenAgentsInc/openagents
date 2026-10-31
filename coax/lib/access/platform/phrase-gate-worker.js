// Firm reason: 1 (browser worker glue: scheduling and transport for Coax's wasm in a Web Worker)
// Dedicated-worker scheduling, scalar/byte transport and OS entropy only.
// Coax performs every format, KDF, MAC/decryption, locator and terminal decision.
let engine,active,initializing=false,poisoned=false;
const channel=new MessageChannel();channel.port1.onmessage=advance;
const fields=['password','record','message','relay','recipient'];
function wipeReceived(request){
 const buffers=new Set();for(const name of fields){const v=request?.[name];if(v instanceof ArrayBuffer)buffers.add(v);else if(ArrayBuffer.isView(v)&&v.buffer instanceof ArrayBuffer)buffers.add(v.buffer);}
 for(const buffer of buffers)new Uint8Array(buffer).fill(0);
}
function reject(request,error){wipeReceived(request);postMessage({type:'refused',id:request?.id,error,fatal:poisoned});}
function progress(){return{completed:engine.completed(),total:engine.total(),maxChunkMs:active.maxChunkMs};}
function terminal(error,retire=false){
 const id=active?.id;let result,emitted=false,cancelled=false,cleanup,clean,observation={};
 try{
  observation=active?progress():{};
  if(!retire){emitted=!!engine.emit();cancelled=!!engine.cancelled();error=error||engine.error();
   if(emitted)result=new Uint8Array(engine.memory.buffer,engine.output(),engine.written()).slice().buffer;}
  cleanup=engine.dispose();if(cleanup)retire=true;
  if(cleanup===0)clean=engine.clean?.();
 }catch{retire=true;error='trap';try{cleanup=engine?.dispose();}catch{}clean=undefined;}
 if(retire){if(result)new Uint8Array(result).fill(0);result=undefined;emitted=false;cancelled=false;error=error||cleanup||'cleanup';engine=undefined;poisoned=true;}
 active=null;
 if(emitted)postMessage({type:'done',id,result,...observation,cleanup,clean},[result]);
 else postMessage({type:cancelled?'cancelled':'refused',id,error,...observation,cleanup,clean,fatal:poisoned});
}
function trap(id){
 if(!active)active={id,maxChunkMs:0};
 try{engine?.abort();}catch{}terminal('trap',true);
}
function advance(){
 if(!active)return;
 try{
  if(active.cancelled){engine.cancel();terminal();return;}
  const started=performance.now(),status=engine.advance(active.budget);active.maxChunkMs=Math.max(active.maxChunkMs,performance.now()-started);
  if(status||!engine.running()){terminal(status);return;}
  if(engine.completed()-active.reported>=4096){active.reported=engine.completed();postMessage({type:'progress',id:active.id,...progress()});}
  channel.port2.postMessage(0);
 }catch{trap(active?.id);}
}
onmessage=event=>{receive(event.data).catch(()=>{wipeReceived(event.data);trap(event.data?.id);});};
async function receive(request){
 if(!request||typeof request!=='object'){reject(request,'ABI');return;}
 if(request.type!=='run')wipeReceived(request);
 if(poisoned){reject(request,'retired');return;}
 if(request.type==='init'){
  if(engine||active||initializing){reject(request,'busy');return;}initializing=true;
  const denied=()=>{throw Error('Unexpected gate host I/O');};
  try{
   const response=fetch(new URL('./phrase-gate-worker.wasm',import.meta.url));
   const result=await WebAssembly.instantiateStreaming(response,{wasi_snapshot_preview1:{fd_fdstat_get:denied,fd_seek:denied,fd_write:denied,proc_exit:denied}});
   engine=result.instance.exports;engine._initialize?.();
   const cleanup=engine.dispose();if(cleanup){engine=undefined;poisoned=true;postMessage({type:'refused',error:'cleanup',cleanup,fatal:true});return;}
   postMessage({type:'ready'});
  }catch{engine=undefined;poisoned=true;postMessage({type:'refused',error:'module',fatal:true});}finally{initializing=false;}return;
 }
 if(request.type==='cancel'){if(active&&request.id===active.id)active.cancelled=true;return;}
 if(request.type==='counts'&&engine){
  if(![request.legacy,request.v2].every(x=>Number.isInteger(x)&&x>=-2147483648&&x<=2147483647)){reject(request,'ABI');return;}
  postMessage({type:'limits',id:request.id,error:engine.attempts(request.legacy,request.v2)});return;
 }
 // These observations/faults exist only in separately compiled fixture modules.
 if(request.type==='test-fault'&&engine?.test_fault){engine.test_fault(request.value);return;}
 if(request.type==='test-trap'&&engine?.test_trap){try{engine.test_trap();}catch{trap(active?.id);}return;}
 if(request.type!=='run'||!engine||active){reject(request,'busy');return;}
 if(!fields.every(name=>request[name] instanceof ArrayBuffer)){reject(request,'ABI');return;}
 const inputs=Object.fromEntries(fields.map(name=>[name,new Uint8Array(request[name])]));
 const budget=request.budget??engine.budget();
 if(![request.id,request.operation,budget,...Object.values(inputs).map(v=>v.length)].every(x=>Number.isInteger(x)&&x>=-2147483648&&x<=2147483647)||request.id<1){reject(request,'ABI');return;}
 const lengths=fields.map(name=>inputs[name].length),status=engine.preflight(...lengths,request.operation);
 if(status){reject(request,status);return;}
 const cleanup=engine.dispose();if(cleanup){engine=undefined;poisoned=true;reject(request,cleanup);return;}
 active={id:request.id,budget,reported:0,maxChunkMs:0,cancelled:false};
 try{
  for(const name of fields)new Uint8Array(engine.memory.buffer,engine[name](),inputs[name].length).set(inputs[name]);
  const random=engine.random_count(request.operation);
  if(random){const target=new Uint8Array(engine.memory.buffer,engine.entropy(),random);if(crypto.getRandomValues(target)!==target)throw Error('Entropy effect refused');}
  const error=engine.begin(...lengths,request.operation);if(error){terminal(error);return;}
  postMessage({type:'progress',id:active.id,...progress()});channel.port2.postMessage(0);
 }catch{trap(request.id);}finally{wipeReceived(request);}
}
