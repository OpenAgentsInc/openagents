import { test } from "node:test";
import assert from "node:assert/strict";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { surfaceFixture } from "./fixtures/surface-sky.ts";
import { sourceDraws } from "../packages/renderer/source.ts";
import { surfaceSkyDraws } from "../packages/renderer/surface-sky-draws.ts";
const model = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
const bounds = { min: [0, 0, 0], max: [1, 1, 1] };
const fixture = (): CompiledScene => ({
  schema: "aethyrforge.private-scene.v1",
  scenario: "day",
  vertices: "/private-assets/owned-vertices.bin",
  indices: "/private-assets/owned-indices.bin",
  vertexColors: "/private-assets/owned-colors.bin",
  vertexStride: 48,
  vertexCount: 23,
  indexCount: 15,
  materials: [
    {
      id: "owned-leaf",
      name: "Owned leaf",
      baseColorFactor: [1, 1, 1, 1],
      emissiveFactor: [0, 0, 0],
      roughness: 0.5,
      metallic: 0,
      normalScale: 1,
      uvScale: [1, 1],
      alphaMode: "OPAQUE",
      alphaCutoff: 0.5,
      doubleSided: true,
      packedChannels: "metallic-r-roughness-g-occlusion-b",
    },
  ],
  meshes: [
    {
      id: "owned-triangle",
      name: "Owned triangle",
      bounds,
      primitives: [
        {
          firstIndex: 12,
          indexCount: 3,
          baseVertex: 20,
          material: 0,
          materialSlot: 0,
        },
      ],
    },
  ],
  instances: ["Before", "Owned plant", "Between", "Second plant", "After"].map(
    (actor) => ({
      actor,
      component: "Mesh",
      mesh: 0,
      model: [...model],
      materialOverrides: [],
      sourceClass: "Owned fixture",
    }),
  ),
  batches: [
    { mesh: 0, firstInstance: 0, instanceCount: 5, materialOverrides: [] },
  ],
  bounds,
  spawn: { position: [0, 1, 0], target: [0, 1, -1], source: "Owned fixture" },
  source: { map: "Owned fixture", placements: 5, actorCensus: {} },
  diagnostics: { warnings: [], skippedPlacements: 0 },
});
function manifest() {
  const value = surfaceFixture();
  value.selections.push({
    ...structuredClone(value.selections[0]),
    actor: "Second plant",
    recordStart: 3,
  });
  value.vertexCount = 6;
  value.scenarios.day!.bytes = 6 * 72 * 4;
  return value;
}
const draws = (scene: CompiledScene) =>
  sourceDraws(scene).map((draw) => ({ ...draw, wind: false }));
test("static surface refinement preserves shared instance addresses and primitive indices", () => {
  const scene = fixture(),
    before = JSON.stringify(scene),
    selections: number[] = [];
  const split = surfaceSkyDraws(
    scene,
    draws(scene),
    manifest(),
    (material, selected) => {
      selections.push(selected);
      return material + selected + 10;
    },
  );
  assert.deepEqual(selections, [1, 2]);
  assert.deepEqual(
    split.map((d) => [d.firstInstance, d.count, d.material]),
    [
      [0, 1, 0],
      [1, 1, 11],
      [2, 1, 0],
      [3, 1, 12],
      [4, 1, 0],
    ],
  );
  assert.ok(
    split.every(
      (d) => d.firstIndex === 12 && d.indexCount === 3 && d.baseVertex === 20,
    ),
  );
  assert.equal(JSON.stringify(scene), before);
});
test("unselected scenarios stay unchanged and animated selections cannot use static lighting", () => {
  const scene = fixture(),
    source = draws(scene);
  assert.deepEqual(
    surfaceSkyDraws({ ...scene, scenario: "night" }, source, manifest(), () => {
      throw Error();
    }),
    source,
  );
  assert.throws(
    () =>
      surfaceSkyDraws(
        scene,
        [{ ...source[0], wind: true }],
        manifest(),
        () => 10,
      ),
    /still object/,
  );
});
