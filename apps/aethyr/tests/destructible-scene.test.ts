import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { collisionInstanceIdentity } from "../packages/runtime/static-collision.ts";
import {
  loadDestructibleScene,
  validateDestructibleScene,
  type DestructibleSceneManifest,
} from "../packages/runtime/destructible-scene.ts";
const sha = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
async function fixture() {
  const vertices = new Uint8Array(3 * 48),
    indices = new Uint8Array(new Uint32Array([0, 1, 2]).buffer);
  const scene = {
    vertices: "/private-assets/geometry/vertices.bin",
    indices: "/private-assets/geometry/indices.bin",
    vertexCount: 3,
    indexCount: 3,
    meshes: [
      { name: "stone_wall", bounds: { min: [0, 0, 0], max: [1, 2, 1] } },
    ],
    instances: [
      { mesh: 0, model: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] },
    ],
  } as unknown as CompiledScene & { gameplayURL?: string };
  const manifest: DestructibleSceneManifest = {
    schema: "aethyrforge.gameplay.v1",
    profile: "source-cells-v1",
    source: {
      vertices: scene.vertices,
      indices: scene.indices,
      verticesSha256: sha(vertices),
      indicesSha256: sha(indices),
      instancesSha256: await collisionInstanceIdentity(scene.instances),
    },
    cellSize: [3.5, 3.2, 3.5],
    objects: [
      {
        id: 0,
        sourceId: 0,
        min: [0, 0, 0],
        max: [1, 1, 1],
        hp: 12,
        anchored: true,
        supports: [],
      },
      {
        id: 1,
        sourceId: 0,
        min: [0, 1, 0],
        max: [1, 2, 1],
        hp: 12,
        anchored: false,
        supports: [0],
      },
    ],
  };
  const encoded = new TextEncoder().encode(JSON.stringify(manifest));
  scene.gameplayURL = `/private-assets/gameplay-${sha(encoded)}.json`;
  const fetcher = (async (url: string) =>
    new Response(
      url === scene.gameplayURL
        ? encoded
        : url === scene.vertices
          ? vertices
          : indices,
    )) as typeof fetch;
  return { manifest, scene, fetcher, encoded, vertices };
}
test("destructible loading binds the manifest, rendered geometry, and exact source placements", async () => {
  const { scene, manifest, fetcher } = await fixture();
  const result = await loadDestructibleScene(scene, fetcher);
  assert.deepEqual(result?.objects, manifest.objects);
  const changed = structuredClone(scene);
  changed.instances[0].model[12] = 1;
  await assert.rejects(loadDestructibleScene(changed, fetcher), /placements/);
  assert.equal(
    await loadDestructibleScene({ ...scene, gameplayURL: undefined }, fetcher),
    undefined,
  );
});
test("source geometry recovers from an interrupted, short, or damaged first response", async () => {
  for (const failure of ["stream", "length", "hash"]) {
    const { scene, manifest, fetcher, vertices } = await fixture();
    let reads = 0;
    const unreliable = (async (url: string) => {
      if (url === scene.vertices && ++reads === 1) {
        if (failure === "stream") {
          let pulls = 0;
          return new Response(
            new ReadableStream({
              pull(controller) {
                if (pulls++ === 0) controller.enqueue(vertices.subarray(0, 4));
                else controller.error(new TypeError("NetworkError"));
              },
            }),
          );
        }
        if (failure === "length")
          return new Response(vertices.subarray(0, vertices.length - 1));
        const changed = vertices.slice();
        changed[0] = 1;
        return new Response(changed);
      }
      return fetcher(url);
    }) as typeof fetch;
    const loaded = await loadDestructibleScene(scene, unreliable);
    assert.deepEqual(loaded?.objects, manifest.objects);
    assert.equal(reads, 2, failure + " must retry the complete source file");
  }
});
test("destructible loading rejects changed manifest or geometry bytes", async () => {
  const { scene, fetcher, encoded, vertices } = await fixture();
  await assert.rejects(
    loadDestructibleScene(scene, (async (url: string) =>
      url === scene.gameplayURL
        ? new Response(new Uint8Array(encoded.length))
        : fetcher(url)) as typeof fetch),
    /rules changed/,
  );
  const changed = vertices.slice();
  changed[0] = 1;
  await assert.rejects(
    loadDestructibleScene(scene, (async (url: string) =>
      url === scene.vertices
        ? new Response(changed)
        : fetcher(url)) as typeof fetch),
    /town geometry/,
  );
});
test("destructible manifests reject oversized cells, bad sources, support cycles, and missing foundations", async () => {
  const { scene, manifest } = await fixture();
  for (const change of [
    (m: DestructibleSceneManifest) => {
      m.objects[0].max[0] = 3.6;
    },
    (m: DestructibleSceneManifest) => {
      m.objects[0].sourceId = 1;
    },
    (m: DestructibleSceneManifest) => {
      m.objects[0].hp = 999;
    },
    (m: DestructibleSceneManifest) => {
      m.objects[1].supports = [1];
    },
    (m: DestructibleSceneManifest) => {
      m.objects[0].anchored = false;
      m.objects[0].supports = [1];
    },
    (m: DestructibleSceneManifest) => {
      m.objects[1].min = [10, 10, 10];
      m.objects[1].max = [11, 11, 11];
    },
    (m: DestructibleSceneManifest) => {
      m.source.indices = "https://other.example/model.bin";
    },
  ]) {
    const m = structuredClone(manifest);
    change(m);
    assert.throws(() => validateDestructibleScene(m, scene));
  }
  await assert.rejects(
    loadDestructibleScene({
      ...scene,
      gameplayURL: "/private-assets/../other.json",
    }),
    /unavailable/,
  );
});

test("castle groups name every cell of every castle source and keep a separate support graph", async () => {
  const { scene, manifest } = await fixture();
  scene.meshes[0].name = "SM_castle_wall_01";
  manifest.targetGroups = { castle: [0, 1] };
  assert.equal(validateDestructibleScene(manifest, scene), manifest);
  for (const ids of [[0], [1], [0, 0], [1, 0], [0, 2], [-1, 1]]) {
    const changed = structuredClone(manifest);
    changed.targetGroups = { castle: ids };
    assert.throws(() => validateDestructibleScene(changed, scene), /castle/i);
  }
  const extra = structuredClone(manifest);
  extra.targetGroups = {
    castle: [0, 1],
    town: [0],
  } as typeof extra.targetGroups;
  assert.throws(() => validateDestructibleScene(extra, scene), /castle/i);
  const missingSource = structuredClone(scene);
  missingSource.instances.push(structuredClone(scene.instances[0]));
  assert.throws(
    () => validateDestructibleScene(manifest, missingSource),
    /every cell/i,
  );
  const ordinary = structuredClone(scene);
  ordinary.meshes[0].name = "stone_wall";
  assert.throws(() => validateDestructibleScene(manifest, ordinary), /castle/i);
  const crossing = structuredClone(manifest);
  const mixed = structuredClone(scene);
  mixed.meshes.push({
    name: "stone_wall",
    bounds: { min: [1, 0, 0], max: [2, 2, 1] },
  } as (typeof mixed.meshes)[number]);
  mixed.instances.push({ ...structuredClone(mixed.instances[0]), mesh: 1 });
  crossing.objects.push({
    id: 2,
    sourceId: 1,
    min: [1, 0, 0],
    max: [2, 1, 1],
    hp: 12,
    anchored: true,
    supports: [],
  });
  crossing.objects[1].supports = [2];
  assert.throws(
    () => validateDestructibleScene(crossing, mixed),
    /inside the castle/i,
  );
});

test("castle groups include contained banners but keep terrain and external props separate", async () => {
  const { scene, manifest } = await fixture();
  scene.meshes[0].name = "SM_castle_wall_01";
  for (const [name, min, max] of [
    ["SM_flag_02", [0.1, 0.5, 0.1], [0.9, 1.9, 0.9]],
    ["SM_terrain", [0.1, 0.5, 0.1], [0.9, 1.9, 0.9]],
    ["SM_flag_02", [0.9, 0.5, 0.1], [1.1, 1.9, 0.9]],
  ] as const) {
    scene.instances.push({
      ...structuredClone(scene.instances[0]),
      mesh: scene.meshes.length,
    });
    scene.meshes.push({
      name,
      bounds: { min, max },
    } as unknown as (typeof scene.meshes)[number]);
  }
  manifest.objects.push({
    id: 2,
    sourceId: 1,
    min: [0.1, 0.5, 0.1],
    max: [0.9, 1.9, 0.9],
    hp: 12,
    anchored: false,
    supports: [0],
  });
  manifest.targetGroups = { castle: [0, 1, 2] };
  assert.equal(validateDestructibleScene(manifest, scene), manifest);
  const missing = structuredClone(manifest);
  missing.targetGroups = { castle: [0, 1] };
  assert.throws(() => validateDestructibleScene(missing, scene), /every cell/);
  const moved = structuredClone(scene);
  moved.instances[1].model[12] = 100;
  assert.throws(() => validateDestructibleScene(manifest, moved), /every cell/);
});
