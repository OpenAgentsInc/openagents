import { test } from "node:test";
import assert from "node:assert/strict";
import { describeSource } from "../src/asset-compiler.ts";
import type { AssetDescription, SourceRecord } from "../src/asset-compiler.ts";
import { AssetWorkerEngine } from "../src/asset-import-worker.ts";
import { assetPath, descriptionTransfers, sourceKey } from "../src/asset-import-protocol.ts";

async function source(id: string, value = 1) {
  const bytes = new Uint8Array([value, 0xff, 0]);
  return { record: await describeSource(id, "glb", `${id}.glb`, bytes), bytes };
}

function triangle(record: SourceRecord, dependencies: SourceRecord[] = []): AssetDescription {
  return { id: record.id, sources: [record, ...dependencies], images: [], diagnostics: [], meshes: [{
    id: `${record.id}-mesh`, source: record.id, positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
    indices: new Uint32Array([0, 1, 2]), vertexCount: 3, indexCount: 3, bounds: { min: [0, 0, 0], max: [1, 1, 0] },
  }] };
}

test("worker verifies exact binary inputs and owns cached buffers after transfer", async () => {
  const input = await source("mesh");
  const engine = new AssetWorkerEngine(async record => triangle(record));
  assert.equal((await engine.handle({ type: "put", requestId: "put", source: input })).type, "stored");
  const first = await engine.handle({ type: "compile", requestId: "a", revision: 1, source: input.record, files: [] });
  assert.equal(first.type, "compiled");
  if (first.type !== "compiled") return;
  assert.equal(first.version, 1);
  assert.equal(first.metrics.outputBytes, 48);
  structuredClone(first.description, { transfer: descriptionTransfers(first.description) });
  assert.equal(first.description.meshes[0].positions.byteLength, 0);
  const second = await engine.handle({ type: "compile", requestId: "b", revision: 2, source: input.record, files: [] });
  assert.equal(second.type, "compiled");
  if (second.type !== "compiled") return;
  assert.equal(second.metrics.cacheHit, true);
  assert.equal(second.description.meshes[0].positions[3], 1);
  assert.equal(engine.stats().conversions, 1);
  const changed = { ...input, bytes: new Uint8Array([9, 0xff, 0]) };
  assert.equal((await engine.handle({ type: "put", requestId: "bad", source: changed })).type, "error");
  assert.equal(engine.stats().conversions, 1);
});

test("cache tracks used dependencies and ignores unrelated source edits", async () => {
  const root = await source("root"), dependency = await source("dependency"), unrelated = await source("other");
  const engine = new AssetWorkerEngine(async (record, _bytes, files) => triangle(record, [files.find(file => file.record.id === "dependency")!.record]));
  for (const input of [root, dependency, unrelated]) await engine.handle({ type: "put", requestId: input.record.id, source: input });
  const compile = (id: string, records: SourceRecord[]) => engine.handle({ type: "compile", requestId: id, revision: 1, source: root.record, files: records });
  assert.equal((await compile("one", [dependency.record, unrelated.record])).type, "compiled");
  const other2 = await source("other", 2);
  await engine.handle({ type: "put", requestId: "put-other", source: other2 });
  const unchanged = await compile("two", [dependency.record, other2.record]);
  assert.equal(unchanged.type === "compiled" && unchanged.metrics.cacheHit, true);
  const dependency2 = await source("dependency", 3);
  await engine.handle({ type: "put", requestId: "put-dep", source: dependency2 });
  const changed = await compile("three", [dependency2.record, other2.record]);
  assert.equal(changed.type === "compiled" && changed.metrics.cacheHit, false);
  assert.equal(engine.stats().conversions, 2);
});

test("cancellation discards an in-flight conversion and releases its job", async () => {
  const input = await source("cancel");
  let resolve!: (description: AssetDescription) => void;
  const engine = new AssetWorkerEngine(() => new Promise(done => { resolve = done; }));
  await engine.handle({ type: "put", requestId: "put", source: input });
  const pending = engine.handle({ type: "compile", requestId: "slow", revision: 1, source: input.record, files: [] });
  assert.equal(engine.stats().active, 1);
  await engine.handle({ type: "cancel", requestId: "cancel-command", target: "slow" });
  resolve(triangle(input.record));
  assert.equal((await pending).type, "cancelled");
  assert.equal(engine.stats().active, 0);
  assert.equal(engine.stats().cacheEntries, 0);
});

test("failed conversions and malformed output do not replace valid cache entries", async () => {
  const good = await source("good"), bad = await source("bad");
  const engine = new AssetWorkerEngine(async record => {
    const description = triangle(record);
    if (record.id === "bad") description.meshes[0].indices[2] = 99;
    return description;
  });
  for (const input of [good, bad]) await engine.handle({ type: "put", requestId: input.record.id, source: input });
  assert.equal((await engine.handle({ type: "compile", requestId: "good-job", revision: 1, source: good.record, files: [] })).type, "compiled");
  const rejected = await engine.handle({ type: "compile", requestId: "bad-job", revision: 1, source: bad.record, files: [] });
  assert.equal(rejected.type, "error");
  assert.equal(rejected.type === "error" && rejected.source, "bad.glb");
  assert.equal(engine.stats().cacheEntries, 1);
  assert.equal(engine.stats().active, 0);
});

test("cache eviction is bounded by actual packed buffer bytes", async () => {
  const engine = new AssetWorkerEngine(async record => triangle(record), 48);
  for (const id of ["a", "b", "c"]) {
    const input = await source(id);
    await engine.handle({ type: "put", requestId: `${id}-put`, source: input });
    await engine.handle({ type: "compile", requestId: id, revision: 1, source: input.record, files: [] });
  }
  assert.equal(engine.stats().cacheBytes, 48);
  assert.equal(engine.stats().cacheEntries, 1);
  assert.equal(engine.stats().conversions, 3);
});

test("converter mutation cannot alter retained source bytes", async () => {
  const input = await source("immutable", 7);
  const engine = new AssetWorkerEngine(async (record, bytes) => {
    assert.equal(bytes[0], 7);
    bytes[0] = 99;
    return triangle(record);
  }, 1);
  await engine.handle({ type: "put", requestId: "put", source: input });
  for (const id of ["first", "second"]) assert.equal((await engine.handle({ type: "compile", requestId: id, revision: 1, source: input.record, files: [] })).type, "compiled");
  assert.equal(engine.stats().conversions, 2);
});

test("asset paths stay inside the project and hashes remain typed", () => {
  assert.equal(assetPath("meshes/../textures/a.png"), "textures/a.png");
  for (const path of ["../a", "/a", "https://example.com/a", "a\\b", "a\0b"]) assert.throws(() => assetPath(path));
  assert.throws(() => sourceKey({ id: "id", kind: "glb", path: "a.glb", bytes: 1, hash: "wrong" }));
});
