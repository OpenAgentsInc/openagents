import assert from "node:assert/strict";
import { test } from "node:test";
import { AssetImportClient, AssetImportSession, type AssetWorkerPort } from "../src/asset-import-client.ts";
import { AssetWorkerEngine } from "../src/asset-import-worker.ts";
import { AssetSourceStore, type AssetStoreBackend } from "../src/asset-source-store.ts";
import { copyImportedAsset, descriptionTransfers, sourceKey } from "../src/asset-import-protocol.ts";
import type { AssetConverter, AssetImportRequest, AssetImportResponse } from "../src/asset-import-protocol.ts";
import type { AssetDescription, SourceRecord } from "../src/asset-compiler.ts";
import type { AssetRuntime, PreparedAsset } from "../src/asset-runtime-adapter.ts";

const text = (value: string) => new TextEncoder().encode(value);
function gate() {
  let release!: () => void;
  const promise = new Promise<void>(resolve => { release = resolve; });
  return { promise, release };
}
async function until(predicate: () => boolean) {
  for (let i = 0; i < 200; i++) {
    if (predicate()) return;
    await new Promise<void>(resolve => setImmediate(resolve));
  }
  assert.fail("fixture did not reach the expected state");
}
function triangle(source: SourceRecord, dependencies: SourceRecord[] = []): AssetDescription {
  return { id: source.id, sources: [source, ...dependencies], diagnostics: [], images: [], meshes: [{
    id: `${source.id}-mesh`, source: source.id, positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
    indices: new Uint32Array([0, 1, 2]), vertexCount: 3, indexCount: 3, bounds: { min: [0, 0, 0], max: [1, 1, 0] },
  }] };
}
const convert: AssetConverter = async (source, bytes, files) => {
  if (new TextDecoder().decode(bytes).startsWith("invalid")) throw new Error("fixture invalid asset");
  return triangle(source, source.id === "dependent" ? [files.find(file => file.record.id === "dep")!.record] : []);
};

class Memory implements AssetStoreBackend {
  data = new Map<string, Uint8Array>();
  beforeWrite?: (key: string, bytes: Uint8Array) => Promise<void>;
  async read(key: string) { return this.data.get(key)?.slice(); }
  async write(key: string, bytes: Uint8Array) {
    await this.beforeWrite?.(key, bytes);
    this.data.set(key, bytes.slice());
  }
  async remove(key: string) { this.data.delete(key); }
}
class Port implements AssetWorkerPort {
  onmessage: AssetWorkerPort["onmessage"] = null;
  onerror: AssetWorkerPort["onerror"] = null;
  onmessageerror: AssetWorkerPort["onmessageerror"] = null;
  engine: AssetWorkerEngine;
  requests: AssetImportRequest[] = [];
  terminated = false;
  hold = false;
  queued: AssetImportResponse[] = [];
  evictNext: string[] = [];
  constructor(converter: AssetConverter = convert) { this.engine = new AssetWorkerEngine(converter); }
  postMessage(message: AssetImportRequest, transfer: Transferable[]) {
    assert.equal(this.terminated, false);
    const received = structuredClone(message, { transfer });
    this.requests.push(received);
    void this.engine.handle(received).then(response => {
      if (response.type === "stored") { response.evicted.push(...this.evictNext); this.evictNext = []; }
      const copy = structuredClone(response, { transfer: response.type === "compiled" ? descriptionTransfers(response.description) : [] });
      if (this.hold) this.queued.push(copy); else this.emit(copy);
    });
  }
  emit(response: AssetImportResponse) { this.onmessage?.({ data: response } as MessageEvent<AssetImportResponse>); }
  flush() { this.hold = false; for (const response of this.queued.splice(0)) this.emit(response); }
  terminate() { this.terminated = true; }
}
type Prepared = PreparedAsset & { disposed: boolean };
class Runtime implements AssetRuntime {
  selected = new Map<string, { revision: number; asset: Prepared }>();
  prepared: Prepared[] = [];
  commits: string[] = [];
  closed = false;
  beforePrepare?: (asset: Prepared) => Promise<void>;
  async prepare(description: AssetDescription): Promise<Prepared> {
    const asset: Prepared = { description: copyImportedAsset(description), meshes: [], images: [], disposed: false,
      dispose() { this.disposed = true; } };
    this.prepared.push(asset);
    await this.beforePrepare?.(asset);
    return asset;
  }
  commit(id: string, revision: number, asset: Prepared) {
    assert.equal(this.closed, false);
    assert.equal(asset.disposed, false);
    this.selected.get(id)?.asset.dispose();
    this.selected.set(id, { revision, asset });
    this.commits.push(id);
  }
  get(id: string) {
    const selected = this.selected.get(id);
    return selected && { revision: selected.revision, description: copyImportedAsset(selected.asset.description) };
  }
  dispose() { this.closed = true; for (const asset of this.prepared) asset.dispose(); this.selected.clear(); }
}
function setup(converter: AssetConverter = convert, backend = new Memory()) {
  const store = new AssetSourceStore(backend), port = new Port(converter), client = new AssetImportClient(port), runtime = new Runtime();
  return { backend, store, port, client, runtime, session: new AssetImportSession(store, client, runtime) };
}

test("worker client transfers owned uploads, reuses cache, and honors evicted source keys", async t => {
  const { store, port, client } = setup(); t.after(() => client.dispose());
  const a = await store.put("a", "glb", "a.glb", text("a"));
  const bytes = await store.read(a.record), input = { record: a.record, bytes };
  const first = await client.compile(input, [], 1);
  assert.equal(first.metrics.uploads, 1);
  assert.equal(first.metrics.uploadedBytes, 1);
  assert.deepEqual(bytes, text("a"));
  assert.deepEqual(await store.read(a.record), text("a"));
  first.description.meshes[0].positions.fill(9);
  const cached = await client.compile(input, [], 2);
  assert.equal(cached.metrics.cacheHit, true);
  assert.equal(cached.metrics.uploads, 0);
  assert.equal(cached.description.meshes[0].positions[0], 0);
  const b = await store.put("b", "glb", "b.glb", text("b"));
  port.evictNext = [sourceKey(a.record)];
  const combined = await client.compile(input, [{ record: b.record, bytes: await store.read(b.record) }], 3);
  assert.equal(combined.metrics.uploads, 2, "refills an input evicted by a later upload in the same request");
  await client.compile({ record: b.record, bytes: await store.read(b.record) }, [], 1);
  assert.equal((await client.compile(input, [], 4)).metrics.uploads, 0);
  assert.equal((await client.stats()).conversions, 2);
});

test("pending limits, cancellation, fatal errors, and timeout release every request", async () => {
  const port = new Port(); port.hold = true;
  const client = new AssetImportClient(port, { maxPending: 1 });
  const pending = client.stats();
  await assert.rejects(client.stats(), /current imports/);
  port.onerror?.({} as ErrorEvent);
  await assert.rejects(pending, /worker stopped/);
  assert.equal(port.terminated, true);
  port.flush();
  const timeoutPort = new Port(); timeoutPort.hold = true;
  const timeout = new AssetImportClient(timeoutPort, { timeoutMs: 10 });
  await assert.rejects(timeout.stats(), /too long/);
  assert.equal(timeoutPort.terminated, true);

  const slow = gate(), entered = gate();
  const fixture = setup(async source => { entered.release(); await slow.promise; return triangle(source); });
  const abort = new AbortController();
  const operation = fixture.session.importAsset("mesh", "glb", "mesh.glb", text("one"), { signal: abort.signal });
  await entered.promise; abort.abort();
  assert.equal((await operation).type, "cancelled");
  assert.ok(fixture.port.requests.some(request => request.type === "cancel"));
  slow.release(); await until(() => fixture.port.engine.stats().active === 0);
  assert.equal(fixture.runtime.commits.length, 0);
  fixture.session.dispose();
});

test("invalid conversion and failed accept keep the previous runtime and allow same-source retry", async t => {
  const f = setup(); t.after(() => f.session.dispose());
  const mutable = text("original");
  const pending = f.session.importAsset("mesh", "glb", "mesh.glb", mutable);
  mutable.fill(0);
  const original = await pending;
  assert.equal(original.type, "accepted");
  const selected = f.runtime.selected.get("mesh")!.asset;
  assert.deepEqual(await f.store.read((await f.store.list())[0].original), text("original"));
  assert.equal((await f.session.importAsset("mesh", "glb", "mesh.glb", text("invalid draft"))).type, "failed");
  assert.equal(f.runtime.selected.get("mesh")!.asset, selected);
  f.runtime.beforePrepare = async () => { f.backend.beforeWrite = async key => {
    if (key === "manifest.json") { f.backend.beforeWrite = undefined; throw new Error("fixture write failed"); }
  }; };
  const failed = await f.session.importAsset("mesh", "glb", "mesh.glb", text("working"));
  assert.equal(failed.type, "failed");
  assert.ok(failed.type === "failed" && /write failed/.test(failed.message));
  assert.equal(f.runtime.selected.get("mesh")!.asset, selected);
  assert.equal(f.runtime.prepared.at(-1)!.disposed, true);
  assert.equal((await f.store.list())[0].accepted!.revision, 1);
  f.runtime.beforePrepare = undefined;
  const retry = await f.session.importAsset("mesh", "glb", "mesh.glb", text("working"));
  assert.equal(retry.type, "accepted");
  assert.ok(retry.type === "accepted" && retry.metrics.cacheHit);
  assert.equal(selected.disposed, true);
});

test("a decoder diagnostic names the source once and preserves the selected asset", async t => {
  const f = setup(async (record, bytes) => new TextDecoder().decode(bytes) === "bad" ? {
    id: record.id, sources: [record], meshes: [], images: [],
    diagnostics: [{ code: "decode", severity: "error", source: record.id, message: "This image is truncated." }],
  } : triangle(record));
  t.after(() => f.session.dispose());
  await f.session.importAsset("mesh", "image", "cards/paper.png", text("good"));
  const previous = f.runtime.selected.get("mesh")!.asset;
  const failed = await f.session.importAsset("mesh", "image", "cards/paper.png", text("bad"));
  assert.equal(failed.type, "failed");
  assert.ok(failed.type === "failed" && failed.message === "cards/paper.png: This image is truncated.");
  assert.equal(f.runtime.selected.get("mesh")!.asset, previous);
});

test("late candidates and changed dependency pins cannot replace accepted resources", async t => {
  const f = setup(); t.after(() => f.session.dispose());
  await f.session.importAsset("mesh", "glb", "mesh.glb", text("original"));
  const slow = gate(), entered = gate();
  f.runtime.beforePrepare = async asset => {
    if (asset.description.sources[0].bytes === 4) { entered.release(); await slow.promise; }
  };
  const older = f.session.importAsset("mesh", "glb", "mesh.glb", text("slow"));
  await entered.promise;
  assert.equal((await f.session.importAsset("mesh", "glb", "mesh.glb", text("newer"))).type, "accepted");
  const newest = f.runtime.selected.get("mesh")!.asset;
  slow.release();
  assert.equal((await older).type, "stale");
  assert.equal(f.runtime.selected.get("mesh")!.asset, newest);
  assert.equal(f.runtime.prepared[1].disposed, true);

  await f.session.importAsset("dep", "image", "dep.png", text("old dependency"));
  const dependencyGate = gate(), dependencyReady = gate();
  f.runtime.beforePrepare = async asset => { if (asset.description.id === "dependent") { dependencyReady.release(); await dependencyGate.promise; } };
  const dependent = f.session.importAsset("dependent", "gltf", "dependent.gltf", text("root"));
  await dependencyReady.promise;
  await f.store.put("dep", "image", "dep.png", text("changed dependency"));
  dependencyGate.release();
  assert.equal((await dependent).type, "stale");
  assert.equal(f.runtime.get("dependent"), undefined);
  assert.equal(f.runtime.prepared.at(-1)!.disposed, true);
});

test("dependency edits rebuild only accepted users and retain unrelated resources", async t => {
  const f = setup(); t.after(() => f.session.dispose());
  await f.session.importAsset("dep", "image", "dep.png", text("old"));
  await f.session.importAsset("dependent", "gltf", "dependent.gltf", text("root"));
  await f.session.importAsset("unrelated", "glb", "unrelated.glb", text("other"));
  const unrelated = f.runtime.selected.get("unrelated")!.asset;
  const dependent = f.runtime.selected.get("dependent")!.asset;
  const result = await f.session.importAsset("dep", "image", "dep.png", text("new"));
  assert.equal(result.type, "accepted");
  assert.deepEqual(result.rebuilds?.map(outcome => [outcome.id, outcome.type]), [["dependent", "accepted"]]);
  assert.equal(f.runtime.selected.get("unrelated")!.asset, unrelated);
  assert.equal(unrelated.disposed, false);
  assert.equal(dependent.disposed, true);
  const sources = await f.store.list(), pin = sources.find(source => source.record.id === "dep")!.record;
  assert.deepEqual(f.runtime.get("dependent")!.description.sources.find(source => source.id === "dep"), pin);
});

test("source-only edits rebuild their accepted users without compiling a palette as a model", async t => {
  const calls: string[] = [];
  const f = setup(async (record, bytes, files) => {
    calls.push(record.id);
    assert.notEqual(record.id, "palette", "a source-only file must not become a standalone model");
    const palette = files.find(file => file.record.id === "palette");
    if (record.id === "house" && palette && new TextDecoder().decode(palette.bytes).startsWith("invalid")) {
      throw new Error("palettes/paper.json: invalid color");
    }
    return triangle(record, record.id === "house" ? [palette!.record] : []);
  });
  t.after(() => f.session.dispose());
  await f.store.put("palette", "gltf", "palettes/paper.json", text("warm"));
  assert.equal((await f.session.importAsset("house", "gltf", "house.gltf", text("geometry"))).type, "accepted");
  assert.equal((await f.session.importAsset("other", "glb", "other.glb", text("unrelated"))).type, "accepted");
  const other = f.runtime.selected.get("other")!.asset;
  const first = f.runtime.selected.get("house")!.asset;
  const changed = await f.session.updateSource("palette", "gltf", "palettes/paper.json", text("cool"));
  assert.equal(changed.type, "stored");
  assert.ok(changed.type === "stored" && changed.rebuilds.length === 1 && changed.rebuilds[0].type === "accepted");
  assert.deepEqual(calls, ["house", "other", "house"]);
  assert.equal(first.disposed, true);
  assert.equal(f.runtime.selected.get("other")!.asset, other);
  const lastGood = f.runtime.selected.get("house")!.asset;
  const failed = await f.session.updateSource("palette", "gltf", "palettes/paper.json", text("invalid palette"));
  assert.equal(failed.type, "stored");
  assert.ok(failed.type === "stored" && failed.rebuilds[0].type === "failed" && failed.rebuilds[0].message.includes("palettes/paper.json"));
  assert.equal(f.runtime.selected.get("house")!.asset, lastGood);
  const savedPalette = (await f.store.list()).find(source => source.record.id === "palette")!;
  assert.deepEqual(await f.store.read(savedPalette.original), text("warm"));
});

test("reopening restores exact accepted pins rather than the latest invalid draft", async t => {
  const f = setup();
  await f.session.importAsset("dep", "image", "dep.png", text("original dep"));
  await f.session.importAsset("dependent", "gltf", "dependent.gltf", text("original root"));
  await f.session.importAsset("dependent", "gltf", "dependent.gltf", text("working root"));
  const accepted = f.runtime.get("dependent")!;
  assert.equal((await f.session.importAsset("dependent", "gltf", "dependent.gltf", text("invalid latest"))).type, "failed");
  const before = await f.store.list();
  f.session.dispose();
  const reopened = setup(convert, f.backend); t.after(() => reopened.session.dispose());
  const restored = await reopened.session.restore();
  assert.deepEqual(restored.map(outcome => [outcome.id, outcome.type]), [["dep", "accepted"], ["dependent", "accepted"]]);
  assert.deepEqual(reopened.runtime.get("dependent"), accepted);
  assert.deepEqual(await reopened.store.list(), before);
  assert.deepEqual(await reopened.store.read(before.find(source => source.record.id === "dependent")!.original), text("original root"));
});

test("cancel and dispose release prepared candidates before accepting", async () => {
  for (const dispose of [false, true]) {
    const f = setup(), ready = gate(), release = gate();
    await f.session.importAsset("mesh", "glb", "mesh.glb", text("good"));
    const before = (await f.store.list())[0].accepted;
    f.runtime.beforePrepare = async () => { ready.release(); await release.promise; };
    const pending = f.session.importAsset("mesh", "glb", "mesh.glb", text("candidate"));
    await ready.promise;
    if (dispose) f.session.dispose(); else f.session.cancel("mesh");
    release.release();
    assert.equal((await pending).type, "cancelled");
    assert.deepEqual((await f.store.list())[0].accepted, before);
    assert.equal(f.runtime.prepared.at(-1)!.disposed, true);
    assert.equal(f.port.terminated, dispose);
    f.session.dispose();
  }
});

test("once acceptance begins cancellation cannot persist a hidden accepted candidate", async () => {
  for (const dispose of [false, true]) {
    const f = setup(), writing = gate(), release = gate(), abort = new AbortController();
    f.runtime.beforePrepare = async () => { f.backend.beforeWrite = async key => {
      if (key === "manifest.json") { writing.release(); await release.promise; }
    }; };
    const pending = f.session.importAsset("mesh", "glb", "mesh.glb", text("good"), { signal: abort.signal });
    await writing.promise;
    abort.abort(); f.session.cancel("mesh");
    if (dispose) f.session.dispose();
    assert.equal(f.runtime.closed, false);
    release.release();
    assert.equal((await pending).type, "accepted");
    assert.deepEqual(f.runtime.commits, ["mesh"]);
    assert.equal((await f.store.list())[0].accepted!.revision, 1);
    assert.equal(f.runtime.closed, dispose);
    f.session.dispose();
  }
});
