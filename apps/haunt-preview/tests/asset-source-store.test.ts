import assert from "node:assert/strict";
import { test } from "node:test";
import { AssetSourceStore, type AssetStoreBackend } from "../src/asset-source-store.ts";
import { MAX_SOURCE_BYTES, MAX_SOURCE_COUNT } from "../src/asset-import-protocol.ts";

const text = (value: string) => new TextEncoder().encode(value);
class Memory implements AssetStoreBackend {
  data = new Map<string, Uint8Array>();
  writes: string[] = [];
  fail?: string;
  async read(key: string) { return this.data.get(key)?.slice(); }
  async write(key: string, bytes: Uint8Array) {
    this.writes.push(key);
    if (this.fail === key || (this.fail === "object" && key.startsWith("object-"))) {
      this.fail = undefined;
      throw new Error("fixture write failure");
    }
    this.data.set(key, bytes.slice());
  }
  async remove(key: string) { this.data.delete(key); }
}

test("original and accepted sources survive later drafts and reopening", async () => {
  const backend = new Memory(), store = new AssetSourceStore(backend);
  const first = await store.put("mesh", "glb", "models/mesh.glb", text("original"));
  const working = await store.put("mesh", "glb", "models/mesh.glb", text("working"));
  assert.equal(await store.accept("mesh", working.revision, [working.record]), true);
  const rejected = await store.put("mesh", "glb", "models/mesh.glb", text("invalid later draft"));
  const latest = await store.put("mesh", "glb", "models/mesh.glb", text("another invalid draft"));
  assert.equal(latest.revision, 4);
  assert.deepEqual(latest.original, first.record);
  assert.equal(backend.data.has(`object-${rejected.record.hash}`), false);
  const reopened = new AssetSourceStore(backend);
  await reopened.initialize();
  const [saved] = await reopened.list();
  assert.deepEqual(saved.accepted, { revision: 2, sources: [working.record] });
  assert.deepEqual(await reopened.read(saved.original), text("original"));
  assert.deepEqual(await reopened.read(saved.accepted!.sources[0]), text("working"));
  const bytes = await reopened.read(saved.record);
  bytes.fill(0);
  saved.original.path = "changed-by-caller";
  assert.deepEqual(await reopened.read(first.record), text("original"));
  assert.deepEqual(await reopened.read(latest.record), text("another invalid draft"));
  await assert.rejects(reopened.read(rejected.record), /not saved/);
});

test("accept fences the root revision and every dependency version", async () => {
  const store = new AssetSourceStore(new Memory());
  const root = await store.put("mesh", "gltf", "mesh.gltf", text("mesh"));
  const image = await store.put("image", "image", "image.png", text("first image"));
  assert.equal(await store.accept("mesh", 1, [root.record, image.record]), true);
  const newer = await store.put("image", "image", "image.png", text("new image"));
  const before = await store.list();
  assert.equal(await store.accept("mesh", 1, [root.record, image.record]), false);
  assert.deepEqual(await store.list(), before);
  assert.equal(await store.accept("mesh", 1, [root.record, { ...newer.record, path: "other.png" }]), false);
  assert.equal(await store.accept("mesh", 1, [root.record, newer.record]), true);
  const changed = await store.put("mesh", "gltf", "mesh.gltf", text("new mesh"));
  assert.equal(await store.accept("mesh", 1, [changed.record, newer.record]), false);
  await assert.rejects(store.accept("mesh", 2, [newer.record]), /sources/);
  assert.deepEqual(await store.read(root.record), text("mesh"));
});

test("failed object or manifest writes keep the saved state and can be retried", async () => {
  const backend = new Memory(), store = new AssetSourceStore(backend);
  backend.fail = "object";
  await assert.rejects(store.put("mesh", "glb", "mesh.glb", text("first")), /write failure/);
  assert.deepEqual(await store.list(), []);
  const first = await store.put("mesh", "glb", "mesh.glb", text("first"));
  const manifest = backend.data.get("manifest.json")!.slice();
  backend.fail = "manifest.json";
  await assert.rejects(store.put("mesh", "glb", "mesh.glb", text("second")), /write failure/);
  assert.deepEqual(await store.list(), [first]);
  assert.deepEqual(backend.data.get("manifest.json"), manifest);
  const reopened = new AssetSourceStore(backend);
  assert.deepEqual(await reopened.list(), [first]);
  assert.deepEqual(await reopened.read(first.record), text("first"));
  const second = await store.put("mesh", "glb", "mesh.glb", text("second"));
  assert.equal(second.revision, 2);
  backend.fail = "manifest.json";
  await assert.rejects(store.accept("mesh", 2, [second.record]), /write failure/);
  assert.equal((await store.list())[0].accepted, undefined);
  assert.equal(await store.accept("mesh", 2, [second.record]), true);
});

test("queued puts own their bytes and publish monotonically", async () => {
  const backend = new Memory(), store = new AssetSourceStore(backend);
  const mutable = text("one");
  const first = store.put("mesh", "glb", "mesh.glb", mutable);
  mutable.fill(0);
  const second = store.put("mesh", "glb", "mesh.glb", text("two"));
  const [a, b] = await Promise.all([first, second]);
  assert.equal(a.revision, 1);
  assert.equal(b.revision, 2);
  assert.deepEqual(await store.read(a.original), text("one"));
  assert.deepEqual(await store.read(b.record), text("two"));
  assert.equal((await store.put("mesh", "glb", "mesh.glb", text("two"))).revision, 2);
});

test("per-source, source-count, and original-version byte limits reject before publishing", async () => {
  const backend = new Memory(), store = new AssetSourceStore(backend);
  await assert.rejects(store.put("large", "glb", "large.glb", new Uint8Array(MAX_SOURCE_BYTES + 1)), /large/);
  for (let i = 0; i < MAX_SOURCE_COUNT; i++) {
    await store.put(`id-${i}`, "image", `${i}.png`, text("deduplicated"));
  }
  const writes = backend.writes.length;
  await assert.rejects(store.put("extra", "image", "extra.png", text("x")), /too many/);
  assert.equal(backend.writes.length, writes);
  assert.equal((await store.list()).length, MAX_SOURCE_COUNT);
  assert.equal([...backend.data.keys()].filter((k) => k.startsWith("object-")).length, 1);

  const bytesBackend = new Memory(), bytesStore = new AssetSourceStore(bytesBackend);
  for (let i = 0; i < 4; i++) {
    await bytesStore.put(`large-${i}`, "glb", `${i}.glb`, new Uint8Array(MAX_SOURCE_BYTES).fill(i));
  }
  const before = await bytesStore.list(), count = bytesBackend.writes.length;
  await assert.rejects(bytesStore.put("large-0", "glb", "0.glb", new Uint8Array(MAX_SOURCE_BYTES).fill(4)), /full/);
  assert.deepEqual(await bytesStore.list(), before);
  assert.equal(bytesBackend.writes.length, count);
});

test("paths, forged records, and corrupt bytes cannot cross the saved source boundary", async () => {
  const backend = new Memory(), store = new AssetSourceStore(backend);
  for (const path of ["../mesh.glb", "/mesh.glb", "//server/a", "https://host/a", "a\\b", "a?b", "a#b"]) {
    await assert.rejects(store.put("mesh", "glb", path, text("mesh")));
  }
  const saved = await store.put("mesh", "glb", "models/../mesh.glb", text("mesh"));
  assert.equal(saved.record.path, "mesh.glb");
  await assert.rejects(store.put(undefined as unknown as string, "glb", "mesh.glb", text("mesh")), /invalid/);
  await assert.rejects(store.read({ ...saved.record, path: "other.glb" }), /not saved/);
  backend.data.get(`object-${saved.record.hash}`)![0] ^= 1;
  await assert.rejects(store.read(saved.record), /changed/);
  await assert.rejects(store.accept("mesh", 1, [saved.record]), /changed/);
  assert.equal((await store.list())[0].accepted, undefined);
  backend.data.delete(`object-${saved.record.hash}`);
  await assert.rejects(store.read(saved.record), /missing/);
});

test("malformed saved metadata is rejected without replacing a good saved list", async () => {
  const backend = new Memory();
  backend.data.set("manifest.json", text('{"version":99,"sources":[]}'));
  const store = new AssetSourceStore(backend);
  await assert.rejects(store.initialize(), /unsupported/);
  backend.data.set("manifest.json", text('{"version":1,"sources":[]}'));
  await store.initialize();
  assert.deepEqual(await store.list(), []);
  const saved = await store.put("mesh", "glb", "mesh.glb", text("mesh"));
  const malformed = { version: 1, sources: [{ ...saved, accepted: null }] };
  backend.data.set("manifest.json", text(JSON.stringify(malformed)));
  await assert.rejects(new AssetSourceStore(backend).initialize(), /invalid/);
});

test("OPFS storage opens isolated project directories without real browser storage", async () => {
  class Directory {
    directories = new Map<string, Directory>();
    files = new Map<string, Uint8Array>();
    async getDirectoryHandle(name: string) {
      let dir = this.directories.get(name);
      if (!dir) { dir = new Directory(); this.directories.set(name, dir); }
      return dir;
    }
    async getFileHandle(name: string, options?: { create?: boolean }) {
      if (!this.files.has(name) && !options?.create) throw new DOMException("missing", "NotFoundError");
      return {
        getFile: async () => new File([this.files.get(name)!.slice().buffer], name),
        createWritable: async () => {
          let pending: Uint8Array;
          return {
            write: async (bytes: ArrayBuffer) => { pending = new Uint8Array(bytes).slice(); },
            close: async () => { this.files.set(name, pending); },
            abort: async () => {},
          };
        },
      };
    }
    async removeEntry(name: string) { this.files.delete(name); }
  }
  const original = Object.getOwnPropertyDescriptor(globalThis, "navigator");
  const directory = new Directory();
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: { storage: { getDirectory: async () => directory } } });
  try {
    for (const id of ["..", "a/b", "https:host", "/absolute"]) await assert.rejects(AssetSourceStore.open(id), /valid project/);
    await assert.rejects(AssetSourceStore.open(undefined as unknown as string), /valid project/);
    const haunt = await AssetSourceStore.open("haunt-webgpu-v1");
    const town = await AssetSourceStore.open("medieval-town-v1");
    const a = await haunt.put("mesh", "glb", "mesh.glb", text("haunt"));
    await town.put("mesh", "glb", "mesh.glb", text("town"));
    await assert.rejects(town.read(a.record), /not saved/);
    assert.deepEqual(await (await AssetSourceStore.open("haunt-webgpu-v1")).read(a.record), text("haunt"));
    assert.deepEqual(await haunt.read(a.record), text("haunt"));
  } finally {
    if (original) Object.defineProperty(globalThis, "navigator", original);
    else Reflect.deleteProperty(globalThis, "navigator");
  }
});
