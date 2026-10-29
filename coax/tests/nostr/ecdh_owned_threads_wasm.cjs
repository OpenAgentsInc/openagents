const fs=require('node:fs'),{Worker,isMainThread,workerData,parentPort}=require('node:worker_threads');
async function instance(module,memory){const wasi={};for(const item of WebAssembly.Module.imports(module))if(item.module==='wasi_snapshot_preview1')wasi[item.name]=()=>{throw Error('unexpected WASI effect: '+item.name);};return WebAssembly.instantiate(module,{env:{memory},wasi_snapshot_preview1:wasi});}
if(!isMainThread){
 (async()=>{const x=(await instance(workerData.module,workerData.memory)).exports;x.__stack_pointer.value=8*1024*1024;parentPort.postMessage('ready');parentPort.on('message',()=>{try{const status=x.coax_ecdh_owned_start(workerData.handle,workerData.key,32n,workerData.peer,32n);parentPort.postMessage({status});}catch(e){parentPort.postMessage({error:String(e.stack||e)});}});})().catch(e=>parentPort.postMessage({error:String(e.stack||e)}));
}else{
 (async()=>{
  const module=new WebAssembly.Module(fs.readFileSync(process.argv[2])),memory=new WebAssembly.Memory({initial:256,maximum:256,shared:true});const x=(await instance(module,memory)).exports;
  x.__stack_pointer.value=10*1024*1024;x._initialize();const heap=Number(x.__heap_base.value);if(heap>7*1024*1024-128)throw Error('stack boundary overlaps static memory');
  const data=new Uint8Array(memory.buffer),key=12*1024*1024,peer=key+64,out=peer+64,bounds=[7*1024*1024,9*1024*1024];bounds.forEach(n=>data.fill(0x6d,n-128,n));data.fill(0,key,key+32);data[key+31]=1;data.set(Buffer.from('79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798','hex'),peer);data.fill(0xa5,out,out+32);
  const h=x.coax_ecdh_owned_acquire();if(!h)throw Error('acquire');const g=x.gate_address();const gate=new Int32Array(memory.buffer,g,3);Atomics.store(gate,0,1);
  const worker=new Worker(__filename,{workerData:{module,memory,handle:h,key,peer}});const message=()=>new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('worker message timeout')),10000);worker.once('message',value=>{clearTimeout(timer);resolve(value);});worker.once('error',error=>{clearTimeout(timer);reject(error);});});
  try{
  const ready=await message();if(ready!=='ready')throw Error('worker did not initialize: '+JSON.stringify(ready));worker.postMessage('go');const until=Date.now()+10000;while(!Atomics.load(gate,1)){if(Date.now()>until)throw Error('wipe effect was not entered');Atomics.wait(gate,1,0,10);}
  if(x.coax_ecdh_owned_acquire()!==0n||x.coax_ecdh_owned_cancel(h)!==8||x.coax_ecdh_owned_close(h)!==8||x.coax_ecdh_owned_retry(h)!==8||x.coax_ecdh_owned_collect(h,out,32n)!==8)throw Error('concurrent operation did not report busy');
  if(data.slice(out,out+32).some(v=>v!==0xa5))throw Error('busy collect changed output');const result=message();Atomics.store(gate,2,1);Atomics.notify(gate,2);const answer=await result;if(answer.status!==5)throw Error('worker start result '+JSON.stringify(answer));
  if(x.coax_ecdh_owned_cancel(h)!==6||x.coax_ecdh_owned_close(h)!==0)throw Error('post-contention cleanup');if(bounds.some(n=>data.slice(n-128,n).some(v=>v!==0x6d)))throw Error('stack boundary canary changed');console.log(JSON.stringify({engine:`Node ${process.version} V8 ${process.versions.v8}`,shared:true,busy_calls:5,output_unchanged:true,cancelled_and_closed:true,stack_boundary_canaries_unchanged:true}));
  }finally{Atomics.store(gate,2,1);Atomics.notify(gate,2);await worker.terminate();}
 })().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
}
