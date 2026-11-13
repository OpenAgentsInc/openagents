// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and times its exports)
// Times the Coax verifiers compiled to wasm32 in Node, on the keys and
// signatures `secp_bench N dump` prints (five hex lines for each: hash,
// compressed key, x-only key, ECDSA signature, BIP-340 signature). Every
// signature must verify, and must refuse with one bit changed. Prints the
// best of 5 passes in microseconds. For coax/tests/bitcoin/secp_bench.sh:
//   node secp_bench_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const lines = fs.readFileSync(process.argv[3], 'utf8').trim().split('\n').map(l => Buffer.from(l, 'hex'));
  const count = lines.length / 5, at = (i, k) => base + 256 * (5 * i + k);
  const view = new Uint8Array(memory.buffer);
  lines.forEach((b, i) => view.set(b, base + 256 * i));
  const kinds = [['ECDSA verify', x.ecdsa, 1, 3], ['BIP-340 verify', x.schnorr, 2, 4]];
  for (const [name, fn, key, sig] of kinds) {
    for (let i = 0; i < count; i++) {
      if (fn(at(i, 0), at(i, key), at(i, sig)) !== 1) throw new Error(`${name} ${i}: a valid signature refused`);
      view[at(i, sig) + 40] ^= 1;
      if (fn(at(i, 0), at(i, key), at(i, sig)) !== 0) throw new Error(`${name} ${i}: a changed signature accepted`);
      view[at(i, sig) + 40] ^= 1;
    }
    let best = Infinity;
    for (let pass = 0; pass < 5; pass++) {
      const start = process.hrtime.bigint();
      for (let i = 0; i < count; i++) fn(at(i, 0), at(i, key), at(i, sig));
      best = Math.min(best, Number(process.hrtime.bigint() - start) / 1000 / count);
    }
    console.log(`${name.padEnd(16)} ${best.toFixed(2).padStart(10)}`);
  }
  console.log(`wasm32-wasip1 in Node ${process.version}; module ${fs.statSync(process.argv[2]).size} bytes`);
})().catch(e => { process.stderr.write(String(e.stack || e) + '\n'); process.exitCode = 1; });
