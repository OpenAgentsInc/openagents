const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  assert.equal(instance.exports.wipe_fixture(),0);
  assert.equal(instance.exports.wipe_fixture(),0);
  console.log('Wasm: chunked observable wipe and reuse passed');
})().catch(e=>{console.error(e);process.exit(1)});
