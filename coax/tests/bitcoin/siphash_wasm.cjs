// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Run SipHash-2-4 compiled to wasm32 in Node's WASI and compare each tag with
// the fixture's (the 64 reference vectors and the oracle cases from siphash.py).
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, key = Number(x.__heap_base.value) + 1024, input = key + 64, output = input + 65536;
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  for (const v of cases) {
    const k = Buffer.from(v.key, 'hex'), m = Buffer.from(v.input, 'hex'), bytes = new Uint8Array(memory.buffer);
    bytes.set(k, key); bytes.set(m, input); bytes.fill(165, output, output + 16);
    const status = x.sip_hash(key, input, m.length, output);
    if (status !== 0) throw new Error(`Wasm refused a valid case (${status})`);
    const actual = Buffer.from(bytes.slice(output, output + 8)).toString('hex');
    if (actual !== v.tag) throw new Error(`Wasm tag mismatch: ${actual} != ${v.tag}`);
    if (bytes.slice(output + 8, output + 16).some(b => b !== 165)) throw new Error('Wasm wrote past the tag');
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
