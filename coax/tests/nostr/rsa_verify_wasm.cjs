// Test-only Wasm ABI; expected decisions are computed by the Python integer oracle.
const fs = require('node:fs'), {WASI} = require('node:wasi');
(async () => {
 const memory = new WebAssembly.Memory({initial:64, maximum:1024});
 const wasi = new WASI({version:'preview1', args:[], env:{}});
 const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env:{memory}, wasi_snapshot_preview1:wasi.wasiImport});
 wasi.initialize(instance);
 const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
 const u = new Uint8Array(memory.buffer), vectors = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
 for (const v of vectors) {
  const n = Buffer.from(v.n,'hex'), s = Buffer.from(v.s,'hex'), m = Buffer.from(v.m,'hex');
  u.set(n,base); u.set(s,base+1024); u.set(m,base+2048);
  const got = x.verify(base,n.length,base+1024,s.length,base+2048,m.length);
  if (got !== Number(v.expected)) throw Error(`${v.size} case ${v.id}: ${got} != ${v.expected}`);
 }
 console.log(`${vectors.length} Wasm cases match independent integer oracle`);
})().catch(e => {console.error(e.stack || e); process.exitCode = 1;});
