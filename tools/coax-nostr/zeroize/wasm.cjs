const fs=require('node:fs');const{WASI}=require('node:wasi');
(async()=>{
 const wasi=new WASI({version:'preview1',args:[],env:{}});const{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const x=instance.exports;
 if(x.zero_probe){if(x.zero_probe(17)!==0)throw new Error('Unobserved probe refused');process.stdout.write(JSON.stringify({probe:true}));return;}
 const memory=x.memory;memory.grow(20);const bytes=new Uint8Array(memory.buffer),p=Number(x.__heap_base.value)+64;
 let cases=0;for(let cap=0;cap<=64;cap++)for(let at=0;at<=cap;at++){
  const n=cap-at;bytes.fill(165,p-1,p+cap+1);if(x.range(p,cap,at,n)!==0)throw new Error('Valid range refused');
  for(let i=-1;i<=cap;i++)if(bytes[p+i]!==((i>=at&&i<cap)?0:165))throw new Error('Wasm range/guard mismatch');cases++;}
 for(const args of [[-1,0,0],[4,-1,1],[4,0,5],[1048577,0,0]])if(x.range(p,...args)===0)throw new Error('Bad metadata accepted');
 bytes.fill(165,p,p+1048576);if(x.range(p,1048576,0,1048576)!==0||bytes.slice(p,p+1048576).some(x=>x!==0))throw new Error('Wasm full range not zero');
 for(let reason=0;reason<3;reason++)if(x.scenario(reason)!==0)throw new Error('Wasm scope cleanup failed');
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node ${process.version} V8 ${process.versions.v8}`,ranges:cases,scopes:3,max_bytes:1048576}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
