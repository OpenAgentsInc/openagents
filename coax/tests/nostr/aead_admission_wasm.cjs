const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const x=instance.exports;
 for(const [spans,seal,narrow,want] of JSON.parse(fs.readFileSync(process.argv[3]))){for(let i=0;i<7;i++)x.aa_set(i,BigInt(spans[i][0]),BigInt(spans[i][1]));assert.equal(x.aa_check(seal,narrow),want);}
})().catch(e=>{console.error(e);process.exit(1)});
