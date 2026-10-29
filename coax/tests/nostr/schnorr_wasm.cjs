const fs=require('node:fs'),{WASI}=require('node:wasi'),{performance}=require('node:perf_hooks');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,places=[base,base+2097216,base+2097344],u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8')),timing=[];
 for(const row of rows){
  const values=[row.message,row.public,row.signature].map(v=>Buffer.from(v,'hex'));
  values.forEach((bytes,i)=>{u.fill(165,places[i]-8,places[i]+bytes.length+8);u.set(bytes,places[i]);});
  const counts=row.counts||values.map(v=>v.length),start=performance.now(),result=x.verify(places[0],counts[0],places[1],counts[1],places[2],counts[2],row.nostr?1:0),elapsed=performance.now()-start;
  if(Boolean(result)!==row.expected)throw Error(`Wasm verification mismatch:${row.name}`);
  values.forEach((bytes,i)=>{if(Buffer.from(u.slice(places[i],places[i]+bytes.length)).compare(bytes))throw Error('input modified');if(u.slice(places[i]-8,places[i]).some(v=>v!==165)||u.slice(places[i]+bytes.length,places[i]+bytes.length+8).some(v=>v!==165))throw Error('input guard modified');});
  timing.push({name:row.name,nostr:!!row.nostr,valid:row.expected,milliseconds:elapsed});
 }
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node${process.version} V8${process.versions.v8}`,cases:rows.length,failures:0,timing_scope:'exported C/Wasm call; excludes writes and borrowed-input comparisons',timing}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
