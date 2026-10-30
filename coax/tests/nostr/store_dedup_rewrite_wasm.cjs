const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:128,maximum:256}),wasi=new WASI({version:'preview1',args:[],env:{}}),{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=(Number(x.__heap_base.value)+4095)&~7,old=base+16,text=base+1400000,flags=base+1700000,out=base+1800000,u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of rows){
  const a=Buffer.from(row.old,'base64'),b=Buffer.from(row.text,'base64'),f=Buffer.from(row.flags),expected=Buffer.from(row.out,'base64');
  u.fill(0xa5,old-8,old+a.length+8);u.set(a,old);u.fill(0xa5,text-8,text+b.length+8);u.set(b,text);u.fill(0xa5,flags-8,flags+f.length+8);u.set(f,flags);u.fill(0xa5,out-8,out+1050648);
  const got=x.rewrite(old,BigInt(a.length),text,BigInt(b.length),flags,BigInt(f.length),out,BigInt(row.capacity),row.overlap?1:0);
  if(Boolean(got)!==row.ok)throw Error(`status ${row.name}: ${got}`);
  if(got&&!Buffer.from(u.slice(out,out+expected.length)).equals(expected))throw Error('frame '+row.name);
  if(!got&&u.slice(out,out+1050640).some(v=>v!==0xa5))throw Error('refusal output '+row.name);
  for(const [start,end] of [[old-8,old],[old+a.length,old+a.length+8],[text-8,text],[text+b.length,text+b.length+8],[flags-8,flags],[flags+f.length,flags+f.length+8],[out-8,out],[out+1050640,out+1050648]])if(u.slice(start,end).some(v=>v!==0xa5))throw Error('guard '+row.name);
  if(!Buffer.from(u.slice(old,old+a.length)).equals(a)||!Buffer.from(u.slice(text,text+b.length)).equals(b)||!Buffer.from(u.slice(flags,flags+f.length)).equals(f))throw Error('input '+row.name);
 }
 process.stdout.write(JSON.stringify({engine:`Node ${process.version} V8 ${process.versions.v8}`,cases:rows.length,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
