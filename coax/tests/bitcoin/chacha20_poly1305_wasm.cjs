// Firm reason: 2 (independent oracle: OpenSSL's chacha20-poly1305 through Node's crypto seals the vectors; it also loads the wasm module and compares its outputs)
// Two modes for coax/tests/bitcoin/chacha20_poly1305.py.
//   oracle FIXTURE: seal each request with OpenSSL's chacha20-poly1305 through
//     Node's crypto and print the sealed boxes as JSON.
//   MODULE FIXTURE: run the Coax build compiled to wasm32 in Node's WASI on
//     every case and print the counts.
const fs = require('node:fs');
const crypto = require('node:crypto');
const { WASI } = require('node:wasi');
const hex = s => Buffer.from(s, 'hex');
if (process.argv[2] === 'oracle') {
  const out = JSON.parse(fs.readFileSync(process.argv[3], 'utf8')).map(c => {
    const cipher = crypto.createCipheriv('chacha20-poly1305', hex(c.key), hex(c.nonce), {authTagLength: 16});
    cipher.setAAD(hex(c.aad), {plaintextLength: c.msg.length / 2});
    const ct = Buffer.concat([cipher.update(hex(c.msg)), cipher.final()]);
    return {ct: ct.toString('hex'), tag: cipher.getAuthTag().toString('hex')};
  });
  fs.writeFileSync(1, JSON.stringify({engine: `Node ${process.version}, OpenSSL ${process.versions.openssl}`, boxes: out}));
  process.exit(0);
}
(async () => {
  const memory = new WebAssembly.Memory({initial: 64, maximum: 1024});
  const wasi = new WASI({version: 'preview1', args: [], env: {}});
  const {instance} = await WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {env: {memory}, wasi_snapshot_preview1: wasi.wasiImport});
  wasi.initialize(instance);
  const x = instance.exports, base = Number(x.__heap_base.value) + 1024;
  const K = base, N = base + 64, A = base + 128, P = A + 8192, O = P + 65536;
  const cases = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
  const put = (at, b) => new Uint8Array(memory.buffer).set(b, at);
  const get = (at, n) => Buffer.from(new Uint8Array(memory.buffer).slice(at, at + n));
  const fill = (at, n) => new Uint8Array(memory.buffer).fill(165, at, at + n);
  const untouched = (at, n) => get(at, n).every(b => b === 165);
  const fail = (c, what) => { throw new Error(`${c.id}: ${what}`); };
  let poly = 0, valid = 0, refused = 0;
  for (const c of cases) {
    if (c.kind === 'poly') {
      const key = hex(c.key), msg = hex(c.msg);
      put(K, key); put(P, msg);
      for (const chunk of [0, 1, 7, 16, 17]) {
        fill(O, 24);
        const status = chunk ? x.poly_stream(K, P, msg.length, chunk, O) : x.poly(K, P, msg.length, O);
        if (status !== 0 || get(O, 16).toString('hex') !== c.tag || !untouched(O + 16, 8)) fail(c, `poly chunk ${chunk}`);
      }
      poly++; continue;
    }
    const key = hex(c.key), nonce = hex(c.nonce), aad = hex(c.aad), msg = hex(c.msg), box = Buffer.concat([hex(c.ct), hex(c.tag)]);
    put(K, key); put(N, nonce); put(A, aad);
    const args = (p, pn, on) => [K, key.length, N, nonce.length, A, aad.length, p, pn, O, on];
    if (c.result === 'valid') {
      put(P, msg); fill(O, box.length + 8);
      if (x.seal(...args(P, msg.length, box.length + 8)) !== 0 || !get(O, box.length).equals(box) || !untouched(O + box.length, 8)) fail(c, 'seal');
      put(P, box); fill(O, msg.length + 8);
      if (x.open(...args(P, box.length, msg.length + 8)) !== 0 || !get(O, msg.length).equals(msg) || !untouched(O + msg.length, 8)) fail(c, 'open');
      const bad = Buffer.from(box); bad[bad.length - 1 - (c.flip % 16)] ^= 1 << (c.flip % 8);
      put(P, bad); fill(O, msg.length + 8);
      if (x.open(...args(P, bad.length, msg.length + 8)) !== 7 || !untouched(O, msg.length + 8)) fail(c, 'one-bit-wrong tag');
      valid++;
    } else {
      put(P, box); fill(O, msg.length + 8);
      if (x.open(...args(P, box.length, msg.length + 8)) !== c.status || !untouched(O, msg.length + 8)) fail(c, 'refusal');
      refused++;
    }
  }
  process.stdout.write(JSON.stringify({target: 'wasm32-wasip1', engine: `Node ${process.version}`, poly, valid, refused}));
})().catch(e => { process.stderr.write(String(e.stack || e)); process.exitCode = 1; });
