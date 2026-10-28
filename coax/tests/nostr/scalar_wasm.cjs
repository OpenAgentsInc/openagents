// Real wasm32/V8/WASI observations against independently computed fixtures.
const fs=require('node:fs');const{WASI}=require('node:wasi');
(async()=>{
 const memory=new WebAssembly.Memory({initial:512,maximum:1024});const wasi=new WASI({version:'preview1',args:[],env:{}});
 const{instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{env:{memory},wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);
 const bytes=new Uint8Array(memory.buffer),a=Number(instance.exports.__heap_base.value)+1024,b=a+64,out=b+64;const rows=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
 for(const row of rows){bytes.set(Buffer.from(row.a,'hex'),a);bytes.set(Buffer.from(row.b,'hex'),b);bytes.fill(165,out,out+40);const ok=instance.exports.evaluate(row.op,a,b,out);
 if(Boolean(ok)!==(row.value!==null))throw new Error('Wasm scalar refusal mismatch');
 if(ok&&Buffer.from(bytes.slice(out,out+32)).toString('hex')!==row.value)throw new Error('Wasm scalar integer-oracle disagreement');
 const guard=ok?out+32:out;if(bytes.slice(guard,out+40).some(x=>x!==165))throw new Error('Wasm output changed outside commit');}
 process.stdout.write(JSON.stringify({target:'wasm32-wasip1',engine:`Node ${process.version} V8 ${process.versions.v8}`,comparisons:rows.length}));
})().catch(e=>{process.stderr.write(String(e.stack||e));process.exitCode=1;});
