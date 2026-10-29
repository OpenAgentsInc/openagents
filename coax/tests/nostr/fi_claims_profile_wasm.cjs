const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:96,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const [i,r] of rows.entries()){const t=Buffer.from(r.text,'base64'),m=Buffer.from(r.marker,'base64');u.set(t,base);u.set(m,base+131072);const got=instance.exports.profile(base,t.length,base+131072,m.length,BigInt(r.mode));if(got!==r.want)throw Error(`case ${i}: ${got} != ${r.want}`);}
 console.log(`${rows.length} class-aware claims Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
