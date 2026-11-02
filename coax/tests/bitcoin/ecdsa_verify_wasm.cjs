// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax ECDSA verifier compiled to wasm32 in Node's WASI on every
// case of the fixture, with the key uncompressed and compressed, and prints
// how many decisions match the expected result. For
// coax/tests/bitcoin/ecdsa_verify.py:  node ecdsa_verify_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const H = base, K = base + 64, S = base + 192;
  const put = (at, b) => new Uint8Array(memory.buffer).set(b, at);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const sig = hex(c.der);
    put(H, hex(c.digest)); put(S, sig);
    for (const key of [hex(c.key), hex(c.compressed)]) {
      put(K, key);
      if ((x.verify(H, K, key.length, S, sig.length, 1) === 1) === c.valid) agree++;
      else process.stderr.write(`Wycheproof #${c.id}: wrong result\n`);
    }
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
