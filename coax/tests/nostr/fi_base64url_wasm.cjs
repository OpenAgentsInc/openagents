const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024});const wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,out=base+65536,u=new Uint8Array(memory.buffer),cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const v of cases){const data=Buffer.from(v.text,'base64');u.set(data,base);u.fill(165,out,out+512);let n=instance.exports.decode(base,data.length,out,512);if(n!==(v.expected===null?-1:v.expected.length/2))throw Error(`bad length ${n}`);if(n>=0 && Buffer.from(u.slice(out,out+n)).toString('hex')!==v.expected)throw Error('bytes mismatch');if(u.slice(out+(n<0?0:n),out+512).some(x=>x!==165))throw Error('guard changed');}
 console.log(`${cases.length} base64url Wasm cases match independent Python oracle`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
