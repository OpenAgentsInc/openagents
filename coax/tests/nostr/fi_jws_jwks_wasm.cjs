const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer),cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of cases){const t=Buffer.from(row.token,'base64url'),j=Buffer.from(row.jwks,'base64url');u.set(t,base);u.set(j,base+32768);const got=instance.exports.verify(base,t.length,base+32768,j.length);if(got!==row.want)throw Error(`got ${got}, wanted ${row.want}`);}
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
