const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;assert.equal(e.ss_refusal(),1);
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){new Uint8Array(e.memory.buffer,e.ss_input(),32).set(c.key);for(const odd of [0,1]){assert.equal(e.ss_run(odd),1);assert.equal(e.ss_valid(),BigInt(c.valid));assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.ss_output(),32)),odd?c.odd:c.even);}}
console.log('Wasm: independent scalar admission/parity inputs passed');})().catch(e=>{console.error(e);process.exit(1)});
