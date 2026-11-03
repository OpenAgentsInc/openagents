// Firm reason: 5 (a thin loader that hands the wasm module the WASI host and compares its outputs)
// Runs the Coax recovery, recoverable signer and signed-message code
// compiled to wasm32 in Node's WASI on every row of the fixture and prints
// how many results match. For coax/tests/bitcoin/ecdsa_recover.py:
//   node signed_messages_wasm.cjs MODULE FIXTURE
const fs = require('node:fs');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
(async () => {
  const memory = new WebAssembly.Memory({initial: 256, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const A = base, B = base + 4096, O = base + 8192, C = base + 12288;   // C: messages of any length
  const bytes = () => new Uint8Array(memory.buffer);
  const put = (at, b) => { bytes().set(b, at); return b.length; };
  const out = n => Buffer.from(bytes().slice(O, O + n));
  const fixture = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  let cases = 0, agree = 0;
  const check = (ok, label) => { cases++; if (ok) agree++; else process.stderr.write(`${label}: wrong result\n`); };
  fixture.recover.forEach((row, i) => {
    const hn = put(A, hex(row.m)); put(B, hex(row.sig)); bytes().fill(0, O, O + 65);
    const n = x.recover(A, hn, B, row.id, 33, O);
    check(row.key === null ? n === row.code : n === 33 && out(33).toString('hex') === row.key, `recover ${i}`);
  });
  fixture.sign.forEach((row, i) => {
    put(A, hex(row.d)); put(B, hex(row.m)); bytes().fill(0, O, O + 65);
    const r = x.sign(A, B, O);
    check(r === 1 && out(65).toString('hex') === row.sig, `sign ${i}`);
  });
  fixture.verify.forEach((row, i) => {
    const an = put(A, Buffer.from(row.address)), sn = put(B, Buffer.from(row.sig)), mn = put(C, hex(row.message));
    check(x.message_verify(A, an, B, sn, C, mn, row.testnet ? 1 : 0) === row.code, `verify ${i}`);
  });
  fixture.message.forEach((row, i) => {
    put(A, hex(row.d)); const mn = put(C, hex(row.message));
    const n = x.message_sign(A, row.compressed ? 1 : 0, C, mn, O);
    check(n === 88 && out(88).toString() === row.sig, `message ${i}`);
  });
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, cases, agree}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
