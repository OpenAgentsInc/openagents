// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax strict DER checks compiled to wasm32 in Node's WASI on
// every input of the fixture: the refusal code, the script check under all
// eight flag sets and the lax parser's r || s. Prints how many inputs agree
// with the expected values. For coax/tests/bitcoin/der.py:
//   node der_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const S = base, O = base + 256;
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let agree = 0;
  for (const c of cases) {
    const sig = hex(c.sig);
    new Uint8Array(memory.buffer).set(sig, S);
    let ok = x.check(S, sig.length) === c.code;
    for (let f = 0; f < 8; f++) ok = ok && x.script(S, sig.length, f) === c.script[f];
    new Uint8Array(memory.buffer).fill(0, O, O + 64);
    const read = x.lax(S, sig.length, O) === 1;
    ok = ok && read === c.read && (!read || Buffer.from(memory.buffer, O, 64).toString('hex') === c.lax);
    if (ok) agree++;
    else process.stderr.write(`${c.sig}: wrong result\n`);
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases: cases.length, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
