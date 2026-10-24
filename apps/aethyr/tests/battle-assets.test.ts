import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  BATTLE_CLASSES,
  BATTLE_CLIPS,
  loadBattleCrowdAssets,
  validateBattleCrowd,
  validateBattleCrowdBuffers,
  type BattleCrowdManifest,
} from "../packages/renderer/battle-assets.ts";

const hash = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
function fixture(sampleRate: 20 | 60 = 60) {
  const files = new Map<string, Uint8Array<ArrayBuffer>>();
  const blob = (role: string, bytes: Uint8Array<ArrayBuffer>) => {
    const sha256 = hash(bytes),
      url = `/private-assets/battle-${sha256}.${role}.bin`;
    files.set(url, bytes);
    return { url, sha256, byteLength: bytes.length };
  };
  const vertices = new ArrayBuffer(48),
    view = new DataView(vertices);
  for (let i = 0; i < 3; i++) view.setUint32(i * 16 + 8, i, true);
  const positions = new Uint16Array(4 * 8 * 4),
    normals = new Uint16Array(positions.length);
  for (let i = 0; i < normals.length; i += 4) normals[i + 1] = 0x3c00;
  const manifest: BattleCrowdManifest = {
    schema: "aethyrforge.battle-crowd.v1",
    sampleRate,
    classes: BATTLE_CLASSES.map((kind) => ({
      kind,
      height: 1.8,
      vertexCount: 3,
      indexCount: 6,
      vertices: blob("vertices", new Uint8Array(vertices)),
      indices: blob(
        "indices",
        new Uint8Array(new Uint32Array([0, 1, 2, 0, 2, 1]).buffer),
      ),
      lods: [
        { firstIndex: 0, indexCount: 3 },
        { firstIndex: 3, indexCount: 3 },
      ],
      vat: {
        width: 4,
        height: 8,
        positions: blob("positions", new Uint8Array(positions.buffer)),
        normals: blob("normals", new Uint8Array(normals.buffer)),
      },
      materials: {
        alphaMode: "OPAQUE",
        width: 256,
        height: 256,
        layers: 1,
        rgba: blob("rgba", new Uint8Array(256 * 256 * 4).fill(255)),
      },
      clips: Object.fromEntries(
        BATTLE_CLIPS.map((name, i) => [
          name,
          {
            duration: 1 / sampleRate,
            firstFrame: i * 2,
            frameCount: 2,
            loop: name === "idle" || name === "walk",
          },
        ]),
      ) as BattleCrowdManifest["classes"][number]["clips"],
    })),
  };
  const bytes = new TextEncoder().encode(JSON.stringify(manifest)),
    url = `/private-assets/battle-crowd-${hash(bytes)}.json`;
  files.set(url, bytes);
  const attempts = new Map<string, number>();
  const fetcher = async (path: string) => {
    attempts.set(path, (attempts.get(path) ?? 0) + 1);
    const body = files.get(path);
    return body
      ? new Response(body, {
          headers: { "content-length": String(body.length) },
        })
      : new Response(null, { status: 404 });
  };
  return { manifest, files, url, fetcher, attempts };
}

test("four source classes load bounded hashed geometry, VAT, LODs and color maps", async () => {
  const f = fixture(),
    loaded = await loadBattleCrowdAssets(f.url, {
      fetcher: f.fetcher,
      baseURL: "https://battle.test/",
    });
  assert.deepEqual(
    loaded.classes.map((c) => c.kind),
    [...BATTLE_CLASSES],
  );
  assert.equal(loaded.classes[0].vertices.byteLength, 48);
  assert.deepEqual([...loaded.classes[0].indices], [0, 1, 2, 0, 2, 1]);
  assert.equal(loaded.classes[0].vat.positions.length, 128);
  assert.equal(loaded.classes[0].materials.rgba.length, 256 * 256 * 4);
  assert.ok(loaded.bytes > 4 * 256 * 256 * 4);
  const model = loaded.classes[0],
    recipe = f.manifest.classes[0];
  new DataView(model.vertices).setUint32(8, 99999, true);
  assert.throws(() => validateBattleCrowdBuffers(recipe, model));
  new DataView(model.vertices).setUint32(8, 0, true);
  model.vat.positions[0] = 0x7c00;
  assert.throws(
    () => validateBattleCrowdBuffers(recipe, model),
    /invalid pose/,
  );
  model.vat.positions[0] = 0;
  model.materials.rgba[3] = 127;
  assert.throws(() => validateBattleCrowdBuffers(recipe, model), /stay opaque/);
});

test("crowd recipes reject unbounded atlases, missing classes, bad ranges and unpinned URLs", () => {
  const f = fixture();
  for (const mutate of [
    (m: BattleCrowdManifest) => {
      m.classes.pop();
    },
    (m: BattleCrowdManifest) => {
      m.classes[1].kind = "paladin";
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].vat.width = 8192;
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].lods[1].firstIndex = 0;
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].clips.death.firstFrame = 0;
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].clips.attack.duration = 1;
    },
    (m: BattleCrowdManifest) => {
      (m as { sampleRate: number }).sampleRate = 30;
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].materials.layers = 100;
    },
    (m: BattleCrowdManifest) => {
      (m.classes[0].materials as { alphaMode: string }).alphaMode = "BLEND";
    },
    (m: BattleCrowdManifest) => {
      Reflect.deleteProperty(m.classes[0].materials, "alphaMode");
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].vertices.url = "https://untrusted.test/model.bin";
    },
    (m: BattleCrowdManifest) => {
      m.classes[0].height = NaN;
    },
  ]) {
    const copy = structuredClone(f.manifest);
    mutate(copy);
    assert.throws(() => validateBattleCrowd(copy));
  }
});

test("historical 20 Hz assets remain loadable alongside the 60 Hz bake", async () => {
  for (const sampleRate of [20, 60] as const) {
    const f = fixture(sampleRate);
    assert.equal(validateBattleCrowd(f.manifest).sampleRate, sampleRate);
    const loaded = await loadBattleCrowdAssets(f.url, {
      fetcher: f.fetcher,
      baseURL: "https://battle.test/",
    });
    assert.equal(loaded.classes.length, 4);
  }
});

test("corrupt immutable manifests and blobs never become crowd assets", async () => {
  const f = fixture();
  f.files.set(f.url, new TextEncoder().encode("{}"));
  await assert.rejects(
    loadBattleCrowdAssets(f.url, {
      fetcher: f.fetcher,
      baseURL: "https://battle.test/",
    }),
    /incomplete/,
  );
  assert.equal(f.attempts.get(f.url), 3);
  const g = fixture(),
    file = g.manifest.classes[0].vertices;
  g.files.set(file.url, new Uint8Array(file.byteLength).fill(255));
  await assert.rejects(
    loadBattleCrowdAssets(g.url, {
      fetcher: g.fetcher,
      baseURL: "https://battle.test/",
    }),
    /incomplete/,
  );
  assert.equal(g.attempts.get(file.url), 3);
});

test("HTTP failures and oversized streamed documents are rejected before JSON use", async () => {
  const f = fixture();
  await assert.rejects(
    loadBattleCrowdAssets(f.url, {
      fetcher: async () => new Response(null, { status: 404 }),
    }),
    /could not open/,
  );
  await assert.rejects(
    loadBattleCrowdAssets(f.url, {
      fetcher: async () =>
        new Response("{}", { headers: { "content-length": "90000000" } }),
    }),
    /too large/,
  );
  await assert.rejects(
    loadBattleCrowdAssets("/private-assets/battle-crowd-latest.json"),
    /prepared battle/,
  );
});
