const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{const wasi=new WASI({version:'preview1'});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});wasi.initialize(instance);const e=instance.exports;for(let i=0;i<5;i++)assert.equal(e.sn_refusal(i),1);
for(const c of JSON.parse(fs.readFileSync(process.argv[3],'utf8'))){for(const n of ['key','pub','aux','message'])new Uint8Array(e.memory.buffer,e['sn_'+n](),c[n].length).set(c[n]);assert.equal(e.sn_run(c.message.length),1);assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.sn_output(),32)),c.expected);}
console.log('Wasm: independent BIP-340 nonce hashes passed');})().catch(e=>{console.error(e);process.exit(1)});
