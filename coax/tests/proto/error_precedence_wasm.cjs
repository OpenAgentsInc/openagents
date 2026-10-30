const fs = require('fs');
const { WASI } = require('wasi');
(async () => {
  const wasi = new WASI({version:'preview1'});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  for (let mode=0;mode<8;mode++) {
    for (const code of [1,2,3,4,5,6,7,0xdeadbeef]) {
      const value = instance.exports.probe_bad(mode, code) >>> 0;
      if (value !== code) throw Error(`guard mode=${mode} code=${code} got=${value}`);
    }
    if (instance.exports.probe_wire(mode)!==1) throw Error(`wire mode=${mode}`);
  }
  console.log('inaccessible view and malformed wire controls passed');
})().catch(e=>{console.error(e);process.exit(1)});
