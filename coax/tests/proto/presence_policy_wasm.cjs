const fs = require('fs'), assert = require('assert'), {WASI} = require('node:wasi');
(async () => {
  const wasi = new WASI({version:'preview1'});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const e = instance.exports;
  for (const item of JSON.parse(fs.readFileSync(process.argv[3], 'utf8'))) {
    const bytes = Uint8Array.from(item.wire);
    new Uint8Array(e.memory.buffer, e.policy_input(), bytes.length).set(bytes);
    assert.equal(e.policy_eval(BigInt(bytes.length),item.which),item.error,item.name);
  }
  assert.equal(e.policy_build(),0,'partial setters allow incomplete construction');
})().catch(error => {console.error(error);process.exit(1);});
