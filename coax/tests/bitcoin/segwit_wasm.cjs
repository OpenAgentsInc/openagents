// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax segwit address code compiled to wasm32 in Node's WASI on
// every case of the fixture and prints how many results and outputs match.
// Each case is {op, input (hex), result, output (hex), network, version,
// capacity}. For coax/tests/bitcoin/segwit.py:
//   node segwit_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const IN = base, OUT = base + 1024, INFO = base + 2048, SPAN = 128;
  const view = () => new Uint8Array(memory.buffer);
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const input = hex(c.input);
    view().set(input, IN);
    view().fill(0xa5, OUT, OUT + SPAN);
    let r, network = 0, version = 0;
    switch (c.op) {
      case 'variant': r = x.variant(IN, input.length); break;
      case 'decode': {
        r = x.decode(IN, input.length, OUT, c.capacity, INFO);
        const info = new Int32Array(memory.buffer, INFO, 2);
        network = info[0]; version = info[1];
        break;
      }
      case 'encode': r = x.encode(c.network, c.version, IN, input.length, OUT, c.capacity); network = c.network; version = c.version; break;
      default: throw new Error('unknown op ' + c.op);
    }
    const written = c.op === 'variant' ? 0 : Math.max(0, r);
    const output = Buffer.from(view().slice(OUT, OUT + written)).toString('hex');
    const tail = view().slice(OUT + written, OUT + SPAN).every(b => b === 0xa5);
    if (r === c.result && network === c.network && version === c.version && output === c.output && tail) agree++;
    else process.stderr.write(`${c.op} ${c.input}: got ${r} ${network} ${version} ${output}, want ${c.result} ${c.network} ${c.version} ${c.output}\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
