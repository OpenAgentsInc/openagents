// Firm reason: 1 (browser SDK observations of actual Wasm carrier exports and exact physical source spans; Coax owns all admission).
export async function transportFixture({sceneSHA,transportSHA}){
  const memory=new WebAssembly.Memory({initial:32,maximum:128}),module=await WebAssembly.compile(await(await fetch('/build/stream-walk.wasm')).arrayBuffer());
  const wasi={proc_exit(code){throw Error(`reactor exit ${code}`);},fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,fd_write:()=>8};
  const x=(await WebAssembly.instantiate(module,{env:{memory},wasi_snapshot_preview1:wasi})).exports;x._initialize?.();x.sw_loss_open();
  const bytes=async url=>new Uint8Array(await(await fetch(url)).arrayBuffer());
  function config(data,root){new Uint8Array(memory.buffer,x.sw_config_ptr(),data.length).set(data);new Uint8Array(memory.buffer,x.sw_root_ptr(),64).set(new TextEncoder().encode(root));}
  const sceneBytes=await bytes('/scene/scene.json'),scene=JSON.parse(new TextDecoder().decode(sceneBytes)),m=scene.manifests.find(m=>m.backend===0&&m.tier===0),manifest=await bytes(`/scene/${m.sha256}.rcm`),htp=await bytes(`/transport/${transportSHA}.htp`);
  const facts={};config(sceneBytes,'0'.repeat(64));facts.badScene=x.sw_transport_scene(sceneBytes.length)===0&&x.sw_transport_info(0)===0n;
  config(sceneBytes,sceneSHA);facts.scene=x.sw_transport_scene(sceneBytes.length)===1;facts.sceneOnce=x.sw_transport_scene(sceneBytes.length)===0;
  config(manifest,m.sha256);facts.manifest=x.sw_host_start(manifest.length,0,0)===1;
  config(htp,'0'.repeat(64));facts.badCompanion=x.sw_transport_open(htp.length)===0&&x.sw_transport_info(0)===1n;
  config(htp,transportSHA);facts.companion=x.sw_transport_open(htp.length)===1;facts.companionOnce=x.sw_transport_open(htp.length)===0;
  x.sw_host_frame(15625000n,0);const keys=Array.from({length:Number(x.sw_host_info(1))},(_,i)=>({owner:Number(x.sw_host_command(0,i,0)),generation:x.sw_host_command(0,i,1),epoch:x.sw_host_command(0,i,2)}));
  if(!keys.length)throw Error('fixture has no actual resource commands');
  const k=keys[0],args=[k.owner,k.generation,k.epoch],r=scene.resources[k.owner];
  function begin(key,offset=scene.resources[key.owner].offset){const r=scene.resources[key.owner];new Uint8Array(memory.buffer,x.sw_transport_input(),128).set(new TextEncoder().encode(r.container_sha256+r.sha256));return x.sw_transport_begin(key.owner,key.generation,key.epoch,BigInt(offset),BigInt(r.bytes),BigInt(Number(r.raw)));}
  facts.unboundAllocation=x.sw_host_allocate(...args)===0&&x.sw_host_info(14)===0n;
  facts.wrongLocation=begin(k,r.offset+1)===0;
  facts.begin=begin(k)===1;facts.beginOnce=begin(k)===0;facts.allocate=x.sw_host_allocate(...args)===1;
  facts.earlyDecode=x.sw_host_decode(...args)!==0;
  const retained=x.sw_host_info(14);
  facts.earlyFinish=x.sw_host_finish(...args,1)===0&&x.sw_host_info(14)===retained;
  facts.earlyDestroy=x.sw_host_destroy(...args)===0&&x.sw_host_info(14)===retained;
  facts.earlyRecovery=x.sw_host_recover()===0&&x.sw_host_info(14)===retained;
  facts.earlyClose=x.sw_transport_close()===0;
  const whole=await bytes(`/scene/${r.container_sha256}${r.raw?'.raw':'.vpk'}`),bad=whole.slice();bad[bad.length-1]^=1;config(bad,r.container_sha256);
  facts.badSource=x.sw_transport_source(...args,bad.length)===0&&x.sw_transport_row(k.owner,5)===0n;
  config(whole,r.container_sha256);facts.staleSource=x.sw_transport_source(k.owner,k.generation+1n,k.epoch,whole.length)===0;
  facts.source=x.sw_transport_source(...args,whole.length)===1;
  facts.sourceOnce=x.sw_transport_source(...args,whole.length)===0;
  new Uint8Array(memory.buffer,x.sw_host_pointer(k.owner,0),r.bytes).set(whole.subarray(r.offset,r.offset+r.bytes));facts.decode=x.sw_host_decode(...args)===0;
  facts.unjoined=x.sw_transport_end(...args,0,1)===0&&x.sw_transport_row(k.owner,4)===1n;
  facts.staleEnd=x.sw_transport_end(k.owner,k.generation+1n,k.epoch,1,1)===0&&x.sw_transport_row(k.owner,4)===1n;
  facts.end=x.sw_transport_end(...args,1,1)===1;facts.endOnce=x.sw_transport_end(...args,1,1)===0;
  facts.finish=x.sw_host_finish(...args,0)===1;facts.finishOnce=x.sw_host_finish(...args,0)===0;
  facts.otherAttempts=keys.slice(1).every(key=>begin(key)===1&&x.sw_host_allocate(key.owner,key.generation,key.epoch)===1&&x.sw_transport_end(key.owner,key.generation,key.epoch,1,0)===1&&x.sw_host_finish(key.owner,key.generation,key.epoch,1)===1);
  x.sw_host_ack();x.sw_host_close();
  for(let frame=0;frame<32&&!x.sw_host_info(24);frame++){x.sw_host_frame(15625000n,0);for(let i=0;i<x.sw_host_info(2);i++)if(!x.sw_host_destroy(Number(x.sw_host_command(1,i,0)),x.sw_host_command(1,i,1),x.sw_host_command(1,i,2)))throw Error('retirement refused');x.sw_host_ack();}
  facts.drained=x.sw_host_info(24)===1n&&[10,11,12,13,14,22,23].every(f=>x.sw_host_info(f)===0n);
  facts.closed=x.sw_transport_close()===1&&x.sw_transport_info(0)===3n;
  facts.closedOnce=x.sw_transport_close()===0;
  new Uint8Array(memory.buffer).fill(0);return facts;
}
