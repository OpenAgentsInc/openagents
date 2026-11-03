// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax script interpreter compiled to wasm32 in Node's WASI on
// every case of the fixture and prints how many results match. A verify
// case is {tx, spk (hex), index, amount, flags, want (an error number)}; a
// sighash case is {tx, code (hex), index, amount, hashtype, v0, want (hex)}.
// For coax/tests/bitcoin/script.py:
//   node script_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 256, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const TX = base, SPK = TX + 2 * 1024 * 1024, OUT = SPK + 64 * 1024;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const tx = Buffer.from(c.tx, 'hex'), second = Buffer.from(c.spk !== undefined ? c.spk : c.code, 'hex');
    view().set(tx, TX);
    view().set(second, SPK);
    let got;
    if (c.spk !== undefined) {
      got = x.verify(TX, tx.length, SPK, second.length, c.index, c.amount, c.flags);
    } else {
      const r = x.sighash(TX, tx.length, SPK, second.length, c.index, c.amount, c.hashtype, c.v0, OUT);
      got = r < 0 ? String(r) : Buffer.from(view().slice(OUT, OUT + 32)).toString('hex');
    }
    if (got === c.want) agree++;
    else process.stderr.write(`${c.tx.slice(0, 60)} flags ${c.flags}: got ${got}, want ${c.want}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
