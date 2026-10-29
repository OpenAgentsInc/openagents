const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,places=[base,base+2097216,base+2097344],u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of rows){
  const values=[row.message,row.public,row.signature].map(v=>Buffer.from(v,'hex'));
  values.forEach((bytes,i)=>{u.fill(165,places[i]-8,places[i]+bytes.length+8);u.set(bytes,places[i]);});
  for(const nostr of [0,1]){
   const expected=row.expected&&(!nostr||values[0].length===32),args=[places[0],values[0].length,places[1],values[1].length,places[2],values[2].length,nostr];
   if(Boolean(x.verify(...args))!==expected||Boolean(x.old_verify(...args))!==expected)throw Error('verification mismatch:'+row.name+':'+nostr);
  }
  values.forEach((bytes,i)=>{if(Buffer.from(u.slice(places[i],places[i]+bytes.length)).compare(bytes)||u.slice(places[i]-8,places[i]).some(v=>v!==165)||u.slice(places[i]+bytes.length,places[i]+bytes.length+8).some(v=>v!==165))throw Error('input or guard modified');});
 }
 process.stdout.write(JSON.stringify({cases:rows.length,entries:2,failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
