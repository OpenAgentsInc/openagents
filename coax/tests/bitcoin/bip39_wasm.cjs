// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax BIP-39 code compiled to wasm32 in Node's WASI on every case
// of the fixture and prints how many results and outputs match. Each case
// is {op, input (hex), passphrase (hex), room, result, output (hex), touched}.
// For coax/tests/bitcoin/bip39.py:
//   node bip39_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const IN = base, PASS = base + 4096, OUT = base + 8192, WORK = base + 12288, CAP = 2048, WORK_BYTES = 1520;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const input = hex(c.input), pass = hex(c.passphrase);
    view().set(input, IN); view().set(pass, PASS);
    view().fill(0xa5, OUT, OUT + CAP); view().fill(0x5a, WORK, WORK + WORK_BYTES);
    let r;
    switch (c.op) {
      case 'encode': r = x.encode(IN, input.length, WORK, OUT, c.room); break;
      case 'decode': r = x.decode(IN, input.length, WORK, OUT, c.room); break;
      case 'seed': r = x.seed(IN, input.length, PASS, pass.length, WORK, OUT, c.room); break;
      default: throw new Error('unknown op ' + c.op);
    }
    const output = Buffer.from(view().slice(OUT, OUT + Math.max(0, r))).toString('hex');
    const work = view().slice(WORK, WORK + WORK_BYTES);
    const wiped = c.touched ? work.every(b => b === 0) : work.every(b => b === 0x5a);
    const rest = view().slice(OUT + Math.max(0, r), OUT + CAP).every(b => b === 0xa5);
    if (r === c.result && output === c.output && wiped && rest) agree++;
    else process.stderr.write(`${c.op} ${c.input}: got ${r} ${output}, want ${c.result} ${c.output}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
