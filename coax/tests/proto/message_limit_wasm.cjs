const fs = require('fs'), assert = require('assert'), {WASI} = require('node:wasi');
(async () => {
  const wasi = new WASI({version:'preview1'});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const e = instance.exports;
  for (const c of JSON.parse(fs.readFileSync(process.argv[3], 'utf8'))) {
    const bytes = new Uint8Array(e.memory.buffer, e.limit_input(), c.n);
    bytes.fill(0); if (c.n) bytes.set(c.header);
    assert.equal(e.limit_eval(BigInt(c.n), c.which), c.error, c.name);
  }
})().catch(error => {console.error(error);process.exit(1);});
