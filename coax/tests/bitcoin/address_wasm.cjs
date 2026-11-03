// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax address code compiled to wasm32 in Node's WASI on every
// case of the fixture and prints how many results and outputs match. Each
// case is {op, which, network, input (hex), capacity, result, kind, output
// (hex)}. For coax/tests/bitcoin/address.py:
//   node address_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const IN = base, OUT = base + 16384, INFO = base + 16384 + 1024, SPAN = 128;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const input = hex(c.input);
    view().set(input, IN);
    view().fill(0xa5, OUT, OUT + SPAN);
    let r, kind = 0;
    switch (c.op) {
      case 'build': r = x.build(c.which, IN, input.length, OUT, c.capacity); break;
      case 'tweak': r = x.tweak(IN, input.length, OUT, c.capacity); break;
      case 'kind': r = x.kind(IN, input.length); break;
      case 'encode': r = x.encode(c.network, IN, input.length, OUT, c.capacity); break;
      case 'decode': r = x.decode(c.network, IN, input.length, OUT, c.capacity, INFO); kind = new Int32Array(memory.buffer, INFO, 1)[0]; break;
      default: throw new Error('unknown op ' + c.op);
    }
    const written = c.op === 'kind' ? 0 : c.op === 'tweak' ? (r >= 0 ? 32 : 0) : Math.max(0, r);
    const output = Buffer.from(view().slice(OUT, OUT + written)).toString('hex');
    const tail = view().slice(OUT + written, OUT + SPAN).every(b => b === 0xa5);
    if (r === c.result && kind === c.kind && output === c.output && tail) agree++;
    else process.stderr.write(`${c.op} ${c.which} ${c.network} ${c.input}: got ${r} ${kind} ${output}, want ${c.result} ${c.kind} ${c.output}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
