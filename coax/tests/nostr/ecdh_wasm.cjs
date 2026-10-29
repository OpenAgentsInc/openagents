const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,peer=base+32,work=peer+40,u=new Uint8Array(memory.buffer),data=fs.readFileSync(process.argv[3]);
 if(data.length%97)throw Error('fixture length');
 for(let i=0;i<data.length/97;i++){
  const at=i*97;u.set(data.subarray(at,at+32),base);u.set(data.subarray(at+32,at+64),peer);u.fill(0xa5,work-8,work+480);
  const got=x.ecdh_test(base,peer,work);if(got!==data[at+64])throw Error(`status ${i}: ${got}`);
  if(got!==2&&!Buffer.from(u.slice(work+440,work+472)).equals(data.subarray(at+65,at+97)))throw Error(`output ${i}`);
  if(u.slice(work-8,work).some(v=>v!==0xa5)||u.slice(work+472,work+480).some(v=>v!==0xa5))throw Error(`guard ${i}`);
 }
 process.stdout.write(JSON.stringify({engine:`Node ${process.version} V8 ${process.versions.v8}`,cases:data.length/97,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
