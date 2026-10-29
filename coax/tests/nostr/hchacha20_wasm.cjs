const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  for(let i=0;i<4;i++) assert.equal(e.hc_refusal(i),1);
  const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.hc_key(),32).set(c.key);
    new Uint8Array(e.memory.buffer,e.hc_nonce(),16).set(c.nonce);
    assert.equal(e.hc_run(),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.hc_output(),32)),c.expected);
  }
  console.log(`Wasm: ${cases.length} independent HChaCha20 inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
