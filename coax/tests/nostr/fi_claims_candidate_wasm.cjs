const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const [i,r] of rows.entries()){
  let a=[r.text,r.host,r.marker,r.algorithm,r.typ].map(x=>Buffer.from(x,'base64'));let p=[0,131072,132096,133120,134144];a.forEach((x,j)=>u.set(x,base+p[j]));
  const got=instance.exports.candidate(base,a[0].length,base+p[1],a[1].length,base+p[2],a[2].length,base+p[3],a[3].length,base+p[4],a[4].length,BigInt(r.now));
  if(got!==r.want)throw Error(`case ${i}: ${got} != ${r.want}`);
 }
 console.log(`${rows.length} semantic candidate Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
