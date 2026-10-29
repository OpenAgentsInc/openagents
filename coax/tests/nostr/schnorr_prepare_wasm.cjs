const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;assert.equal(e.sk_refusal(),1);
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){new Uint8Array(e.memory.buffer,e.sk_key(),32).set(c.key);assert.equal(e.sk_run(),1);assert.equal(e.sk_valid(),BigInt(c.valid));for(const n of ['public','normalized'])assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e['sk_'+n](),32)),c[n]);}
console.log('Wasm: independent BIP-340 key preparations passed');})().catch(e=>{console.error(e);process.exit(1)});
