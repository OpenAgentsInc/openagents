// Firm reason: 2 (independent SDK-effect oracle for the actual legacy pack loader's recovery trace).
// The scene/parser/GPU effects are doubles; fetch count, retained identity, SHA re-admission and
// replacement-device uploads are observed independently of the loader's cache implementation.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { webcrypto } = require('node:crypto');
const source = fs.readFileSync('coax/web/host.js', 'utf8');
const start = source.indexOf('async function loadPack(pack) {');
const end = source.indexOf('// The Coax companion admits complete bytes', start);
assert(start >= 0 && end > start, 'locate the actual legacy loader');
const loader = source.slice(start, end);
async function fixture({ broker = false, weak = true } = {}) {
  const bytes = Uint8Array.from({ length: 64 }, (_, i) => i);
  new DataView(bytes.buffer).setUint32(8, 64, true);
  const expected = Buffer.from(await webcrypto.subtle.digest('SHA-256', bytes)).toString('hex');
  const pack = { url: '/fixture.pack', broker, quiet: true };
  const assets = ['data', 'storage', 'index', 'layers'].map(kind => ({ kind, pack }));
  const events = { fetches: 0, sessions: 0, hashes: 0, admitted: 0, uploads: [], errors: [], bound: 0 };
  const memory = { buffer: new ArrayBuffer(64) };
  const exports = {
    coaxial_pack_gate_version: () => 1,
    coaxial_pack_digest: () => 8,
    coaxial_pack_digest_capacity: () => 32,
    coaxial_pack_admit: (_state, length) => {
      events.hashes++;
      const digest = Buffer.from(new Uint8Array(memory.buffer, 8, 32)).toString('hex');
      if (length !== bytes.length || digest !== expected) return 0;
      events.admitted++;
      return 1;
    },
  };
  const device = id => ({
    createBuffer: desc => ({ device: id, bytes: desc.size }),
    queue: { writeBuffer: (buffer, at, data) => events.uploads.push({ device: buffer.device, at, size: data.length }) },
  });
  const context = vm.createContext({
    Uint8Array, DataView, Number, Promise, Object, Array, Math, Boolean, WeakRef: weak ? WeakRef : undefined,
    crypto: webcrypto, performance, memory, engine: { exports }, wanted: 1,
    GPUBufferUsage: { INDEX: 1, STORAGE: 2, COPY_DST: 4 },
    hostTransport: null, renderUrl: null, assets, gpu: { device: device(1) },
    PACK_CATALOG: 1, PACK_VERTICES: 2, PACK_INDICES: 3, PACK_LAYER: 4, PACK_SOUND: 5, PACK_KIND: 605,
    arena: { state: 0 }, dev: true, app: 'fixture',
    loading: { bytes() {}, textures() {}, assetDone() {} },
    console: { error: (...args) => events.errors.push(args.join(' ')) },
    send() {}, say: (...args) => events.errors.push(args.join(' ')),
    currentAsset: () => true, align: (n, a) => Math.ceil(n / a) * a,
    unpack: payload => [
      { kind: 1, bytes: payload.subarray(0, 8) },
      { kind: 2, bytes: payload.subarray(16, 24) },
      { kind: 3, bytes: payload.subarray(24, 32) },
      { kind: 4, bytes: payload.subarray(32) },
    ],
    layerTexture: async () => ({ device: context.gpu.device.createBuffer({ size: 0 }).device }),
    deliver() {}, bindScene: () => events.bound++, privatePack: async (_session, payload) => payload,
    fetch: async (_url, options) => {
      if (options?.method === 'POST') {
        events.sessions++;
        return { ok: true, json: async () => ({ url: '/signed.pack', size: bytes.length }) };
      }
      events.fetches++;
      let sent = false;
      return {
        ok: true, headers: { get: () => String(bytes.length) },
        body: { getReader: () => ({ read: async () => sent ? { done: true } : (sent = true, { done: false, value: bytes.slice() }) }) },
      };
    },
  });
  vm.runInContext(loader, context, { filename: 'host.js#loadPack' });
  async function load() { await context.loadPack(pack); }
  function replace(generation = false) {
    if (generation === true || generation === 'generation') context.wanted++;
    if (generation === true || generation === 'engine') context.engine = { exports: { ...exports } };
    context.gpu = { device: device(2) };
    for (const row of assets) Object.assign(row, { done: false, fetching: false, failed: false, buffer: null, texture: null });
    Object.assign(pack, { done: false, fetching: false });
  }
  return { assets, context, events, load, replace };
}
(async () => {
  const same = await fixture(); await same.load();
  assert.equal(same.events.fetches, 1); assert.equal(same.events.admitted, 1);
  assert(same.assets.every(row => row.done && !row.failed), JSON.stringify(same.events.errors));
  same.replace(); await same.load();
  assert.equal(same.events.fetches, 1, 'same-session replacement must reuse the already retained source bytes');
  assert.equal(same.events.admitted, 2, 'reuse must pass the Coax SHA gate again');
  assert.deepEqual(same.events.uploads.map(event => event.device), [1, 1, 2, 2], 'new device gets new buffers and uploads');
  assert.equal(same.assets[3].texture.device, 2);
  console.log('PASS same-session recovery: no second download, SHA re-admission, fresh device resources');

  const replaced = await fixture(); await replaced.load(); replaced.replace(true); await replaced.load();
  assert.equal(replaced.events.fetches, 2, 'another engine generation must fetch its own source');
  assert.equal(replaced.events.admitted, 2);
  console.log('PASS replacement engine: no reuse across session ownership');

  for (const changed of ['engine', 'generation']) {
    const changedOwner = await fixture(); await changedOwner.load(); changedOwner.replace(changed); await changedOwner.load();
    assert.equal(changedOwner.events.fetches, 2, `${changed} identity alone must invalidate reuse`);
  }
  console.log('PASS engine and generation identities each independently fence reuse');

  const corrupt = await fixture(); await corrupt.load(); corrupt.assets[0].bytes[0] ^= 128; corrupt.replace(); await corrupt.load();
  assert.equal(corrupt.events.admitted, 1, 'modified retained bytes cannot be re-admitted');
  assert.equal(corrupt.events.fetches, 1, 'corruption must refuse without silently replacing its source');
  assert.equal(corrupt.events.uploads.length, 2, 'refused bytes cannot upload on the new device');
  assert(corrupt.assets.every(row => row.failed));
  console.log('PASS corruption: SHA refusal precedes replacement uploads');

  const privateSession = await fixture({ broker: true }); await privateSession.load(); privateSession.replace(); await privateSession.load();
  assert.equal(privateSession.events.sessions, 2, 'broker authorization must be renewed');
  assert.equal(privateSession.events.fetches, 2);
  console.log('PASS broker packs: retained-source optimization cannot skip authorization');

  const incomplete = await fixture(); const upload = incomplete.context.layerTexture;
  incomplete.context.layerTexture = async () => { throw new Error('fixture upload refusal'); };
  await incomplete.load(); assert(incomplete.assets.every(row => row.failed));
  incomplete.context.layerTexture = upload; incomplete.replace(); await incomplete.load();
  assert.equal(incomplete.events.fetches, 2, 'an incomplete GPU installation cannot seed the retained-source cache');
  assert(incomplete.assets.every(row => row.done && !row.failed));
  console.log('PASS incomplete installation: retry fetches a fresh source');

  const hidden = await fixture(); await hidden.load(); hidden.assets[0].pack.away = true; hidden.replace(); await hidden.load();
  assert.equal(hidden.events.admitted, 1); assert.equal(hidden.events.uploads.length, 2);
  assert(hidden.assets.every(row => row.failed));
  console.log('PASS unavailable private assets: existing refusal precedes reuse');

  const noWeak = await fixture({ weak: false }); await noWeak.load(); noWeak.replace(); await noWeak.load();
  assert.equal(noWeak.events.fetches, 2); assert(noWeak.assets.every(row => row.done && !row.failed));
  console.log('PASS missing WeakRef: ordinary load and recovery remain available');

  const v1 = await fixture(); let delegated = 0;
  v1.context.hostTransport = {}; v1.context.loadPackV1 = async () => { delegated++; };
  await v1.load(); assert.equal(delegated, 1); assert.equal(v1.events.fetches, 0);
  console.log('PASS v1 transport: delegates before any legacy cache effect');
})().catch(error => { console.error(error); process.exitCode = 1; });
