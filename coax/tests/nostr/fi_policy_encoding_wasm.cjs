const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:96,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const r of rows){const n=instance.exports.encode(r.id,base,65536);if(r.expect===null){if(n!==-1)throw Error(`case ${r.id}`);}else{const want=Buffer.from(r.expect,'base64'),got=Buffer.from(u.slice(base,base+n));if(n!==want.length||!got.equals(want))throw Error(`case ${r.id}`);}}
 console.log(`${rows.length} canonical config encoding Wasm cases passed`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
