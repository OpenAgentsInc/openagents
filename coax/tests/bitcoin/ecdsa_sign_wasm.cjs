// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax ECDSA signer compiled to wasm32 in Node's WASI on every row
// of the fixture and prints how many results match: the published
// signature, or a refusal where sig is null. For
// coax/tests/bitcoin/ecdsa_sign.py:  node ecdsa_sign_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 256, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const K = base, H = base + 64, O = base + 128;
  const bytes = () => new Uint8Array(memory.buffer);
  const rows = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  rows.forEach((row, i) => {
    bytes().set(hex(row.d), K); bytes().set(hex(row.m), H); bytes().fill(0, O, O + 64);
    const result = x.sign(K, H, O), ok = result === 1;   // 2: workspace not cleared
    const got = Buffer.from(bytes().slice(O, O + 64)).toString('hex');
    if (result !== 2 && (row.sig === null ? !ok : ok && got === row.sig)) agree++;
    else process.stderr.write(`row ${i}: wrong result\n`);
  });
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: rows.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
