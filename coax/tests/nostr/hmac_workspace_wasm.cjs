const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;assert.equal(e.hm_refusal(),1);
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){new Uint8Array(e.memory.buffer,e.hm_key(),c.key.length).set(c.key);new Uint8Array(e.memory.buffer,e.hm_input(),c.input.length).set(c.input);assert.equal(e.hm_run(c.key.length,c.input.length),1);assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.hm_output(),32)),c.expected);}
console.log('Wasm: independent HMAC vectors passed');})().catch(e=>{console.error(e);process.exit(1)});
