const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){new Uint8Array(e.memory.buffer,e.sp_input(),32).set(c.scalar);assert.equal(e.sp_run(),1);assert.equal(e.sp_finite(),BigInt(c.finite));assert.equal(e.sp_odd(),BigInt(c.odd));assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.sp_output(),32)),c.x);}
console.log('Wasm: independent fixed-base affine inputs passed');})().catch(e=>{console.error(e);process.exit(1)});
