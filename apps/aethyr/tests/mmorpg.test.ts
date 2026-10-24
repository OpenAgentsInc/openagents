import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { AssetIntegrityError } from "../packages/assets/src/asset-download.ts";
import { Renderer } from "../packages/renderer/renderer.ts";
import { prepareSourceWind } from "../packages/renderer/wind-source.ts";
import { sourceDraws } from "../packages/renderer/source.ts";
import { createResidentSourcePreparation } from "../packages/renderer/resident-source.ts";
import {
  validateMmorpgActors,
  validateMmorpgWorld,
  loadMmorpgWorld,
  loadMmorpgDocument,
  type MmorpgActorPose,
  type MmorpgWorldManifest,
} from "../packages/assets/src/mmorpg.ts";
import {
  mmorpgSceneryCenter,
  mmorpgSceneryPlacements,
  mmorpgSceneryInstances,
  mmorpgSceneBatches,
} from "../packages/renderer/mmorpg-scenery.ts";
const actor = (id = 1): MmorpgActorPose => ({
  id,
  class: "paladin",
  name: "Player",
  position: [-10, 1, 0],
  yaw: 0,
  state: "idle",
  actionTime: 0,
  animationTime: 0,
  health: 200,
  maxHealth: 200,
  alive: true,
  weapon: "weapon.sword1",
});
const world = (): MmorpgWorldManifest => ({
  schema: "mmorpg-world.v1",
  worldScale: 1 / 6,
  seed: 1,
  sourceRepository: "https://github.com/simondevyoutube/Quick_3D_MMORPG",
  sourceCommit: "0".repeat(40),
  characters: Object.fromEntries(
    ["paladin", "sorceror", "zombie", "warrok"].map((k) => [
      k,
      "/private-assets/" + k + ".json",
    ]),
  ) as MmorpgWorldManifest["characters"],
  classClips: Object.fromEntries(
    ["paladin", "sorceror", "zombie", "warrok"].map((k) => [
      k,
      { idle: 1, walk: 1, attack: 1, death: 1 },
    ]),
  ) as MmorpgWorldManifest["classClips"],
  uiURL: `/private-assets/mmorpg-ui-${"0".repeat(64)}.json`,
  icons: Object.fromEntries(
    Array.from({ length: 9 }, (_, i) => [
      "i" + i,
      {
        url: "/private-assets/i.png",
        sha256: "0".repeat(64),
        byteLength: 4,
        width: 1,
        height: 1,
      },
    ]),
  ),
  particleTextures: Object.fromEntries(
    ["blood", "fire", "levelUp"].map((k) => [
      k,
      {
        url: "/private-assets/p.png",
        sha256: "0".repeat(64),
        byteLength: 4,
        width: 1,
        height: 1,
      },
    ]),
  ) as MmorpgWorldManifest["particleTextures"],
  scenery: {
    vertices: {
      url: "/private-assets/v.bin",
      sha256: "0".repeat(64),
      byteLength: 108 * 48,
    },
    indices: {
      url: "/private-assets/i.bin",
      sha256: "0".repeat(64),
      byteLength: 108 * 4,
    },
    vertexCount: 108,
    indexCount: 108,
    vertexStride: 48,
    meshes: Array.from({ length: 36 }, (_, i) => ({
      id: "m" + i,
      name: "m" + i,
      bounds: { min: [0, 0, 0], max: [1, 1, 1] },
      primitives: [
        {
          firstIndex: i * 3,
          indexCount: 3,
          baseVertex: 0,
          material: 0,
          materialSlot: 0,
        },
      ],
    })),
    materials: [],
    catalog: Array.from({ length: 36 }, (_, i) => ({
      id: "kind" + i,
      file: "source.fbx",
      scale: 0.01,
      biomes: ["forest", "arid", "desert"],
      collidable: i < 18,
    })),
    placements: [],
    windowCells: 21,
    cellMetres: 50 / 6,
    seed: 2,
  },
  simulation: { seed: 1, worldScale: 1 / 6, spawn: [-10, 1, 0], yaw: 0 },
});
test("MMORPG actors are bounded and validated as one atomic submission", () => {
  validateMmorpgActors(Array.from({ length: 64 }, (_, i) => actor(i)));
  assert.throws(
    () => validateMmorpgActors(Array.from({ length: 65 }, (_, i) => actor(i))),
    /64/,
  );
  assert.throws(() => validateMmorpgActors([actor(), actor()]), /complete/);
  for (const patch of [
    { position: [NaN, 0, 0] },
    { health: 201 },
    { alive: false },
    { class: "guard" },
    { state: "jump" },
    { weapon: "weapon.bow1" },
    { actionTime: Infinity },
    { name: "x".repeat(65) },
  ])
    assert.throws(
      () => validateMmorpgActors([{ ...actor(), ...patch } as MmorpgActorPose]),
      /complete/,
    );
});
test("world manifest rejects incomplete class sets and unsafe private files", () => {
  validateMmorpgWorld(world());
  for (const mutate of [
    (w: MmorpgWorldManifest) => {
      w.seed = 3;
    },
    (w: MmorpgWorldManifest) => {
      w.uiURL = "/private-assets/ui.json";
    },
    (w: MmorpgWorldManifest) => {
      w.characters.sorceror = "https://other.test/a.json";
    },
    (w: MmorpgWorldManifest) => {
      w.scenery.vertices.byteLength--;
    },
    (w: MmorpgWorldManifest) => {
      w.scenery.catalog.pop();
    },
    (w: MmorpgWorldManifest) => {
      w.scenery.catalog[1].id = w.scenery.catalog[0].id;
    },
  ]) {
    const w = world();
    mutate(w);
    assert.throws(() => validateMmorpgWorld(w));
  }
});

test("world loading verifies the pinned filename hash over streamed valid JSON", async () => {
  const value = world(),
    bytes = new TextEncoder().encode(JSON.stringify(value));
  const hash = createHash("sha256").update(bytes).digest("hex");
  const url = `/private-assets/mmorpg-world-${hash}.json`;
  let calls = 0;
  const loaded = await loadMmorpgWorld(url, {
    fetcher: async (requested) => {
      calls++;
      assert.equal(requested, url);
      return new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(bytes.slice(0, 91));
            controller.enqueue(bytes.slice(91));
            controller.close();
          },
        }),
      );
    },
  });
  assert.deepEqual(loaded, value);
  assert.equal(calls, 1);
  const ui = { schema: "aethyrforge.mmorpg-ui.v1", icons: value.icons },
    uiBytes = JSON.stringify(ui);
  const uiHash = createHash("sha256").update(uiBytes).digest("hex"),
    uiURL = `/private-assets/mmorpg-ui-${uiHash}.json`;
  assert.deepEqual(
    await loadMmorpgDocument(uiURL, "ui", 65536, {
      fetcher: async () => new Response(uiBytes),
    }),
    ui,
  );
  await assert.rejects(loadMmorpgDocument(uiURL, "world", 65536), /prepared/);
});

test("world loading rejects a valid-schema tampered byte, HTTP failure and oversized stream", async () => {
  const text = JSON.stringify(world()),
    hash = createHash("sha256").update(text).digest("hex");
  const url = `/private-assets/mmorpg-world-${hash}.json`,
    tampered = text.replace('"yaw":0', '"yaw":1');
  assert.notEqual(text, tampered);
  assert.equal(text.length, tampered.length);
  validateMmorpgWorld(JSON.parse(tampered));
  let calls = 0;
  await assert.rejects(
    loadMmorpgWorld(url, {
      fetcher: async () => {
        calls++;
        return new Response(tampered);
      },
    }),
    AssetIntegrityError,
  );
  assert.equal(calls, 3);
  await assert.rejects(
    loadMmorpgWorld(url, {
      fetcher: async () => new Response(text, { status: 404 }),
    }),
    /finish loading/,
  );
  let cancelled = false;
  await assert.rejects(
    loadMmorpgWorld(url, {
      fetcher: async () =>
        new Response(
          new ReadableStream({
            start(controller) {
              controller.enqueue(new Uint8Array(2 * 1024 * 1024 + 1));
            },
            cancel() {
              cancelled = true;
            },
          }),
        ),
    }),
    /too large/,
  );
  assert(cancelled);
});
test("source scenery is deterministic, bounded and moves by cell rather than frame", () => {
  const w = world(),
    a = mmorpgSceneryPlacements(w, [-10, 1, 0]),
    b = mmorpgSceneryPlacements(w, [-10.1, 2, 0.1]);
  assert.deepEqual(a, b);
  assert(a.length > 0 && a.length <= 441);
  assert.deepEqual(mmorpgSceneryCenter([-10, 100, 0]), [-1, 0]);
  const c = mmorpgSceneryPlacements(w, [40, 1, 0]);
  assert.notDeepEqual(a, c);
  for (const p of a) {
    assert(p.position.every(Number.isFinite));
    assert(p.cell[0] >= -11 && p.cell[0] <= 9);
    assert(p.cell[1] >= -10 && p.cell[1] <= 10);
  }
  const instances = mmorpgSceneryInstances(w, a, 1),
    batches = mmorpgSceneBatches(instances);
  assert.equal(
    batches.reduce((n, b) => n + b.instanceCount, 0),
    instances.length,
  );
  assert(instances.every((i) => i.sourceClass === "MmorpgDecoration"));
});

test("streamed windows rebuild fixed resident capacities when population changes", () => {
  const w = world(),
    scene: any = {
      scenario: "day",
      meshes: w.scenery.meshes,
      materials: [{ name: "Stone" }],
      instances: [],
      batches: [],
      source: {
        mmorpgScenery: {
          firstInstance: 0,
          firstMesh: 0,
          firstVertex: 0,
          firstIndex: 0,
        },
      },
    };
  const vertices = new Float32Array(108 * 12),
    indices = Uint32Array.from({ length: 108 }, (_, i) => i),
    colors = new Uint8Array(108 * 4).fill(255);
  scene.instances = mmorpgSceneryInstances(
    w,
    mmorpgSceneryPlacements(w, [-10, 0, 0]),
    0,
  );
  scene.batches = mmorpgSceneBatches(scene.instances);
  const wind = prepareSourceWind(
    scene,
    sourceDraws(scene),
    vertices,
    indices,
    colors,
  );
  const variant: any = {
    scene,
    wind,
    bounds: wind.bounds,
    models: new Float32Array(scene.instances.flatMap((i: any) => i.model)),
    draws: wind.draws,
    instances: { size: 512 * 64 },
    windBuffer: { size: 512 * 16 },
  };
  const renderer: any = new Renderer(),
    core = createResidentSourcePreparation();
  const getSource = () => ({
    bounds: variant.bounds,
    models: variant.models,
    draws: variant.draws,
    wind: variant.wind.parameters,
  });
  const resident = core.prepare(getSource, 0),
    previousCount = variant.models.length / 16;
  Object.assign(renderer, {
    scene,
    mmorpgWorld: w,
    variants: new Map([["day", variant]]),
    frameScenes: new Map([["day", resident]]),
    gameplaySource: { vertices, indices, colors },
    device: { queue: { writeBuffer() {} } },
    windShadowPlanner: { reset() {} },
  });
  assert.equal(renderer.setMmorpgScenery([40, 0, 0]), true);
  assert.notEqual(variant.models.length / 16, previousCount);
  assert.equal(renderer.frameScenes.size, 0);
  assert.throws(() => resident.update(), /closed/);
  const replacement = core.prepare(getSource, 1);
  replacement.update();
  assert.equal(
    replacement.cull(
      Float32Array.from([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
    ).visibility.length,
    variant.bounds.count,
  );
  assert.equal(renderer.setMmorpgScenery([40.1, 0, 0]), false);
  replacement.dispose();
  core.dispose();
});
