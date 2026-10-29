const fs=require('fs'),assert=require('assert'),crypto=require('crypto'),{WASI}=require('node:wasi');
const cases=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
if(process.argv[2]==='oracle') {
  const out=cases.map(c=>{const iv=Buffer.concat([Buffer.alloc(4),Buffer.from(c.nonce.slice(16))]);
    const cipher=crypto.createCipheriv('chacha20-poly1305',Buffer.from(c.subkey),iv,{authTagLength:16});
    cipher.setAAD(Buffer.from(c.aad));return Array.from(Buffer.concat([cipher.update(Buffer.from(c.message)),cipher.final(),cipher.getAuthTag()]));});
  fs.writeFileSync(1,JSON.stringify(out));process.exit(0);
}
(async()=>{
  const wasi=new WASI({version:'preview1'});
  const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);const e=instance.exports;
  for(const c of cases){
    new Uint8Array(e.memory.buffer,e.aead_key(),32).set(c.key);
    new Uint8Array(e.memory.buffer,e.aead_nonce(),24).set(c.nonce);
    new Uint8Array(e.memory.buffer,e.aead_aad(),c.aad.length).set(c.aad);
    new Uint8Array(e.memory.buffer,e.aead_input(),c.message.length).set(c.message);
    assert.equal(e.aead_seal(c.aad.length,c.message.length),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.aead_output(),c.expected.length)),c.expected);
    new Uint8Array(e.memory.buffer,e.aead_input(),c.expected.length).set(c.expected);
    for(let i=c.message.length;i<c.expected.length;i++) assert.equal(e.aead_tamper(c.aad.length,c.expected.length,i),1);
    if(c.message.length) assert.equal(e.aead_tamper(c.aad.length,c.expected.length,0),1);
    assert.equal(e.aead_open(c.aad.length,c.expected.length),1);
    assert.deepStrictEqual(Array.from(new Uint8Array(e.memory.buffer,e.aead_output(),c.message.length)),c.message);
  }
  console.log(`Wasm: ${cases.length} OpenSSL AEAD inputs passed`);
})().catch(e=>{console.error(e);process.exit(1)});
