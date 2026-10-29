const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024});const wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,out=base+65536,u=new Uint8Array(memory.buffer),cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const v of cases){const data=Buffer.from(v.text,'base64');u.set(data,base);let typ=instance.exports.parse(base,data.length,out);if(typ!==(v.typ||0))throw Error(`typ ${typ} != ${v.typ}`);if(typ && Buffer.from(u.slice(out,out+u[out+128])).toString('hex')!==v.kid)throw Error('kid mismatch');}
 console.log(`${cases.length} protected header Wasm cases match independent Python oracle`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
