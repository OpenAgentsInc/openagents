import assert from "node:assert/strict";
import test from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { sourceDraws } from "../packages/renderer/source.ts";
import { surfaceSkyDraws } from "../packages/renderer/surface-sky-draws.ts";
import { surfaceFixture } from "./fixtures/surface-sky.ts";

function fixture() {
  return {
    scenario: "day",
    materials: [
      { id: "old-leaf", name: "Old leaf" },
      { id: "owned-leaf", name: "Native leaf" },
    ],
    meshes: [
      {
        id: "owned-triangle",
        name: "Owned tree",
        primitives: [
          {
            material: 0,
            materialSlot: 0,
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
          },
        ],
      },
    ],
    instances: [
      {
        actor: "Owned plant",
        component: "Mesh",
        mesh: 0,
        materialOverrides: [1],
      },
    ],
    batches: [
      { mesh: 0, materialOverrides: [0], firstInstance: 0, instanceCount: 1 },
    ],
  } as unknown as CompiledScene;
}
const draws = (scene: CompiledScene) =>
  sourceDraws(scene).map((d) => ({ ...d, wind: false }));

test("stale render batches cannot silently disable a selected lighting field", () => {
  const scene = fixture();
  assert.equal(draws(scene)[0].material, 0);
  assert.throws(
    () => surfaceSkyDraws(scene, draws(scene), surfaceFixture(), () => 7),
    /surface lighting again/,
  );
  scene.batches[0].materialOverrides = [1];
  assert.equal(draws(scene)[0].material, 1);
  let bound = 0;
  const result = surfaceSkyDraws(
    scene,
    draws(scene),
    surfaceFixture(),
    (material, selection) => {
      assert.equal(material, 1);
      assert.equal(selection, 1);
      bound++;
      return 7;
    },
  );
  assert.equal(bound, 1);
  assert.equal(result[0].material, 7);
});

test("every selected material must reach a render batch", () => {
  const scene = fixture(),
    manifest = surfaceFixture();
  scene.batches[0].materialOverrides = [1];
  manifest.selections[0].materialIds.push("missing-bark");
  assert.throws(
    () => surfaceSkyDraws(scene, draws(scene), manifest, () => 7),
    /surface lighting again/,
  );
});

test("selected batch mesh and effective materials must agree with the source instance", () => {
  const scene = fixture();
  scene.batches[0].materialOverrides = [1];
  scene.meshes.push({
    ...structuredClone(scene.meshes[0]),
    id: "wrong-mesh",
    primitives: [{ ...scene.meshes[0].primitives[0], firstIndex: 3 }],
  });
  scene.batches[0].mesh = 1;
  assert.equal(draws(scene)[0].material, 1);
  assert.throws(
    () => surfaceSkyDraws(scene, draws(scene), surfaceFixture(), () => 7),
    /surface lighting again/,
  );
  scene.batches[0].mesh = 0;
  scene.batches.push(structuredClone(scene.batches[0]));
  assert.throws(
    () => surfaceSkyDraws(scene, draws(scene), surfaceFixture(), () => 7),
    /surface lighting again/,
  );
  scene.batches.pop();
  scene.batches[0].materialOverrides = [];
  assert.throws(
    () => surfaceSkyDraws(scene, draws(scene), surfaceFixture(), () => 7),
    /surface lighting again/,
  );
});

test("selected draws must preserve every primitive range, base vertex and effective material", () => {
  const scene = fixture();
  scene.batches[0].materialOverrides = [1];
  const original = draws(scene);
  for (const patch of [
    { firstIndex: 3 },
    { indexCount: 6 },
    { baseVertex: 1 },
    { material: 0 },
  ]) {
    assert.throws(
      () =>
        surfaceSkyDraws(
          scene,
          [{ ...original[0], ...patch }],
          surfaceFixture(),
          () => 7,
        ),
      /surface lighting again/,
    );
  }
  for (const altered of [[], [...original, ...original]])
    assert.throws(
      () => surfaceSkyDraws(scene, altered, surfaceFixture(), () => 7),
      /surface lighting again/,
    );
});

test("valid shared batches split only the selected instance and no-field draws remain unchanged", () => {
  const scene = fixture();
  scene.instances = [
    { ...scene.instances[0], actor: "Before" },
    scene.instances[0],
    { ...scene.instances[0], actor: "After" },
  ];
  scene.batches[0].materialOverrides = [1];
  scene.batches[0].instanceCount = 3;
  const source = draws(scene);
  assert.deepEqual(
    source.map((d) => [d.firstInstance, d.count, d.material]),
    [[0, 3, 1]],
  );
  const actual = surfaceSkyDraws(scene, source, surfaceFixture(), () => 7);
  assert.deepEqual(
    actual.map((d) => [d.firstInstance, d.count, d.material]),
    [
      [0, 1, 1],
      [1, 1, 7],
      [2, 1, 1],
    ],
  );
  scene.batches[0].mesh = 999;
  assert.deepEqual(
    surfaceSkyDraws(scene, source, undefined, () => 7),
    source,
  );
  const otherMode = surfaceFixture();
  delete otherMode.scenarios.day;
  assert.deepEqual(
    surfaceSkyDraws(scene, source, otherMode, () => 7),
    source,
  );
});
