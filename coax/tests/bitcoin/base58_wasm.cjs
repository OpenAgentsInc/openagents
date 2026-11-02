// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax Base58 code compiled to wasm32 in Node's WASI on every case
// of the fixture and prints how many results and outputs match. Each case
// is {op, input (hex), result, output (hex)}; WIF encoding also carries
// compressed and testnet. For coax/tests/bitcoin/base58.py:
//   node base58_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const IN = base, OUT = base + 4096, WORK = base + 8192, CAP = 2048;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const input = hex(c.input);
    view().set(input, IN);
    view().fill(0xa5, OUT, OUT + CAP);
    let r, written;
    switch (c.op) {
      case 'encode': r = x.encode(IN, input.length, OUT, CAP); written = r; break;
      case 'decode': r = x.decode(IN, input.length, OUT, CAP); written = r; break;
      case 'check_encode': r = x.check_encode(IN, input.length, OUT, CAP); written = r; break;
      case 'check_decode': r = x.check_decode(IN, input.length, OUT, CAP); written = r; break;
      case 'address_decode': r = x.address_decode(IN, input.length, OUT); written = r > 0 ? 20 : 0; break;
      case 'wif_encode': r = x.wif_encode(IN, c.compressed ? 1 : 0, c.testnet ? 1 : 0, WORK, OUT, CAP); written = r; break;
      case 'wif_decode': r = x.wif_decode(IN, input.length, WORK, OUT); written = r > 0 ? 32 : 0; break;
      default: throw new Error('unknown op ' + c.op);
    }
    const output = Buffer.from(view().slice(OUT, OUT + Math.max(0, written))).toString('hex');
    const wiped = c.op.startsWith('wif') ? view().slice(WORK, WORK + 42).every(b => b === 0) : true;
    if (r === c.result && output === c.output && wiped) agree++;
    else process.stderr.write(`${c.op} ${c.input}: got ${r} ${output}, want ${c.result} ${c.output}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
