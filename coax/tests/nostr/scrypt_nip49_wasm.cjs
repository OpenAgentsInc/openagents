const fs=require('fs'),assert=require('assert'),{WASI}=require('node:wasi');
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.sk_password(),c.password.length).set(c.password);
    new Uint8Array(e.memory.buffer,e.sk_salt(),16).set(c.salt);
    assert.equal(e.sk_start(c.password.length,c.log),1);assert.equal(e.sk_finish(),0);
    let calls=0;while(e.sk_phase()!=3){assert.equal(e.sk_step(c.budget),1);assert(++calls<=2*(1<<c.log));}
    assert.equal(e.sk_finish(),1);assert.equal(e.sk_finish(),0);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.sk_output(),32)),c.expected);
  }
  console.log(`Wasm: ${cases.length} independent NIP-49 scrypt inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
