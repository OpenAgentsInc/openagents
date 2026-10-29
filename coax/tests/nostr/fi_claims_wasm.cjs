const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:96,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const [i,row] of rows.entries()){
  const t=Buffer.from(row.text,'base64'),m=Buffer.from(row.marker,'base64');u.set(t,base);u.set(m,base+131072);
  const n=instance.exports.project(base,t.length,base+131072,m.length,base+196608,40000);
  if(row.expect===null){if(n!==-1)throw Error(`case ${i} expected refusal got ${n}`);}
  else {const want=Buffer.from(row.expect,'base64'),got=Buffer.from(u.slice(base+196608,base+196608+n));if(n!==want.length||!got.equals(want))throw Error(`case ${i} projection mismatch`);}
 }
 console.log(`${rows.length} independent claims Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
