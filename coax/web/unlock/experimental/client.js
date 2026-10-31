// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Explicit experimental/staging client. No auto-arm, production injection,
// module loading, persistence or reinterpretation of a failed v2 entry.
const capacities={password:128,record:4096,message:6144,relay:2048,recipient:64};
function transport(value,capacity){
 if(value instanceof Uint8Array){if(value.length>capacity)throw Error('ABI');return value.slice().buffer;}
 if(typeof value!=='string')throw Error('ABI');
 const buffer=new Uint8Array(capacity),result=new TextEncoder().encodeInto(value,buffer);
 if(result.read!==value.length){buffer.fill(0);throw Error('ABI');}
 const exact=buffer.slice(0,result.written).buffer;buffer.fill(0);return exact;
}
export function createExperimentalGate(config){
 const snapshot=value=>value instanceof Uint8Array?value.slice():value;
 const record=snapshot(config.record),relay=snapshot(config.relay),recipient=snapshot(config.recipient),legacyAdapter=config.legacyAdapter;
 if(!legacyAdapter||typeof legacyAdapter.match!=='function'||typeof legacyAdapter.normalize!=='function'||legacyAdapter.ROUNDS!==210000)throw Error('legacy adapter');
 const inactivity=config.inactivityMs??30000;if(!Number.isInteger(inactivity)||inactivity<100||inactivity>30000)throw Error('deadline');
 const originalEntries=structuredClone(config.legacy??[]),rawEntries=(config.entries??[]).map(value=>value instanceof Uint8Array?value.slice():value);
 const worker=new Worker(config.worker,{type:'module'}),pending=new Map();let id=0,active,latest=0,chain=Promise.resolve(),closed=false,failed=false;
 let resolveReady,rejectReady;const ready=new Promise((resolve,reject)=>{resolveReady=resolve;rejectReady=reject;});
 ready.catch(()=>{});
 function fail(){failed=true;worker.terminate();rejectReady(Error('worker'));for(const p of pending.values()){clearTimeout(p.timer);p.resolve({type:'refused',error:'worker'});}pending.clear();}
 function waiting(resolve,progress){return{resolve,progress,timer:setTimeout(fail,inactivity)};}
 const timeout=setTimeout(fail,15000);
 function post(message,transfers){try{worker.postMessage(message,transfers);}catch{fail();}}
 worker.onmessage=event=>{
  const m=event.data;if(m.fatal){clearTimeout(timeout);fail();return;}if(m.type==='ready'){clearTimeout(timeout);if(!failed)resolveReady();return;}
  if(m.id===undefined){clearTimeout(timeout);fail();return;}
  const request=pending.get(m.id);if(!request)return;
  if(m.type==='progress'){clearTimeout(request.timer);request.timer=setTimeout(fail,inactivity);request.progress?.(m);return;}
  clearTimeout(request.timer);pending.delete(m.id);request.resolve(m);
 };
 worker.onerror=()=>{clearTimeout(timeout);fail();};
 post({type:'init'});
 async function counts(legacy,v2){await ready;if(failed||closed)return{type:'refused',error:'worker'};const n=++id;const result=new Promise(resolve=>pending.set(n,waiting(resolve)));post({type:'counts',id:n,legacy,v2});return result;}
 async function invoke(operation,password,message,progress){
  await ready;if(active?.cancelled||closed||failed)return{type:'cancelled'};const n=++id,inputs={};
  try{
   for(const[name,value]of Object.entries({password,record,message,relay,recipient}))inputs[name]=transport(value,capacities[name]);
   const result=new Promise(resolve=>pending.set(n,waiting(resolve,progress)));active.workerId=n;
   post({type:'run',id:n,operation,...inputs},Object.values(inputs));return await result;
  }finally{for(const value of Object.values(inputs))if(value.byteLength)new Uint8Array(value).fill(0);}
 }
 function schedule(body){
  const token=++latest;if(active){active.cancelled=true;if(active.workerId)post({type:'cancel',id:active.workerId});}
  const result=chain.catch(()=>{}).then(async()=>{
   if(closed||token!==latest)return null;const current={cancelled:false,workerId:0};active=current;
   try{return await body(current);}finally{if(active===current)active=undefined;}
  });chain=result.then(()=>{},()=>{});return result;
 }
 return{
  ready,
  open(typed,progress){return schedule(async current=>{
   const limits=await counts(originalEntries.length,rawEntries.length);if(limits.type!=='limits'||limits.error!==0||current.cancelled||failed)return null;
   // Frozen exact-terminal semantics, independent of v2's ASCII profile.
   const legacy=legacyAdapter.normalize(typed);if(legacy.length>=4&&legacy.length<=40){
    let pointer=null;try{pointer=await legacyAdapter.match(typed,originalEntries,legacyAdapter.ROUNDS,true);}catch{}
    if(current.cancelled)return null;if(pointer)return pointer;
   }
   for(const raw of rawEntries){
    if(current.cancelled)return null;const result=await invoke(0,typed,raw,progress);current.workerId=0;
    if(current.cancelled)return null;
    if(result.type==='done')return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(result.result));
   }return null;
  });},
  seal(typed,canonicalPointer,progress){return schedule(async current=>{
   const limits=await counts(originalEntries.length,rawEntries.length);if(limits.type!=='limits'||limits.error!==0||current.cancelled||failed)return null;
   const result=await invoke(1,typed,canonicalPointer,progress);current.workerId=0;
   if(current.cancelled||result.type!=='done')return null;return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(result.result));
  });},
  async cancel(){++latest;if(active){active.cancelled=true;if(active.workerId)post({type:'cancel',id:active.workerId});}await chain;},
  async close(){if(closed)return;closed=true;clearTimeout(timeout);rejectReady(Error('closed'));await this.cancel();worker.terminate();},
 };
}
