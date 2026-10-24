import { test } from "node:test";
import assert from "node:assert/strict";
import { sourceDraws, emissiveAnchors } from "../packages/renderer/source.ts";
import { modelMatrix } from "../packages/renderer/math.ts";
const fixture = () =>
  ({
    materials: [
      { name: "Fallback", emissiveFactor: [0, 0, 0] },
      { name: "Lamp glass", emissiveFactor: [3, 2, 1] },
    ],
    meshes: [
      {
        name: "Owned engine plane",
        primitives: [
          {
            firstIndex: 0,
            indexCount: 3,
            baseVertex: 0,
            material: 1,
            materialSlot: 0,
          },
        ],
      },
    ],
    instances: [
      {
        mesh: 0,
        actor: "lamp",
        materialOverrides: [-1],
        model: modelMatrix([4, 2, 8]),
      },
      {
        mesh: 0,
        actor: "FX_fog_Card",
        materialOverrides: [-1],
        model: modelMatrix([4, 2, 8]),
      },
      {
        mesh: 0,
        actor: "lamp2",
        materialOverrides: [-1],
        model: modelMatrix([8, 2, 8]),
      },
    ],
    batches: [
      { mesh: 0, materialOverrides: [-1], firstInstance: 0, instanceCount: 3 },
    ],
  }) as any;
test("source draw filtering respects -1 material defaults and splits mixed fog-card batches", () => {
  const draws = sourceDraws(fixture());
  assert.deepEqual(
    draws.map((d) => [d.material, d.firstInstance, d.count]),
    [
      [1, 0, 1],
      [1, 2, 1],
    ],
  );
});
test("emissive anchors use geometry centers and deduplicate shared placements", () => {
  const vertices = new Float32Array(36);
  vertices.set([0, 0, 0], 0);
  vertices.set([2, 0, 0], 12);
  vertices.set([0, 2, 0], 24);
  assert.deepEqual(
    emissiveAnchors(fixture(), vertices, new Uint32Array([0, 1, 2])),
    [
      [5, 3, 8],
      [9, 3, 8],
    ],
  );
});
