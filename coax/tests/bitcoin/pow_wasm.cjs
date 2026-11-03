// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax proof-of-work code compiled to wasm32 in Node's WASI on
// every case of the fixture and prints how many reports match, with the
// output buffer past each report untouched. Each case is {op, input (hex),
// index, report (hex)}. For coax/tests/bitcoin/pow.py:
//   node pow_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const IN = base, CAP = 4096, OUT = IN + CAP;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const input = Buffer.from(c.input, 'hex');
    view().set(input, IN);
    view().fill(0xa5, OUT, OUT + CAP);
    const r = x.report(c.op, IN, input.length, c.index >>> 0, OUT, CAP);
    const got = r < 0 ? String(r) : Buffer.from(view().slice(OUT, OUT + r)).toString('hex');
    const rest = r < 0 || view().slice(OUT + r, OUT + CAP).every(b => b === 0xa5);
    if (got === c.report && rest) agree++;
    else process.stderr.write(`op ${c.op} ${c.index} ${c.input.slice(0, 80)}: got ${got.slice(0, 64)}, want ${c.report.slice(0, 64)}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
