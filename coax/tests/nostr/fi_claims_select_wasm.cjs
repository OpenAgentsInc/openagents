const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const [i,row] of rows.entries()){
  const a=[row.text,row.host,row.allow,row.marker].map(x=>Buffer.from(x,'base64'));let offsets=[0,131072,132096,135168];
  a.forEach((x,j)=>u.set(x,base+offsets[j]));const out=base+136192;u.fill(0,out,out+129);
  const got=instance.exports.select_claims(base,a[0].length,base+offsets[1],a[1].length,base+offsets[2],a[2].length,base+offsets[3],a[3].length,out);
  let end=out;while(u[end])end++;const marker=Buffer.from(u.slice(out,end)).toString();
  if(got!==row.want || marker!==row.output)throw Error(`case ${i}: ${got}, ${marker}`);
 }
 console.log(`${rows.length} untrusted issuer/marker selection Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
