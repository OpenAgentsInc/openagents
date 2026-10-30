// Test-only observer execution. The input fields are not authorized evidence.
const fs = require('node:fs'), {WASI} = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 32, maximum: 128});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env:{memory}, wasi_snapshot_preview1:wasi.wasiImport});
  wasi.initialize(instance);
  const rows = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  const u = new Uint8Array(memory.buffer), base = Number(instance.exports.__heap_base.value) + 1024;
  for (const row of rows) {
    let offset = base, args = [];
    for (const value of row.fields) {
      const bytes = Buffer.from(value, 'utf8');
      if (offset + bytes.length + 16 >= u.length) throw Error('fixture capacity');
      u.set(bytes, offset); args.push(offset, bytes.length); offset += bytes.length + 1;
    }
    const got = instance.exports.observe(BigInt(row.kind), BigInt(row.created), BigInt(row.now),
      ...args, BigInt(row.uc), BigInt(row.mc), BigInt(row.pc), row.malformed ? 1 : 0);
    if (Boolean(got) !== row.expected) throw Error(`observation mismatch ${row.name}: ${got}`);
  }
  process.stdout.write(JSON.stringify({cases: rows.length, failures: 0, target: 'wasm32-wasip1', engine: `Node${process.version} V8${process.versions.v8}`}));
})().catch(e => {process.stderr.write(String(e.stack || e)); process.exitCode = 1;});
