import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { modelMatrix, cross, type V3 } from "../packages/renderer/math.ts";
import {
  fracturePlacement,
  fracturePlacementSteps,
  fractureSurfaceArea,
  fractureSurfaceAreaSteps,
  type FractureSource,
} from "../packages/renderer/source-fracture.ts";
import { SourceDebris } from "../packages/renderer/source-debris.ts";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  GameplayEffectSampler,
  GAMEPLAY_VISUAL_LIMITS,
  gameplayLights,
  maskDestroyedPlacements,
  validateGameplayVisualState,
  type GameplayDebris,
  type GameplayEffect,
} from "../packages/renderer/gameplay-effects.ts";

function fixture() {
  const points: V3[] = [
    [-1, -1, -1],
    [1, -1, -1],
    [1, 1, -1],
    [-1, 1, -1],
    [-1, -1, 1],
    [1, -1, 1],
    [1, 1, 1],
    [-1, 1, 1],
  ];
  const vertices = new Float32Array(
    points.flatMap(([x, y, z]) => [
      x,
      y,
      z,
      0,
      1,
      0,
      (x + 1) / 2,
      (z + 1) / 2,
      1,
      0,
      0,
      1,
    ]),
  );
  const indices = new Uint32Array([
    0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0,
    4, 7, 0, 7, 3, 1, 2, 6, 1, 6, 5,
  ]);
  const source: FractureSource = {
    vertices,
    indices,
    colors: new Uint8Array(32).fill(255),
    uv1: new Float32Array(
      points.flatMap(([x, y]) => [(x + 1) / 2, (y + 1) / 2]),
    ),
  };
  const scene = {
    materials: [{}],
    meshes: [
      {
        id: "owned:cube",
        bounds: { min: [-1, -1, -1], max: [1, 1, 1] },
        primitives: [
          {
            firstIndex: 0,
            indexCount: 36,
            baseVertex: 0,
            material: 0,
            materialSlot: 0,
          },
        ],
      },
    ],
    instances: [
      { mesh: 0, model: modelMatrix([0, 0, 0]), materialOverrides: [] },
    ],
  } as unknown as CompiledScene;
  return { scene, source };
}
function area(parts: ReturnType<typeof fracturePlacement>) {
  let sum = 0;
  for (const part of parts)
    for (let i = 0; i < part.vertices.length; i += 36) {
      const v = part.vertices;
      const a = [
          v[i + 12] - v[i],
          v[i + 13] - v[i + 1],
          v[i + 14] - v[i + 2],
        ] as V3,
        b = [
          v[i + 24] - v[i],
          v[i + 25] - v[i + 1],
          v[i + 26] - v[i + 2],
        ] as V3;
      sum += Math.hypot(...cross(a, b)) * 0.5;
    }
  return sum;
}

test("real source fracture clips each cell without losing area, UVs, material, or source bytes", () => {
  const { scene, source } = fixture(),
    original = source.vertices.slice();
  const left = fracturePlacement(scene, source, 0, [-1, -1, -1], [0, 1, 1]);
  const right = fracturePlacement(
    scene,
    source,
    0,
    [0, -1, -1],
    [1, 1, 1],
    false,
  );
  assert.equal(left.length, 8);
  assert.equal(right.length, 1);
  assert(Math.abs(area(left) - 12) < 1e-6);
  assert(Math.abs(area(right) - 12) < 1e-6);
  for (const [chunk, part] of left.entries())
    for (let i = 0; i < part.vertices.length; i += 12) {
      const p = [0, 1, 2].map(
        (axis) => part.vertices[i + axis] + part.center[axis],
      );
      assert(p[0] >= -1 - 1e-6 && p[0] <= 1e-6);
      for (let axis = 0; axis < 3; axis++) {
        const middle = axis === 0 ? -0.5 : 0;
        assert(
          chunk & (1 << axis)
            ? p[axis] >= middle - 1e-6
            : p[axis] <= middle + 1e-6,
        );
      }
      assert(part.vertices[i + 6] >= 0 && part.vertices[i + 6] <= 1);
      assert(part.vertices[i + 7] >= 0 && part.vertices[i + 7] <= 1);
    }
  assert(
    left.some((p) =>
      p.vertices.some((v, i) => i % 12 === 6 && v > 0.001 && v < 0.999),
    ),
  );
  assert(left.every((part) => part.draws.every((draw) => draw.material === 0)));
  assert(left.every((part) => part.colors.every((c) => c === 255)));
  assert.deepEqual(source.vertices, original);
});

test("resumable fracture checkpoints preserve exact geometry and area across all subdivision modes", () => {
  const { scene, source } = fixture();
  const repeated = {
    ...source,
    indices: new Uint32Array(
      Array.from({ length: 24 }, () => [...source.indices]).flat(),
    ),
  };
  scene.meshes[0].primitives[0].indexCount = repeated.indices.length;
  const before = repeated.vertices.slice();
  for (const mirrored of [false, true]) {
    if (mirrored) scene.instances[0].model[0] = -1;
    for (const split of [false, true, 4] as const) {
      const expected = fracturePlacement(
        scene,
        repeated,
        0,
        [-1, -1, -1],
        [1, 1, 1],
        split,
      );
      const steps = fracturePlacementSteps(
        scene,
        repeated,
        0,
        [-1, -1, -1],
        [1, 1, 1],
        split,
      );
      let yields = 0;
      for (;;) {
        const step = steps.next();
        if (step.done) {
          assert.deepEqual(step.value, expected);
          break;
        }
        assert.equal(
          step.value,
          undefined,
          "No partial geometry may leave the iterator.",
        );
        yields++;
      }
      assert.ok(
        yields >= 9,
        "Input triangle traversal must checkpoint within one large cell.",
      );
      const areaSteps = fractureSurfaceAreaSteps(expected);
      let areaYields = 0;
      for (;;) {
        const step = areaSteps.next();
        if (step.done) {
          assert.equal(step.value, fractureSurfaceArea(expected));
          break;
        }
        areaYields++;
      }
      assert.ok(areaYields > 0);
      assert.ok(Math.abs(fractureSurfaceArea(expected) - 24 * 24) < 1e-4);
    }
  }
  assert.deepEqual(repeated.vertices, before);
});

test("resumable packing matches owned geometry bytes recorded before checkpoint extraction", () => {
  // Captured independently from the implementation at 5109abf before this change.
  const expected = [
    "6b9d90a39ae43471931bfa1f1fab9d15c0c51d1a6e44e37684ca68cb3fa7eae2",
    "9485acfa76ebc5d020a3e6083ff476be25fc0b32c2f34964425f340ed5c6c03d",
    "e5812ae190887aded4f5df4a95e115ae0e558346f0dd717638e1336dcdf6619c",
    "047a2901dd190ac97f24d9b70b0e41444ef522733c500470269d618a5b702af5",
    "7a883579ed4fc83b5ee5c1b29ee26d9b047733a1f213202707c1d17bd3e38bf6",
    "17837d33b592e82a55c0738dc8695516a407f0ae509f5fed40273e2a2db80432",
  ];
  let index = 0;
  for (const mirrored of [false, true])
    for (const split of [false, true, 4] as const) {
      const { scene, source } = fixture();
      if (mirrored) scene.instances[0].model[0] = -1;
      const chunks = fracturePlacement(
        scene,
        source,
        0,
        [-1, -1, -1],
        [1, 1, 1],
        split,
      );
      const hash = createHash("sha256");
      for (const chunk of chunks) {
        hash.update(
          JSON.stringify({ center: chunk.center, draws: chunk.draws }),
        );
        for (const data of [chunk.vertices, chunk.colors, chunk.uv1])
          hash.update(
            new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
          );
      }
      assert.equal(hash.digest("hex"), expected[index++]);
    }
});

test("fracture cancellation can stop during one large cell without producing geometry or altering input", () => {
  const { scene, source } = fixture();
  const repeated = {
    ...source,
    indices: new Uint32Array(
      Array.from({ length: 24 }, () => [...source.indices]).flat(),
    ),
  };
  scene.meshes[0].primitives[0].indexCount = repeated.indices.length;
  const before = structuredClone(repeated);
  const steps = fracturePlacementSteps(
    scene,
    repeated,
    0,
    [-1, -1, -1],
    [1, 1, 1],
    4,
  );
  assert.equal(steps.next().done, false);
  assert.deepEqual(steps.return([]), { done: true, value: [] });
  assert.equal(steps.next().done, true);
  assert.deepEqual(repeated, before);
  assert.throws(
    () => fracturePlacement(scene, source, -1, [-1, -1, -1], [1, 1, 1]),
    /valid source/,
  );
  assert.throws(
    () =>
      fracturePlacementSteps(scene, source, -1, [-1, -1, -1], [1, 1, 1]).next(),
    /valid source/,
  );
});

test("reflected source transforms retain finite normalized shading frames and exact surface area", () => {
  const { scene, source } = fixture();
  scene.instances[0].model = [-2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1];
  const parts = fracturePlacement(scene, source, 0, [2, 2, 5], [6, 8, 7]);
  assert(Math.abs(area(parts) - 88) < 1e-5);
  for (const part of parts)
    for (let i = 0; i < part.vertices.length; i += 12) {
      assert(
        Math.abs(Math.hypot(...part.vertices.subarray(i + 3, i + 6)) - 1) <
          1e-6,
      );
      assert.equal(part.vertices[i + 11], -1);
    }
});

test("castle quarter-grid fragments preserve source area and stay within their smaller physics cells", () => {
  const { scene, source } = fixture();
  const parts = fracturePlacement(scene, source, 0, [-1, -1, -1], [1, 1, 1], 4);
  assert.equal(parts.length, 64);
  assert(Math.abs(area(parts) - 24) < 1e-6);
  assert(parts.some((part, chunk) => chunk >= 32 && part.vertices.length));
  for (const [chunk, part] of parts.entries())
    for (let i = 0; i < part.vertices.length; i += 12)
      for (let axis = 0; axis < 3; axis++) {
        const digit = 2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1);
        const center = -1 + (digit + 0.5) / 2;
        assert.equal(part.center[axis], center);
        assert(Math.abs(part.vertices[i + axis]) <= 0.25 + 1e-6);
      }
  assert(
    parts.every((part) => part.draws.every((draw) => draw.material === 0)),
  );
});

test("shared source bounds preserve coplanar faces despite affine rounding differences", () => {
  const { scene, source } = fixture();
  const parts = fracturePlacement(
    scene,
    source,
    0,
    [-1 + 3e-15, -1 + 3e-15, -1 + 3e-15],
    [1 - 3e-15, 1 - 3e-15, 1 - 3e-15],
  );
  assert(Math.abs(area(parts) - 24) < 1e-5);
});

test("cell preflight is invisible and atomic; partial destruction retains textured standing surfaces", () => {
  const old = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  const uploads = new Map<any, Uint8Array>(),
    buffers: any[] = [];
  const device = {
    createBuffer: ({ size }: any) => {
      const b = { size, destroy() {} };
      buffers.push(b);
      return b;
    },
    queue: {
      writeBuffer: (buffer: any, _offset: number, data: any) =>
        uploads.set(
          buffer,
          new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice(),
        ),
    },
  } as unknown as GPUDevice;
  try {
    const { scene, source } = fixture();
    const debris = new SourceDebris(device, new Map([["day", scene]]), source);
    debris.setObjects("day", [
      { id: 10, sourceId: 0, min: [-1, -1, -1], max: [0, 1, 1] },
      { id: 11, sourceId: 0, min: [0, -1, -1], max: [1, 1, 1] },
    ]);
    const before = uploads.size;
    assert.equal(debris.prepare("day", [10]), true);
    assert.equal(uploads.size, before);
    assert.equal(debris.update("day", [], []).size, 0);
    assert.equal(debris.statsSnapshot.standingTriangles, 0);
    const bodies: GameplayDebris[] = Array.from({ length: 64 }, (_, chunk) => ({
      id: chunk,
      nodeId: 10,
      sourceId: 0,
      subdivisions: 4,
      chunk,
      sourceMin: [-1, -1, -1],
      sourceMax: [0, 1, 1],
      position: [-1, -1, -1].map(
        (v, axis) =>
          v +
          ((2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1) + 0.5) /
            4) *
            (axis === 0 ? 1 : 2),
      ) as V3,
      rotation: [0, 0, 0],
      halfExtents: [0.125, 0.25, 0.25],
      velocity: [0, 1, 0],
      age: 0,
      life: 20,
    }));
    assert.deepEqual([...debris.update("day", [10], bodies)], [0]);
    assert.equal(debris.statsSnapshot.maskedNodes, 1);
    assert.equal(debris.statsSnapshot.standingCells, 1);
    assert(debris.statsSnapshot.standingTriangles > 0);
    assert(debris.statsSnapshot.debrisTriangles > 0);
    assert(debris.statsSnapshot.renderedDebris > 0);
    const standingVertices = new Float32Array(uploads.get(buffers[0])!.buffer);
    for (let i = 0; i < standingVertices.length; i += 12)
      assert(
        standingVertices[i] >= -1e-6 && standingVertices[i] <= 1 + 1e-6,
        "The surviving half must stay in its original world location.",
      );
    const draws: any[] = [];
    const material = {} as GPUBindGroup,
      pass = {
        setVertexBuffer() {},
        setIndexBuffer() {},
        setBindGroup(_index: number, binding: any) {
          assert.equal(binding, material);
        },
        drawIndexed(...args: any[]) {
          draws.push(args);
        },
      } as unknown as GPURenderPassEncoder;
    debris.drawStanding(pass, [material], {} as GPUBuffer, () => true);
    debris.drawMoving(pass, [material], {} as GPUBuffer, () => true);
    assert(draws.length > 1);
    const prior = debris.statsSnapshot;
    assert.equal(debris.prepare("day", [999]), false);
    assert.match(debris.limitReason, /unavailable/);
    assert.deepEqual(
      { ...debris.statsSnapshot, limitReason: "" },
      { ...prior, limitReason: "" },
    );
    assert.equal(debris.prepare("day", [10, 11]), true);
    debris.update("day", [10, 11], bodies);
    assert.equal(debris.statsSnapshot.standingTriangles, 0);
    const settled = { ...bodies[0], age: 60, position: [2, 0, 0] as V3 };
    validateGameplayVisualState({
      destroyed: [10, 11],
      debris: [],
      effects: [],
      castle: { rubble: [settled] },
    });
    debris.update("day", [10, 11], [], [settled]);
    assert.equal(debris.statsSnapshot.renderedDebris, 0);
    assert.equal(debris.statsSnapshot.rubbleCount, 1);
    assert(debris.statsSnapshot.rubbleTriangles > 0);
    const priorDraws = draws.length;
    debris.drawStanding(
      pass,
      [material],
      {} as GPUBuffer,
      () => true,
      new Float32Array(modelMatrix([0, 0, 0])),
    );
    assert.equal(
      draws.length,
      priorDraws,
      "Offscreen permanent rubble must not enter the pass.",
    );
    debris.drawStanding(pass, [material], {} as GPUBuffer, () => true);
    assert.equal(
      draws.length,
      priorDraws + 1,
      "Settled fragments share one draw per source material.",
    );
    const staticRevision = debris.staticRevision;
    debris.update("day", [10, 11], [], [settled]);
    assert.equal(debris.statsSnapshot.rubbleCount, 1);
    assert.equal(debris.staticRevision, staticRevision);
    debris.reset("day");
    assert.equal(debris.statsSnapshot.maskedNodes, 0);
    assert.equal(debris.statsSnapshot.rubbleCount, 0);
    debris.setObjects("day", [
      { id: 10, sourceId: 0, min: [-1, -1, -1], max: [0, 1, 1] },
    ]);
    const beforeIncomplete = uploads.size;
    assert.equal(debris.prepare("day", [10]), false);
    assert.match(debris.limitReason, /incomplete destruction map/);
    assert.equal(uploads.size, beforeIncomplete);
    assert.equal(debris.statsSnapshot.maskedNodes, 0);
    assert.equal(debris.statsSnapshot.standingTriangles, 0);
    debris.dispose();
  } finally {
    if (old) Object.defineProperty(globalThis, "GPUBufferUsage", old);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("town restore preserves topology, fragment caches and GPU slabs and is idle when pristine", () => {
  const old = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  let uploads = 0,
    creates = 0,
    invalidations = 0;
  const device = {
    createBuffer: ({ size }: { size: number }) => {
      creates++;
      return { size, destroy() {} };
    },
    queue: {
      writeBuffer() {
        uploads++;
      },
    },
  } as unknown as GPUDevice;
  try {
    const { scene, source } = fixture();
    const visual = new SourceDebris(device, new Map([["day", scene]]), source);
    const objects = [
      { id: 10, sourceId: 0, min: [-1, -1, -1] as V3, max: [0, 1, 1] as V3 },
      { id: 11, sourceId: 0, min: [0, -1, -1] as V3, max: [1, 1, 1] as V3 },
    ];
    visual.setObjects("day", objects);
    const renderer = Object.assign(Object.create(Renderer.prototype), {
      sourceDebris: visual,
      destroyedPlacements: new Map(),
      gameplayDamageKeys: new Map(),
      gameplayEffects: new Map(),
      geometryVersion: 7,
      gameplayAnimationRevision: 0,
      shadowDirty: false,
      planar: {
        invalidate() {
          invalidations++;
        },
      },
    }) as any;
    const empty = { destroyed: [], debris: [], rubble: [], effects: [] };
    renderer.resetGameplay("day");
    renderer.setGameplayState("day", empty);
    const pristineRevision = visual.staticRevision;
    assert.equal(renderer.geometryVersion, 7);
    assert.equal(renderer.shadowDirty, false);
    assert.equal(invalidations, 0);
    assert.equal(visual.prepare("day", [10]), true);
    const internals = visual as any;
    const saved = {
      nodes: internals.objects.get("day"),
      groups: internals.bySource.get("day"),
      cells: internals.cells,
      cell: internals.cells.get("day:10"),
      masks: internals.fineMasks.get("day"),
      standing: internals.standing,
      moving: internals.moving,
      rubble: internals.rubble,
    };
    renderer.setGameplayState("day", { ...empty, destroyed: [10] });
    assert.equal(visual.statsSnapshot.maskedNodes, 1);
    assert(visual.statsSnapshot.standingTriangles > 0);
    const damagedGeneration = renderer.geometryVersion;
    const beforeUploads = uploads,
      beforeCreates = creates;
    renderer.shadowDirty = false;
    renderer.resetGameplay("day");
    assert.equal(renderer.geometryVersion, damagedGeneration + 1);
    assert.equal(renderer.shadowDirty, true);
    assert.equal(visual.statsSnapshot.maskedNodes, 0);
    assert.equal(visual.statsSnapshot.standingTriangles, 0);
    assert.equal(uploads, beforeUploads);
    assert.equal(creates, beforeCreates);
    for (const key of ["standing", "moving", "rubble"] as const)
      assert.equal(internals[key], saved[key]);
    assert.equal(internals.objects.get("day"), saved.nodes);
    assert.equal(internals.bySource.get("day"), saved.groups);
    assert.equal(internals.cells, saved.cells);
    assert.equal(internals.cells.get("day:10"), saved.cell);
    assert.equal(internals.fineMasks.get("day"), saved.masks);
    assert.throws(() => visual.fragmentMasks("day", [10]), /Prepare/);
    const restoredRevision = visual.staticRevision;
    assert(restoredRevision > pristineRevision);
    const restoredGeneration = renderer.geometryVersion;
    renderer.shadowDirty = false;
    renderer.setGameplayState("day", empty);
    renderer.resetGameplay("day");
    renderer.setGameplayState("day", empty);
    assert.equal(visual.staticRevision, restoredRevision);
    assert.equal(renderer.geometryVersion, restoredGeneration);
    assert.equal(renderer.shadowDirty, false);
    assert.equal(uploads, beforeUploads);
    assert.equal(creates, beforeCreates);
    // A spell must prepare again, but may reuse the same immutable fragments.
    assert.equal(visual.prepare("day", [10]), true);
    assert.equal(internals.cells.get("day:10"), saved.cell);
    assert(visual.fragmentMasks("day", [10])[0].mask.some(Boolean));
    // A genuinely changed destruction map cannot retain those fragments.
    visual.setObjects("day", [objects[0]]);
    assert.equal(internals.cells.has("day:10"), false);
    assert.equal(internals.fineMasks.has("day"), false);
    assert.equal(visual.prepare("day", [10]), false);
    assert.match(visual.limitReason, /incomplete destruction map/);
    visual.dispose();
  } finally {
    if (old) Object.defineProperty(globalThis, "GPUBufferUsage", old);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("restore cancels an invisible preparation without changing source identity or publishing fragments", async () => {
  const saved = ["GPUBufferUsage", "scheduler"].map((key) =>
    Object.getOwnPropertyDescriptor(globalThis, key),
  );
  const oldNow = Object.getOwnPropertyDescriptor(performance, "now");
  let time = 0,
    release!: () => void,
    yields = 0;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  Object.defineProperty(globalThis, "scheduler", {
    value: {
      yield() {
        yields++;
        return gate;
      },
    },
    configurable: true,
  });
  Object.defineProperty(performance, "now", {
    value: () => (time += 3),
    configurable: true,
  });
  try {
    const { scene, source } = fixture();
    const visual = new SourceDebris(
      {
        createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
        queue: { writeBuffer() {} },
      } as unknown as GPUDevice,
      new Map([["day", scene]]),
      source,
    );
    visual.setObjects("day", [
      { id: 10, sourceId: 0, min: [-1, -1, -1], max: [1, 1, 1] },
    ]);
    const nodes = (visual as any).objects.get("day");
    const pending = visual.prepareAsync("day", [10]);
    assert.equal(yields, 1);
    assert.equal(visual.resetPlayState("day"), false);
    release();
    assert.equal(await pending, false);
    assert.equal((visual as any).objects.get("day"), nodes);
    assert.equal((visual as any).cells.size, 0);
    assert.equal(visual.statsSnapshot.maskedNodes, 0);
    assert.throws(() => visual.update("day", [10], []), /Prepare/);
    assert.equal(visual.prepare("day", [10]), true);
    visual.dispose();
  } finally {
    for (const [index, key] of ["GPUBufferUsage", "scheduler"].entries()) {
      if (saved[index]) Object.defineProperty(globalThis, key, saved[index]!);
      else Reflect.deleteProperty(globalThis, key);
    }
    if (oldNow) Object.defineProperty(performance, "now", oldNow);
    else Reflect.deleteProperty(performance, "now");
  }
});

test("effect-only restore clears effects without invalidating static geometry or shadows", () => {
  const renderer = Object.assign(Object.create(Renderer.prototype), {
    destroyedPlacements: new Map(),
    gameplayDamageKeys: new Map([["day", ":"]]),
    gameplayEffects: new Map([
      ["day", [{ id: 1 }]],
      ["night", [{ id: 2 }]],
    ]),
    geometryVersion: 9,
    gameplayAnimationRevision: 3,
    shadowDirty: false,
    planar: {
      invalidate() {
        assert.fail("Unchanged geometry is still valid.");
      },
    },
  }) as any;
  renderer.resetGameplay("day");
  assert.equal(renderer.gameplayEffects.has("day"), false);
  assert.equal(renderer.gameplayEffects.has("night"), true);
  assert.equal(renderer.gameplayAnimationRevision, 4);
  assert.equal(renderer.geometryVersion, 9);
  assert.equal(renderer.shadowDirty, false);
  renderer.resetGameplay("day");
  assert.equal(renderer.gameplayAnimationRevision, 4);
});
test("debris color passes select pipelines only for visible included materials before their binding", () => {
  const old = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  try {
    const { scene, source } = fixture();
    scene.materials.push({} as CompiledScene["materials"][number]);
    scene.meshes[0].primitives = [0, 1].map((material) => ({
      firstIndex: material * 18,
      indexCount: 18,
      baseVertex: 0,
      material,
      materialSlot: material,
    }));
    const device = {
      createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
      queue: { writeBuffer() {} },
    } as unknown as GPUDevice;
    const visual = new SourceDebris(device, new Map([["day", scene]]), source);
    visual.setObjects("day", [
      { id: 10, sourceId: 0, min: [-1, -1, -1], max: [0, 1, 1] },
      { id: 11, sourceId: 0, min: [0, -1, -1], max: [1, 1, 1] },
    ]);
    assert.equal(visual.prepare("day", [10]), true);
    const body: GameplayDebris = {
      id: 1,
      nodeId: 10,
      sourceId: 0,
      chunk: 0,
      subdivisions: 4,
      sourceMin: [-1, -1, -1],
      sourceMax: [0, 1, 1],
      position: [0, 0, 0],
      rotation: [0, 0, 0],
      halfExtents: [0.125, 0.25, 0.25],
      velocity: [0, 0, 0],
      age: 0,
      life: 20,
    };
    visual.update("day", [10], [body]);
    const materials = [0, 1].map(
      (id) => ({ material: id }) as unknown as GPUBindGroup,
    );
    const pipelines = [0, 1].map(
      (id) => ({ material: id }) as unknown as GPURenderPipeline,
    );
    const events: Array<[string, unknown]> = [];
    const pass = {
      setVertexBuffer() {},
      setIndexBuffer() {},
      setPipeline(value: GPURenderPipeline) {
        events.push(["pipeline", pipelines.indexOf(value)]);
      },
      setBindGroup(_group: number, value: GPUBindGroup) {
        events.push(["material", materials.indexOf(value)]);
      },
      drawIndexed(...args: number[]) {
        events.push(["draw", args]);
      },
    } as unknown as GPURenderPassEncoder;
    const choose = (material: number) => {
      events.push(["select", material]);
      return pipelines[material];
    };
    for (const draw of [
      visual.drawStanding.bind(visual),
      visual.drawMoving.bind(visual),
    ]) {
      events.length = 0;
      draw(pass, materials, {} as GPUBuffer, () => true, undefined, choose);
      const colored = structuredClone(events);
      assert.ok(events.some(([kind]) => kind === "draw"));
      for (let index = 0; index < events.length; index++)
        if (events[index][0] === "material") {
          assert.deepEqual(events[index - 2], ["select", events[index][1]]);
          assert.deepEqual(events[index - 1], ["pipeline", events[index][1]]);
        }
      events.length = 0;
      draw(pass, materials, {} as GPUBuffer, () => true);
      assert.deepEqual(
        events,
        colored.filter(([kind]) => kind !== "select" && kind !== "pipeline"),
        "The optional pipeline selection must preserve original bindings and draw order.",
      );
      events.length = 0;
      draw(pass, materials, {} as GPUBuffer, () => false, undefined, choose);
      assert.deepEqual(
        events,
        [],
        "Excluded materials must not select a pipeline.",
      );
    }
    visual.update("day", [10], [{ ...body, position: [100, 0, 0] }]);
    events.length = 0;
    visual.drawMoving(
      pass,
      materials,
      {} as GPUBuffer,
      () => true,
      new Float32Array(modelMatrix([0, 0, 0])),
      choose,
    );
    assert.deepEqual(
      events,
      [],
      "Offscreen fragments must not select a pipeline.",
    );
    visual.dispose();
  } finally {
    if (old) Object.defineProperty(globalThis, "GPUBufferUsage", old);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("async preparation can cancel inside a single large cell without activating a partial candidate", async () => {
  const oldUsage = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUBufferUsage",
  );
  const oldNow = Object.getOwnPropertyDescriptor(performance, "now");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  let time = 0;
  Object.defineProperty(performance, "now", {
    value: () => ++time,
    configurable: true,
  });
  try {
    const { scene, source } = fixture();
    const repeated = {
      ...source,
      indices: new Uint32Array(
        Array.from({ length: 24 }, () => [...source.indices]).flat(),
      ),
    };
    scene.meshes[0].primitives[0].indexCount = repeated.indices.length;
    let uploads = 0;
    const device = {
      createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
      queue: {
        writeBuffer() {
          uploads++;
        },
      },
    } as unknown as GPUDevice;
    const visual = new SourceDebris(
      device,
      new Map([["day", scene]]),
      repeated,
    );
    visual.setObjects("day", [
      { id: 0, sourceId: 0, min: [-1, -1, -1], max: [1, 1, 1] },
    ]);
    const before = visual.statsSnapshot,
      priorUploads = uploads;
    let cancelled = false;
    const cancel = setTimeout(() => {
      cancelled = true;
    }, 0);
    try {
      assert.equal(
        await visual.prepareAsync("day", [0], () => cancelled),
        false,
      );
    } finally {
      clearTimeout(cancel);
    }
    assert.equal(uploads, priorUploads);
    assert.deepEqual(visual.statsSnapshot, before);
    assert.throws(
      () => visual.update("day", [0], []),
      /Prepare the destruction visuals/,
    );
    visual.dispose();
  } finally {
    if (oldUsage) Object.defineProperty(globalThis, "GPUBufferUsage", oldUsage);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
    if (oldNow) Object.defineProperty(performance, "now", oldNow);
    else Reflect.deleteProperty(performance, "now");
  }
});

test("spell geometry is bounded, deterministic at fixed age, and expires without display-rate integration", () => {
  const effects: GameplayEffect[] = [
    {
      id: 1,
      kind: "beam",
      origin: [0, 1, 0],
      target: [0, 1, -30],
      age: 0.12,
      life: 0.5,
      radius: 1.8,
    },
    {
      id: 2,
      kind: "lightning",
      origin: [2, 60, -5],
      target: [2, 0, -5],
      age: 0.2,
      life: 0.5,
      radius: 3.4,
    },
    {
      id: 3,
      kind: "meteor",
      origin: [20, 40, -20],
      target: [0, 0, -5],
      age: 0.5,
      life: 1,
      radius: 4,
    },
    {
      id: 4,
      kind: "impact",
      origin: [0, 0, -5],
      target: [0, 1, -5],
      age: 0.2,
      life: 1.3,
      radius: 4,
    },
  ];
  validateGameplayVisualState({ destroyed: [], debris: [], effects });
  const sampler = new GameplayEffectSampler(),
    eye: V3 = [0, 2, 4],
    forward: V3 = [0, 0, -1];
  const first = sampler.sample(effects, eye, forward);
  assert(first.count > 100);
  assert(first.count <= GAMEPLAY_VISUAL_LIMITS.vertices);
  const copy = first.data.slice();
  sampler.sample(
    effects.map((e) => ({ ...e, age: e.age * 0.5 })),
    eye,
    forward,
  );
  assert.deepEqual(sampler.sample(effects, eye, forward).data, copy);
  assert.equal(
    sampler.sample(
      effects.map((e) => ({ ...e, age: e.life })),
      eye,
      forward,
    ).count,
    0,
  );
  const lights = gameplayLights(effects);
  assert(lights.length <= 4);
  assert(lights.every((l) => l.enabled && !l.castShadows));
  assert.equal(
    gameplayLights(effects.map((e) => ({ ...e, age: e.life }))).length,
    0,
  );
  assert.throws(() =>
    validateGameplayVisualState({
      destroyed: [],
      debris: [],
      effects: [{ ...effects[0], radius: NaN }],
    }),
  );
  assert.throws(() =>
    validateGameplayVisualState({ destroyed: [1, 1], debris: [], effects: [] }),
  );
  assert.deepEqual(
    maskDestroyedPlacements(new Uint8Array([1, 1, 1, 0]), new Set([1, 3])),
    new Uint8Array([1, 0, 1, 0]),
  );
});

test("wall impacts anchor their ring and light at the actual surface point", () => {
  const impact: GameplayEffect = {
    id: 12,
    kind: "impact",
    origin: [2, 3, -5],
    target: [3, 3, -5],
    age: 0.2,
    life: 1.3,
    radius: 4,
  };
  const sample = new GameplayEffectSampler().sample(
    [impact],
    [5, 3, 0],
    [0, 0, -1],
  );
  for (let vertex = 0; vertex < 6; vertex++)
    assert(Math.abs(sample.data[vertex * 10] - 2.08) < 1e-6);
  assert.deepEqual(gameplayLights([impact])[0].position, [2.7, 3, -5]);
});

test("explicit complete flag groups prepare without blocking; partial groups and cancelled plans stay inactive", async () => {
  const old = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  let uploads = 0;
  const device = {
    createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
    queue: {
      writeBuffer() {
        uploads++;
      },
    },
  } as unknown as GPUDevice;
  try {
    const { scene, source } = fixture();
    scene.meshes[0].name = "SM_flag_02";
    const objects = Array.from({ length: 512 }, (_, id) => {
      const p = [id % 8, Math.floor(id / 8) % 8, Math.floor(id / 64)];
      return {
        id,
        sourceId: 0,
        min: p.map((v) => -1 + v / 4) as V3,
        max: p.map((v) => -1 + (v + 1) / 4) as V3,
      };
    });
    const groups = { castle: objects.map((object) => object.id) };
    const visual = new SourceDebris(
      device,
      new Map([
        ["day", scene],
        ["night", scene],
      ]),
      source,
    );
    visual.setObjects("day", objects, groups);
    assert.throws(
      () => visual.castleFragmentMasks("day"),
      /Prepare all castle/,
    );
    const before = uploads;
    let cancelled = false;
    const cancelTimer = setTimeout(() => {
      cancelled = true;
    }, 0);
    assert.equal(
      await visual.prepareAsync("day", groups.castle, () => cancelled),
      false,
    );
    clearTimeout(cancelTimer);
    assert.equal(uploads, before);
    assert.equal(visual.statsSnapshot.maskedNodes, 0);
    assert.throws(
      () => visual.update("day", [0], []),
      /Prepare the destruction visuals/,
    );
    let heartbeats = 0;
    const timer = setInterval(() => {
      visual.update("day", [], []);
      heartbeats++;
    }, 0);
    try {
      assert.equal(await visual.prepareAsync("day", groups.castle), true);
    } finally {
      clearInterval(timer);
    }
    assert(
      heartbeats > 0,
      "World updates must run while castle geometry is prepared.",
    );
    const masks = visual.castleFragmentMasks("day");
    assert.equal(masks.length, objects.length);
    assert(
      masks.every(
        (mask) =>
          mask.length === 2 &&
          mask.every(
            (word) => Number.isInteger(word) && word >= 0 && word <= 0xffffffff,
          ),
      ),
    );
    assert(
      masks.some((mask) => mask.every((word) => word === 0)),
      "Interior cells must not create invisible rubble colliders.",
    );
    assert(masks.some((mask) => mask[1] > 0));
    assert.equal(uploads, before);
    visual.update("day", groups.castle, []);
    assert.equal(visual.statsSnapshot.standingTriangles, 0);
    assert.equal(visual.statsSnapshot.maskedNodes, objects.length);
    const rubble: GameplayDebris[] = [];
    for (const object of objects) {
      const mask = masks[object.id];
      for (let chunk = 0; chunk < 64; chunk++) {
        if (!((mask[chunk >>> 5] >>> (chunk & 31)) & 1)) continue;
        rubble.push({
          id: object.id * 64 + chunk,
          nodeId: object.id,
          subdivisions: 4,
          sourceId: object.sourceId,
          chunk,
          sourceMin: object.min,
          sourceMax: object.max,
          position: object.min.map(
            (v, axis) =>
              v +
              ((2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1) + 0.5) /
                4) *
                (object.max[axis] - v),
          ) as V3,
          rotation: [0, 0, 0],
          halfExtents: object.min.map(
            (v, axis) => (object.max[axis] - v) / 8,
          ) as V3,
          velocity: [0, 0, 0],
          age: 20,
          life: 20,
        });
        if (rubble.length === GAMEPLAY_VISUAL_LIMITS.rubble) break;
      }
      if (rubble.length === GAMEPLAY_VISUAL_LIMITS.rubble) break;
    }
    assert.equal(rubble.length, 1024);
    validateGameplayVisualState({
      destroyed: groups.castle,
      debris: [],
      effects: [],
      castle: { rubble },
    });
    visual.update("day", groups.castle, [], rubble);
    assert.equal(visual.statsSnapshot.rubbleCount, 1024);
    const uploaded = uploads;
    visual.update("day", groups.castle, [], rubble);
    assert.equal(
      uploads,
      uploaded,
      "Settled rubble must not upload geometry each frame.",
    );
    assert.throws(
      () => visual.setObjects("day", objects, { castle: [0] }),
      /complete source pieces/,
    );
    visual.setObjects("day", objects, groups);
    assert.equal(visual.statsSnapshot.maskedNodes, 0);
    assert.equal(await visual.prepareAsync("day", groups.castle), true);
    visual.setObjects("night", objects, groups);
    assert.throws(
      () => visual.castleFragmentMasks("day"),
      /Prepare all castle/,
    );
    assert.equal(await visual.prepareAsync("night", groups.castle), true);
    visual.dispose();
  } finally {
    if (old) Object.defineProperty(globalThis, "GPUBufferUsage", old);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("all buildings use fine fragments and shared rubble without requiring a castle group", () => {
  const old = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    value: { VERTEX: 1, INDEX: 2, COPY_DST: 4 },
    configurable: true,
  });
  const device = {
    createBuffer: ({ size }: { size: number }) => ({ size, destroy() {} }),
    queue: { writeBuffer() {} },
  } as unknown as GPUDevice;
  try {
    const { scene, source } = fixture();
    const visual = new SourceDebris(device, new Map([["day", scene]]), source);
    const objects = [
      { id: 0, sourceId: 0, min: [-1, -1, -1] as V3, max: [0, 1, 1] as V3 },
      { id: 1, sourceId: 0, min: [0, -1, -1] as V3, max: [1, 1, 1] as V3 },
    ];
    visual.setObjects("day", objects);
    assert(visual.prepare("day", [0]));
    const entries = visual.fragmentMasks("day", [0]);
    assert.equal(entries[0].nodeId, 0);
    const mask = entries[0].mask;
    const chunk = Array.from({ length: 64 }, (_, i) => i).find(
      (i) => i >= 32 && (mask[i >>> 5] >>> (i & 31)) & 1,
    )!;
    assert(Number.isInteger(chunk));
    const fine: GameplayDebris = {
      id: 1,
      nodeId: 0,
      sourceId: 0,
      subdivisions: 4,
      chunk,
      sourceMin: objects[0].min,
      sourceMax: objects[0].max,
      halfExtents: [0.125, 0.25, 0.25],
      position: objects[0].min.map(
        (v, axis) =>
          v +
          ((2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1) + 0.5) /
            4) *
            (objects[0].max[axis] - v),
      ) as V3,
      rotation: [0, 0, 0],
      velocity: [0, 0, 0],
      age: 1,
      life: 20,
    };
    visual.update("day", [0], [fine]);
    assert(visual.statsSnapshot.debrisTriangles > 0);
    assert.equal(visual.statsSnapshot.standingCells, 1);
    const state = { destroyed: [0], debris: [], effects: [], rubble: [fine] };
    validateGameplayVisualState(state);
    visual.update("day", state.destroyed, state.debris, state.rubble);
    assert.equal(visual.statsSnapshot.rubbleCount, 1);
    assert.throws(
      () => visual.fragmentMasks("day", [1]),
      /Prepare the source fragments/,
    );
    assert.throws(
      () => visual.fragmentMasks("day", [0, 0]),
      /Prepare the source fragments/,
    );
    entries[0].mask[0] = 0;
    entries[0].mask[1] = 0;
    assert(visual.fragmentMasks("day", [0])[0].mask.some((word) => word > 0));
    visual.update("day", [0], []);
    assert(visual.prepare("day", []));
    assert(visual.prepare("day", [0]));
    assert(
      visual.fragmentMasks("day", [0])[0].mask.some((word) => word > 0),
      "Expired geometry can be evicted without forgetting exact masks.",
    );
    assert.throws(
      () =>
        visual.update("day", [0], [{ ...fine, halfExtents: [0.25, 0.5, 0.5] }]),
      /destroyed source cell/,
    );
    visual.dispose();
  } finally {
    if (old) Object.defineProperty(globalThis, "GPUBufferUsage", old);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("siege projectiles scale their visible head and trail within the fixed effect buffer", () => {
  const sampler = new GameplayEffectSampler();
  const meteor: GameplayEffect = {
    id: 9,
    kind: "meteor",
    origin: [0, 100, 0],
    target: [0, 0, 0],
    age: 0.5,
    life: 1,
    radius: 4,
  };
  const ordinary = sampler.sample([meteor], [0, 50, 10], [0, 0, -1]);
  const ordinaryWidth = Math.max(
    ...ordinary.data.filter((_, index) => index % 10 === 0),
  );
  const count = ordinary.count;
  const siege = sampler.sample(
    [{ ...meteor, radius: 24 }],
    [0, 50, 10],
    [0, 0, -1],
  );
  const siegeWidth = Math.max(
    ...siege.data.filter((_, index) => index % 10 === 0),
  );
  assert.equal(siege.count, count);
  assert(siege.count <= GAMEPLAY_VISUAL_LIMITS.vertices);
  assert(Math.abs(siegeWidth / ordinaryWidth - 6) < 1e-5);
});

test("64 overlapping siege and ordinary effects fit the unchanged vertex slab", () => {
  const effects: GameplayEffect[] = Array.from({ length: 64 }, (_, id) => ({
    id,
    kind: "lightning",
    origin: [id, 100, 0],
    target: [id, 0, 0],
    age: 0.1,
    life: 1,
    radius: 24,
  }));
  validateGameplayVisualState({ destroyed: [], debris: [], effects });
  const sample = new GameplayEffectSampler().sample(
    effects,
    [0, 5, 10],
    [0, 0, -1],
  );
  assert.equal(sample.count, 64 * 240);
  assert(sample.count < GAMEPLAY_VISUAL_LIMITS.vertices);
});
