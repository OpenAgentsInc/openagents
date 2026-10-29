const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;assert.equal(e.so_refusal(),1);
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){new Uint8Array(e.memory.buffer,e.so_input(),c.input.length).set(c.input);for(const chunk of [1,55,64,129,4096]){assert.equal(e.so_run(c.input.length,chunk),1);assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.so_output(),32)),c.expected);}}
console.log('Wasm: independent SHA vectors and streamed chunks passed');})().catch(e=>{console.error(e);process.exit(1)});
