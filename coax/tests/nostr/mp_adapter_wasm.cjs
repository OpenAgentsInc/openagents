// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){const wire=Uint8Array.from(c.wire);new Uint8Array(e.memory.buffer,e.mpa_input(),wire.length).set(wire);const n=Number(e.mpa_test_run(BigInt(wire.length)));assert.equal(n>=0,c.consistent,c.name);if(c.consistent)assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.mpa_output(),n)),c.original,c.name);}
console.log('Wasm MP source/projection tests passed');})().catch(e=>{console.error(e);process.exit(1)});
