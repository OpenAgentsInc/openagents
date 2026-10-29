const fs = require('node:fs');
const {WASI} = require('node:wasi');
const wasi = new WASI({version:'preview1', args:['fixture', process.argv[3]], env:{}});
WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {wasi_snapshot_preview1:wasi.wasiImport})
  .then(({instance}) => wasi.start(instance)).catch(e => {console.error(e); process.exitCode = 1;});
