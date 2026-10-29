const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const [i,r] of rows.entries()){
  const t=Buffer.from(r.token,'base64'),j=Buffer.from(r.jwks,'base64'),h=Buffer.from(r.host,'base64');u.set(t,base);u.set(j,base+131072);u.set(h,base+262144);
  const got=instance.exports.local_class(base,t.length,base+131072,j.length,base+262144,h.length,BigInt(r.now),BigInt(r.klass));if(got!==r.want)throw Error(`case ${i}: ${got} != ${r.want}`);
 }
 console.log(`${rows.length} local JWS candidate Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
