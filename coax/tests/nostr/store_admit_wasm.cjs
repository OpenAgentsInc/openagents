const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:256}),wasi=new WASI({version:'preview1',args:[],env:{}}),{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=(Number(x.__heap_base.value)+4095)&~7,input=base+16,scratch=base+4096,out=base+8192,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of rows){
  const text=Buffer.from(row.text,'base64'),expected=Buffer.from(row.frame,'base64');u.fill(0xa5,input-8,input+text.length+8);u.set(text,input);u.fill(0xa5,scratch-8,scratch+1032);u.fill(0xa5,out-8,out+2056);
  const got=x.admit(input,BigInt(text.length),scratch,1024n,out,BigInt(row.capacity),row.overlap?1:0);if(got!==row.status)throw Error(`status ${row.name}: ${got}`);
  if(!Buffer.from(u.slice(input,input+text.length)).equals(text))throw Error('input '+row.name);
  if(!got&&!Buffer.from(u.slice(out,out+expected.length)).equals(expected))throw Error('frame '+row.name);
  if(got&&u.slice(out,out+2048).some(v=>v!==0xa5))throw Error('refusal output '+row.name);
  for(const [start,end] of [[input-8,input],[input+text.length,input+text.length+8],[scratch-8,scratch],[scratch+1024,scratch+1032],[out-8,out],[out+2048,out+2056]])if(u.slice(start,end).some(v=>v!==0xa5))throw Error('guard '+row.name);
 }
 process.stdout.write(JSON.stringify({engine:`Node ${process.version} V8 ${process.versions.v8}`,cases:rows.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
