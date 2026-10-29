const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  for(let i=0;i<5;i++) assert.equal(e.bm_refusal(i),1);
  const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.bm_input(),c.input.length).set(c.input);
    assert.equal(e.bm_run(c.r),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.bm_output(),c.expected.length)),c.expected);
  }
  console.log(`Wasm: ${cases.length} independent BlockMix inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
