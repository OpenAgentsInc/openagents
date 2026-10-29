const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const wasi=new WASI({version:'preview1',args:[],env:{}}),{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,data=fs.readFileSync(process.argv[3]);if(data.length%97)throw Error('fixture length');
 const blocks=[x.malloc(48),x.malloc(48),x.malloc(48)];if(blocks.some(p=>!p))throw Error('allocation');const places=blocks.map(p=>p+8);
 for(let i=0;i<data.length/97;i++){
  const at=i*97,values=[data.subarray(at,at+32),data.subarray(at+32,at+64)],u=new Uint8Array(x.memory.buffer);
  blocks.forEach(p=>u.fill(0xa5,p,p+48));values.forEach((bytes,j)=>u.set(bytes,places[j]));
  const handle=x.coax_ecdh_owned_acquire();if(!handle)throw Error(`acquire ${i}`);
  let status=x.coax_ecdh_owned_start(handle,places[0],32n,places[1],32n);
  if(status===5)status=x.coax_ecdh_owned_collect(handle,places[2],32n);
  if(status!==data[at+64])throw Error(`status ${i}: ${status}`);
  const after=new Uint8Array(x.memory.buffer),actual=Buffer.from(after.slice(places[2],places[2]+32));
  if(!status&&!actual.equals(data.subarray(at+65,at+97)))throw Error(`output ${i}`);
  if(status&&actual.some(v=>v!==0xa5))throw Error(`error output ${i}`);
  values.forEach((v,j)=>{if(!Buffer.from(after.slice(places[j],places[j]+32)).equals(v))throw Error(`input ${i}`);});
  blocks.forEach(p=>{if(after.slice(p,p+8).some(v=>v!==0xa5)||after.slice(p+40,p+48).some(v=>v!==0xa5))throw Error(`guard ${i}`);});
  if(x.coax_ecdh_owned_close(handle)!==0)throw Error(`close ${i}`);
 }
 blocks.forEach(p=>x.free(p));process.stdout.write(JSON.stringify({engine:`Node ${process.version} V8 ${process.versions.v8}`,cases:data.length/97,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
