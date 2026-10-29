const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  for(let i=0;i<3;i++) assert.equal(e.rm_refusal(i),1);
  const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.rm_input(),c.input.length).set(c.input);
    assert.equal(e.rm_start(c.r,c.n),1);assert.equal(e.rm_finish(),0);
    let calls=0;while(e.rm_phase()!=3){assert.equal(e.rm_step(c.budget),1);assert(++calls<=2*c.n);}
    assert.equal(e.rm_index(),c.n);assert.equal(e.rm_finish(),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.rm_output(),c.expected.length)),c.expected);
  }
  console.log(`Wasm: ${cases.length} independent resumable ROMix inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
