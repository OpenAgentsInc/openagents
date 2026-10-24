import assert from "node:assert/strict";
import { test } from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import { roomRendererFeatures } from "../packages/renderer/room-features.ts";
import { selectedRendererFeatures } from "../packages/renderer/features.ts";
import {
  streamPackageIdentity,
  type StreamPackageManifest,
} from "../packages/runtime/cell-stream.ts";
import {
  playerProfileFeatures,
  type PlayerArtifact,
} from "../packages/runtime/player-artifact.ts";
import { preparePublishedWorld } from "../packages/runtime/published-world.ts";
const encode = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value));
const digest = async (bytes: Uint8Array<ArrayBuffer>) =>
  [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");

async function fixture(
  overrides: {
    world?: Record<string, unknown>;
    scene?: Record<string, unknown>;
    catalog?: Record<string, unknown>;
  } = {},
) {
  const worldURL = "/starter/player-world.json";
  const scene = {
    schema: "aethyrforge.private-scene.v1",
    vertexStride: 48,
    vertexCount: 3,
    indexCount: 3,
    vertices: "/starter/vertices.bin",
    indices: "/starter/indices.bin",
    vertexColors: "/starter/colors.bin",
    materials: [{}],
    meshes: [
      {
        id: "box",
        primitives: [
          {
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
            material: 0,
            materialSlot: 0,
          },
        ],
        bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1, 0.5] },
      },
    ],
    instances: [],
    batches: [],
    catalog: "/starter/catalog.json",
    ...overrides.scene,
  };
  const catalog = {
    schema: "aethyrforge.private-catalog.v1",
    assets: [
      {
        id: "barrel",
        mesh: 0,
        dimensions: [1, 1, 1],
        bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1, 0.5] },
      },
    ],
    ...overrides.catalog,
  };
  const world = {
    schema: "aethyrforge.player-world.v1",
    groundY: 0,
    mode: "walk",
    spawn: { position: [0, 0, 4], yaw: 0, pitch: 0, mode: "walk" },
    entities: [{ id: "prop", kind: "barrel", position: [0, 0, 0] }],
    colliders: [],
    rooms: [],
    ...overrides.world,
  };
  const kernel = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
  const data = new Map<string, Uint8Array<ArrayBuffer>>([
    [worldURL, encode(world)],
    ["/starter/scene.json", encode({ ...scene, scenario: "day" })],
    ["/starter/scene-night.json", encode({ ...scene, scenario: "night" })],
    ["/starter/catalog.json", encode(catalog)],
    ["/starter/vertices.bin", new Uint8Array(144)],
    ["/starter/indices.bin", new Uint8Array(new Uint32Array([0, 1, 2]).buffer)],
    ["/starter/colors.bin", new Uint8Array(12)],
  ]);
  const files = await Promise.all(
    [...data].map(async ([url, bytes]) => ({
      url,
      sha256: await digest(bytes),
      bytes: bytes.byteLength,
      gzipBytes: bytes.byteLength,
    })),
  );
  const unique = [
    ...new Map(files.map((file) => [file.sha256, file])).values(),
  ];
  const sum = (values: typeof files) =>
    values.reduce((total, file) => total + file.bytes, 0);
  const cooked: StreamPackageManifest = {
    schema: "aethyrforge.stream-package.v1",
    abi: 1,
    identityScope: "stream-descriptor-v1",
    sha256: "0".repeat(64),
    entryCell: "entry",
    cells: [
      {
        id: "entry",
        quality: "source",
        bounds: { min: [-10, -10], max: [10, 10] },
        scenes: {
          day: "/starter/scene.json",
          night: "/starter/scene-night.json",
        },
        resources: files,
      },
    ],
    files,
    totals: {
      rawBytes: sum(files),
      gzipBytes: sum(files),
      uniqueRawBytes: sum(unique),
      uniqueGzipBytes: sum(unique),
    },
  };
  cooked.sha256 = await streamPackageIdentity(cooked);
  const artifact: PlayerArtifact = {
    schema: "aethyrforge.player-artifact.v1",
    abi: 1,
    profile: "room",
    worldId: "owned-room",
    sourceIdentity: await digest(encode(world)),
    runtime: { abi: 1, url: "/player.js", sha256: "1".repeat(64), bytes: 1 },
    kernel: {
      abi: 1,
      url: "/kernel.wasm",
      sha256: await digest(kernel),
      bytes: kernel.byteLength,
    },
    features: playerProfileFeatures("room"),
    content: { package: cooked, entryCell: "entry", worldURL },
    resources: Object.fromEntries(
      playerProfileFeatures("room").map((name) => [
        name,
        name === "render"
          ? files.map((file) => file.url)
          : ["rules", "collision"].includes(name)
            ? [worldURL]
            : [],
      ]),
    ),
  };
  data.set("/kernel.wasm", kernel);
  const reads: string[] = [];
  const fetcher: typeof fetch = async (input) => {
    const url = String(input);
    reads.push(url);
    const bytes = data.get(url);
    if (!bytes) return new Response("missing", { status: 404 });
    return new Response(bytes.slice().buffer);
  };
  return { artifact, data, reads, fetcher, kernel };
}
test("published staging uses verified bytes, restores fetch, and transfers only the prepared renderer", async (t) => {
  const input = await fixture();
  const originalFetch = globalThis.fetch;
  let disposed = 0;
  const renderer = {
    dispose() {
      disposed++;
    },
  } as Renderer;
  t.mock.method(
    Renderer,
    "create",
    async (...args: Parameters<typeof Renderer.create>) => {
      assert.deepEqual(args[22]?.features, {
        ...roomRendererFeatures,
        createMeshInspection: selectedRendererFeatures.createMeshInspection,
        createMaterialEditor: selectedRendererFeatures.createMaterialEditor,
      });
      assert.notEqual(globalThis.fetch, originalFetch);
      const body = await (await fetch("/starter/player-world.json")).json();
      assert.equal(body.schema, "aethyrforge.player-world.v1");
      for (const [url, init] of [
        ["https://example.com/outside.bin", undefined],
        ["/starter/missing.bin", undefined],
        ["/starter/player-world.json?other=1", undefined],
        ["/starter/player-world.json", { method: "POST" }],
      ] as const)
        await assert.rejects(fetch(url, init));
      return renderer;
    },
  );
  const result = await preparePublishedWorld(
    input.artifact,
    {} as OffscreenCanvas,
    { fetcher: input.fetcher },
  );
  assert.equal(globalThis.fetch, originalFetch);
  assert.equal(result.prepared?.renderer, renderer);
  assert.equal(result.catalog?.assets[0].id, "barrel");
  assert.deepEqual(new Uint8Array(result.kernelBytes!), input.kernel);
  assert.equal(disposed, 0);
  assert.equal(result.streamMetrics?.disposed, true);
  assert.equal(result.streamMetrics?.heldBytes, 0);
  assert.equal(result.streamMetrics?.cachedResources, 0);
  assert(result.streamMetrics!.highWaterBytes > 0);
  assert.equal(
    input.reads.length,
    input.artifact.content!.package.files.length + 1,
  );
  assert(result.stageTimings.totalMs >= result.stageTimings.prepareMs);
  result.prepared!.renderer.dispose();
  assert.equal(disposed, 1);
});
test("published rooms retain only host editor tools with explicit minimal or full host features", async (t) => {
  const input = await fixture();
  const customInspection =
    selectedRendererFeatures.createMeshInspection!.bind(undefined);
  const customMaterials =
    selectedRendererFeatures.createMaterialEditor!.bind(undefined);
  const seen: Parameters<typeof Renderer.create>[22][] = [];
  t.mock.method(
    Renderer,
    "create",
    async (...args: Parameters<typeof Renderer.create>) => {
      seen.push(args[22]);
      return { dispose() {} } as Renderer;
    },
  );
  for (const features of [
    roomRendererFeatures,
    {
      ...selectedRendererFeatures,
      createMeshInspection: customInspection,
      createMaterialEditor: customMaterials,
    },
  ]) {
    const result = await preparePublishedWorld(
      input.artifact,
      {} as OffscreenCanvas,
      {
        fetcher: input.fetcher,
        rendererOptions: { features, diagnostics: true },
      },
    );
    result.prepared!.renderer.dispose();
  }
  assert.equal(
    seen[0]!.features!.createMeshInspection,
    selectedRendererFeatures.createMeshInspection,
  );
  assert.equal(seen[1]!.features!.createMeshInspection, customInspection);
  assert.equal(
    seen[0]!.features!.createMaterialEditor,
    selectedRendererFeatures.createMaterialEditor,
  );
  assert.equal(seen[1]!.features!.createMaterialEditor, customMaterials);
  for (const options of seen) {
    assert.equal(options!.diagnostics, true);
    assert.deepEqual(Object.keys(options!.features!), [
      "createMeshInspection",
      "createMaterialEditor",
    ]);
    assert.equal(options!.features!.character, undefined);
    assert.equal(options!.features!.sky, undefined);
    assert.equal(options!.features!.createExperiment, undefined);
    assert.equal(options!.features!.prepareCanonicalSourceIndices, undefined);
  }
});
test("tampered kernel, resource bytes, descriptor, and incompatible authored features fail before GPU creation", async (t) => {
  const create = t.mock.method(
    Renderer,
    "create",
    async () => ({}) as Renderer,
  );
  const cases = [
    async () => {
      const value = await fixture();
      value.data.set("/kernel.wasm", new Uint8Array(8));
      return value;
    },
    async () => {
      const value = await fixture();
      value.data.set("/starter/colors.bin", new Uint8Array(11));
      return value;
    },
    async () => {
      const value = await fixture();
      value.artifact.content!.package.sha256 = "f".repeat(64);
      return value;
    },
    async () => {
      const value = await fixture();
      value.artifact.sourceIdentity = "f".repeat(64);
      return value;
    },
    () => fixture({ scene: { characterURL: "/starter/character.json" } }),
    () => fixture({ scene: { environmentURL: "/starter/sky.json" } }),
    () =>
      fixture({
        scene: { instances: [{ mesh: 999, model: [], materialOverrides: [] }] },
      }),
    () =>
      fixture({
        catalog: {
          assets: [
            {
              id: "barrel",
              mesh: 999,
              dimensions: [1, 1, 1],
              bounds: { min: [0, 0, 0], max: [1, 1, 1] },
            },
          ],
        },
      }),
    () =>
      fixture({
        world: {
          entities: [{ id: "missing", kind: "missing", position: [0, 0, 0] }],
        },
      }),
    () =>
      fixture({
        world: { presentationURL: "/starter/absent-presentation.json" },
      }),
  ];
  for (const make of cases) {
    const input = await make();
    await assert.rejects(
      preparePublishedWorld(input.artifact, {} as OffscreenCanvas, {
        fetcher: input.fetcher,
      }),
    );
  }
  assert.equal(create.mock.callCount(), 0);
});
test("canceled GPU preparation disposes the candidate and restores the previous fetch function", async (t) => {
  const input = await fixture(),
    controller = new AbortController();
  const originalFetch = globalThis.fetch;
  let disposed = 0;
  t.mock.method(Renderer, "create", async () => {
    controller.abort();
    return {
      dispose() {
        disposed++;
      },
    } as Renderer;
  });
  await assert.rejects(
    preparePublishedWorld(input.artifact, {} as OffscreenCanvas, {
      fetcher: input.fetcher,
      signal: controller.signal,
    }),
    /cancel/i,
  );
  assert.equal(globalThis.fetch, originalFetch);
  assert.equal(disposed, 1);
});
test("empty player artifacts never fetch content or allocate a simulation or scene renderer", async (t) => {
  const create = t.mock.method(
    Renderer,
    "create",
    async () => ({}) as Renderer,
  );
  let reads = 0;
  const result = await preparePublishedWorld(
    {
      schema: "aethyrforge.player-artifact.v1",
      abi: 1,
      profile: "empty",
      worldId: "empty",
      sourceIdentity: "0".repeat(64),
      runtime: { abi: 1, url: "/empty.js", sha256: "1".repeat(64), bytes: 1 },
      features: ["render"],
      resources: {},
    },
    {} as OffscreenCanvas,
    {
      fetcher: async () => {
        reads++;
        throw Error("Unexpected fetch");
      },
    },
  );
  assert.equal(reads, 0);
  assert.equal(create.mock.callCount(), 0);
  assert.equal(result.world, undefined);
  assert.equal(result.prepared, undefined);
  assert.equal(result.kernelBytes, undefined);
  assert.equal(result.streamMetrics, undefined);
});
