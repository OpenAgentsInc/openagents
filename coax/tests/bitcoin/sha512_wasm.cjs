// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Run SHA-512 compiled to wasm32 in Node's WASI and compare each digest with
// the fixture's (the NIST CAVS rows and hashlib streams from sha512.py).
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, input = Number(x.__heap_base.value) + 1024, output = input + 1048576 + 64;
  const fixtures = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  function check(v, op) {
    const source = Buffer.from(v.input, 'hex'), bytes = new Uint8Array(memory.buffer);
    bytes.set(source, input); bytes.fill(165, output, output + 72);
    const status = op(source.length);
    if (status !== 0) throw new Error(`Wasm refused a valid fixture (${status})`);
    const actual = Buffer.from(bytes.slice(output, output + 64)).toString('hex');
    if (actual !== v.digest) throw new Error(`Wasm digest mismatch: ${actual} != ${v.digest}`);
    if (bytes.slice(output + 64, output + 72).some(b => b !== 165)) throw new Error('Wasm wrote past the digest');
  }
  for (const v of fixtures.vectors) {
    check(v, n => x.sha_hash(input, n, output));
    check(v, n => x.sha_legacy(input, n, output));
    check(v, n => x.sha_stream(input, n, 113, output));
  }
  for (const v of fixtures.streams) check(v, n => x.sha_stream(input, n, v.chunk, output));
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, nist_rows: fixtures.vectors.length, streams: fixtures.streams.length}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
