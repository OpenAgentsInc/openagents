const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024});
 const wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});
 wasi.initialize(instance);
 const base=Number(instance.exports.__heap_base.value)+1024,out=base+65536;
 const u=new Uint8Array(memory.buffer),view=new DataView(memory.buffer),cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const v of cases){
  const input=Buffer.from(v.text,'base64');u.set(input,base);const count=instance.exports.parse(base,input.length,out);
  if(count!==(v.expected?.length||0))throw Error(`snapshot count ${count}`);
  for(let i=0;i<count;i++){
   const p=out+i*649,kn=view.getUint32(p+128,true),mn=view.getUint32(p+644,true);
   if(Buffer.from(u.slice(p,p+kn)).toString('hex')!==v.expected[i][0] || Buffer.from(u.slice(p+132,p+132+mn)).toString('hex')!==v.expected[i][1] || u[p+648]!==Number(v.expected[i][2]))throw Error(`key mismatch ${i}`);
  }
 }
 console.log(`${cases.length} JWKS Wasm cases match independent Python oracle`);
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
