import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { Renderer } from "../packages/renderer/renderer.ts";
import { DEFAULT_ENVIRONMENT } from "../packages/runtime/environment.ts";
import { prepareWorld } from "../packages/runtime/world-loader.ts";
import type { RendererFeatures } from "../packages/renderer/feature-contract.ts";
import type { CharacterManifest } from "../packages/renderer/character.ts";

function sources() {
  const day = {
    scenario: "day",
    vertices: "/owned/vertices.bin",
    indices: "/owned/indices.bin",
    materials: [],
    meshes: [],
    instances: [],
  } as unknown as CompiledScene;
  return [day, { ...day, scenario: "night" } as CompiledScene] as const;
}

test("only the trusted encounter host can load original zombies without patrol routes", async (t) => {
  const [day, night] = sources();
  day.enemyCharacterURL = night.enemyCharacterURL =
    "/private-assets/zombie.json";
  let character = {
    animationProfile: "quick-mmorpg-zombie-v1",
  } as CharacterManifest;
  const features = {
    character: { load: async () => character },
  } as unknown as RendererFeatures;
  const create = t.mock.method(
    Renderer,
    "create",
    async () => ({}) as Renderer,
  );
  const options = { report: () => {}, rendererOptions: { features } };
  await assert.rejects(
    prepareWorld(day, night, {} as OffscreenCanvas, options),
    /matching knight/,
  );
  const prepared = await prepareWorld(day, night, {} as OffscreenCanvas, {
    ...options,
    allowQuickCombat: true,
  });
  assert.equal(prepared.enemyCharacter, character);
  assert.equal(prepared.enemyPatrols, undefined);
  assert.equal(create.mock.callCount(), 1);
  character = {} as CharacterManifest;
  await assert.rejects(
    prepareWorld(day, night, {} as OffscreenCanvas, {
      ...options,
      allowQuickCombat: true,
    }),
    /zombie's original/,
  );
  assert.equal(create.mock.callCount(), 1);
});

test("shared loader prepares both scenarios with one cached sky document and explicit renderer options", async (t) => {
  const [day, night] = sources();
  day.environmentURL = night.environmentURL = "/owned/skies.json";
  const skies = {
    schema: "aethyrforge.private-environments.v1",
    scenarios: {
      day: { url: "/owned/day.bin" },
      night: { url: "/owned/night.bin" },
    },
  };
  const reads: string[] = [];
  t.mock.method(globalThis, "fetch", async (url: string) => {
    reads.push(url);
    assert.equal(url, day.environmentURL);
    return Response.json(skies);
  });
  const renderer = {} as Renderer;
  const create = t.mock.method(Renderer, "create", async () => renderer);
  const canvas = {} as OffscreenCanvas;
  const report = () => {};
  const rendererOptions = {};
  const prepared = await prepareWorld(day, night, canvas, {
    report,
    rendererOptions,
  });
  assert.deepEqual(reads, [day.environmentURL]);
  assert.equal(prepared.renderer, renderer);
  assert.equal(prepared.scene, day);
  assert.equal(prepared.nightScene, night);
  assert.equal(prepared.environment.skySource, "kit");
  assert.equal(prepared.character, undefined);
  assert.equal(prepared.dayCollision, undefined);
  assert.equal(prepared.nightCollision, undefined);
  assert.equal(prepared.dayGameplay, undefined);
  assert.equal(prepared.nightGameplay, undefined);
  const args = create.mock.calls[0].arguments;
  assert.equal(args[0], canvas);
  assert.equal(args[1], day);
  assert.equal(args[2], report);
  assert.equal(args[3], night);
  assert.deepEqual(args[5], skies);
  assert.deepEqual(args[10], { ...skies, atmosphere: {}, fogAmbient: {} });
  assert.deepEqual(args.at(-1), rendererOptions);
});

test("player and editor receive the same validated overview camera and environment", async (t) => {
  const [day, night] = sources();
  const document = {
    schema: "aethyrforge.owned-overview-presentation.v1",
    camera: {
      position: [1, 8, 12],
      target: [0, 0, 0],
      up: [0, 1, 0],
      verticalFovDegrees: 55,
    },
    lighting: {
      kind: "owned-overview-daylight-v1",
      sourceOverviewLightingRecovered: false,
      environment: { ...DEFAULT_ENVIRONMENT, skySource: "kit", exposure: 3 },
    },
  };
  t.mock.method(globalThis, "fetch", async (url: string) => {
    assert.equal(url, "/owned/overview.json");
    return Response.json(document);
  });
  t.mock.method(Renderer, "create", async () => ({}) as Renderer);
  const prepared = await prepareWorld(day, night, {} as OffscreenCanvas, {
    report: () => {},
    presentationURL: "/owned/overview.json",
  });
  assert.deepEqual(prepared.overview?.camera, document.camera);
  assert.deepEqual(prepared.environment, document.lighting.environment);
});

test("incompatible geometry, effect scenarios, and missing skies reject before GPU preparation", async (t) => {
  const create = t.mock.method(
    Renderer,
    "create",
    async () => ({}) as Renderer,
  );
  t.mock.method(
    globalThis,
    "fetch",
    async () => new Response("", { status: 404 }),
  );
  const options = { report: () => {} };
  {
    const [day, night] = sources();
    night.indices = "/owned/other-indices.bin";
    await assert.rejects(
      prepareWorld(day, night, {} as OffscreenCanvas, options),
      /day and night assets/,
    );
  }
  {
    const [day, night] = sources();
    day.particlesURL = "/owned/day-particles.json";
    night.particlesURL = "/owned/night-particles.json";
    await assert.rejects(
      prepareWorld(day, night, {} as OffscreenCanvas, options),
      /town effects together/,
    );
  }
  {
    const [day, night] = sources();
    day.environmentURL = "/owned/missing-skies.json";
    await assert.rejects(
      prepareWorld(day, night, {} as OffscreenCanvas, options),
      /HDR skies/,
    );
  }
  assert.equal(create.mock.callCount(), 0);
});
