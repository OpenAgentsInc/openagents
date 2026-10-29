const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 for(let m=0;m<4096;m++){
  const expected=(!(m&1)||(!(m&2048)&&!!(m&2)))&&(!(m&4)||!!(m&8))&&(!(m&16)||!!(m&32))&&(!(m&64)||!!(m&128))&&!(m&256)&&!(m&512);
  const got=instance.exports.filter_case(m);if(got!==Number(expected))throw Error(`mask ${m}: ${got} vs ${expected}`);
 }
 if(instance.exports.filter_two_tags()!==0)throw Error('multiple tags must all match');
 const base=Number(instance.exports.__heap_base.value)+1024,u=new Uint8Array(memory.buffer);const n=instance.exports.filter_serialize(base,512);
 const got=JSON.parse(Buffer.from(u.slice(base,base+n)).toString('utf8'));
 const want={ids:['53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc'],kinds:[1,42],'#x':['a"b\né'],since:-1,limit:0};
 if(JSON.stringify(got)!==JSON.stringify(want)||instance.exports.filter_serialize(base,1)!==-1)throw Error('serializer mismatch');
 console.log('4096 independently modeled filter combinations and serialization passed in Wasm');
})().catch(e=>{console.error(e.stack||e);process.exitCode=1;});
