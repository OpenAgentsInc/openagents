// Test-only Wasm runner for coax/tests/nostr/schnorr_self_verify.py.
const fs=require('node:fs'),{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:64,maximum:1024}),wasi=new WASI({version:'preview1',args:[],env:{}});
 const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const x=instance.exports,base=Number(x.__heap_base.value)+1024,places=[base,base+2097216,base+2097344],u=new Uint8Array(memory.buffer),rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 let entries=0;
 for(const row of rows){
  const values=[row.message,row.public,row.signature].map(v=>Buffer.from(v,'hex'));
  values.forEach((bytes,i)=>{u.fill(165,places[i]-8,places[i]+bytes.length+8);u.set(bytes,places[i]);});
  const n=values.map(v=>BigInt(v.length));
  for(const nostr of [0,1]){
   const want=row.expected&&(!nostr||values[0].length===32),refused=nostr&&values[0].length!==32;
   const got=x.ssv_case(places[0],n[0],places[1],n[1],places[2],n[2],nostr,0,1n,0);entries++;
   if(got<0||Boolean(got&1)!==want||(refused&&!(got&4))||(!refused&&!(got&2)))throw Error('fixed-work mismatch:'+row.name+':'+nostr+':'+got);
  }
  if(Boolean(x.old_verify(places[0],n[0],places[1],n[1],places[2],n[2]))!==row.expected)throw Error('oracle mismatch:'+row.name);
  values.forEach((bytes,i)=>{if(Buffer.from(u.slice(places[i],places[i]+bytes.length)).compare(bytes)||u.slice(places[i]-8,places[i]).some(v=>v!==165)||u.slice(places[i]+bytes.length,places[i]+bytes.length+8).some(v=>v!==165))throw Error('input or guard modified');});
 }
 const v=[0,1].map(i=>places[i]);
 for(const c of [[-1n,32n,64n,1n],[1048577n,32n,64n,1n],[32n,31n,64n,1n],[32n,32n,63n,1n],[32n,32n,64n,0n],[32n,32n,64n,2n]])if(x.ssv_case(places[0],c[0],places[1],c[1],places[2],c[2],0,0,c[3],0)!==4)throw Error('metadata accepted');
 if(x.ssv_case(places[0],32n,places[1],32n,places[2],64n,0,0,1n,1)!==2)throw Error('faulted stage accepted');
 process.stdout.write(JSON.stringify({cases:rows.length,entries,work_bytes:x.ssv_work_size(),failures:0}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
