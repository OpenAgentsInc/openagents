const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;
for(const [spans,narrow,want] of JSON.parse(fs.readFileSync(process.argv[3]))){for(let i=0;i<6;i++)e.sig_set(i,BigInt(spans[i][0]),BigInt(spans[i][1]));assert.equal(e.sig_check(narrow),want);}
console.log('Wasm: independent signing address cases passed');})().catch(e=>{console.error(e);process.exit(1)});
