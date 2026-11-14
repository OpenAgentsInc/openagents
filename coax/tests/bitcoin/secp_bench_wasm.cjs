// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and times its exports)
// Times the Coax verifiers and signers compiled to wasm32 in Node, on the
// keys and signatures `secp_bench N dump` prints (seven hex lines for each:
// hash, compressed key, x-only key, ECDSA signature, BIP-340 signature,
// secret key, BIP-340 aux). Every signature must verify, and must refuse
// with one bit changed; each signer must give libsecp256k1's signature.
// Prints the best of 5 passes in microseconds. For coax/tests/bitcoin/secp_bench.sh:
//   node secp_bench_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const cpuTime = !!process.env.SECP_BENCH_CPU_TIME;
const cpuUsage = process.threadCpuUsage?.bind(process) ?? process.cpuUsage.bind(process);
const now = cpuTime ? () => { const t = cpuUsage(); return t.user + t.system; }
                    : () => Number(process.hrtime.bigint()) / 1000;
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const lines = fs.readFileSync(process.argv[3], 'utf8').trim().split('\n').map(l => Buffer.from(l, 'hex'));
  const count = lines.length / 7, at = (i, k) => base + 256 * (7 * i + k);
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
    // Let tiered compilation settle before timing; background compiler CPU
    // belongs to startup, not to a steady-state verification call.
    for (let warm = 0; warm < 2048; warm++) { const i = warm % count; fn(at(i, 0), at(i, key), at(i, sig)); }
    let best = Infinity;
    for (let pass = 0; pass < 5; pass++) {
      const start = now();
      for (let i = 0; i < count; i++) fn(at(i, 0), at(i, key), at(i, sig));
      best = Math.min(best, (now() - start) / count);
    }
    console.log(`${name.padEnd(16)} ${best.toFixed(2).padStart(10)}`);
  }
  // Signing: out goes after the fixture; the result must equal the fixture's signature.
  const out = base + 256 * lines.length, same = (sig) => Buffer.compare(Buffer.from(view.subarray(out, out + 64)), Buffer.from(view.subarray(sig, sig + 64))) === 0;
  const signers = [['ECDSA sign', (i) => x.ecdsa_sign(at(i, 5), at(i, 0), out), 3], ['BIP-340 sign', (i) => x.schnorr_sign(at(i, 5), at(i, 6), at(i, 0), out), 4]];
  for (const [name, fn, sig] of signers) {
    for (let i = 0; i < count; i++) if (fn(i) !== 1 || !same(at(i, sig))) throw new Error(`${name} ${i}: not libsecp256k1's signature`);
    for (let warm = 0; warm < 256; warm++) fn(warm % count);
    let best = Infinity;
    for (let pass = 0; pass < 5; pass++) {
      const start = now();
      for (let i = 0; i < count; i++) fn(i);
      best = Math.min(best, (now() - start) / count);
    }
    console.log(`${name.padEnd(16)} ${best.toFixed(2).padStart(10)}`);
  }
  if (cpuTime) console.log(`clock: ${process.threadCpuUsage ? 'thread' : 'process'} CPU time`);
  console.log(`wasm32-wasip1 in Node ${process.version}; module ${fs.statSync(process.argv[2]).size} bytes`);
})().catch(e => { process.stderr.write(String(e.stack || e) + '\n'); process.exitCode = 1; });
