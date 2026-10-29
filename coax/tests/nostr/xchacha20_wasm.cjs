const fs=require('fs'),assert=require('assert'),crypto=require('crypto'),{WASI}=require('node:wasi');
const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
if(process.argv[2]==='oracle') {
  const out=cases.map(c=>{const iv=Buffer.alloc(16);iv.writeUInt32LE(c.counter,0);Buffer.from(c.nonce.slice(16)).copy(iv,8);
    const cipher=crypto.createCipheriv('chacha20',Buffer.from(c.subkey),iv);
    return Array.from(Buffer.concat([cipher.update(Buffer.from(c.input)),cipher.final()]));});
  fs.writeFileSync(1,JSON.stringify(out));process.exit(0);
}
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  for(let i=0;i<5;i++) assert.equal(e.xc_refusal(i),1);
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.xc_key(),32).set(c.key);
    new Uint8Array(e.memory.buffer,e.xc_nonce(),24).set(c.nonce);
    new Uint8Array(e.memory.buffer,e.xc_input(),c.input.length).set(c.input);
    assert.equal(e.xc_run(c.input.length,c.counter),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.xc_output(),c.expected.length)),c.expected);
  }
  console.log(`Wasm: ${cases.length} OpenSSL XChaCha20 inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
