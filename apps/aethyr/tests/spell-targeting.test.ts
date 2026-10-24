import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  add,
  direction,
  dot,
  lookAt,
  multiply,
  normalize,
  perspective,
  scale,
  sub,
  type V3,
} from "../packages/renderer/math.ts";
import {
  LOCKED_THIRD_PERSON_AIM_Y,
  pointerCameraRay,
  resolveSpellTarget,
  spellAimRadius,
  spellAimRange,
  type AimCamera,
  type SpellRay,
  type SpellTarget,
} from "../packages/renderer/spell-targeting.ts";
import {
  GameplayEffectSampler,
  GameplayEffects,
  type GameplayEffect,
} from "../packages/renderer/gameplay-effects.ts";
import { DEFAULT_METEOR_IMPACT_RECIPE } from "../packages/renderer/impact-recipe.ts";
import { RuntimeKernel } from "../packages/runtime/kernel.ts";
import { validDruidStatus } from "../packages/runtime/druid-abilities.ts";

const close = (a: number, b: number, tolerance = 0.00001) =>
  assert.ok(Math.abs(a - b) < tolerance, `${a} differs from ${b}`);
function projected(camera: AimCamera, aspect: number, point: V3) {
  const view = multiply(
    perspective(camera.fov ?? 1, aspect),
    lookAt(
      camera.eye,
      add(camera.eye, direction(camera.yaw, camera.pitch)),
      camera.up,
    ),
  );
  const clip = Array.from({ length: 4 }, (_, axis) =>
    point.reduce((sum, v, i) => sum + v * view[i * 4 + axis], view[12 + axis]),
  );
  return [clip[0] / clip[3], clip[1] / clip[3]];
}
const camera: AimCamera = { eye: [5, 3, 6], yaw: 0, pitch: 0 };
const plane =
  (z: number, xMin = -200, xMax = 200) =>
  (ray: SpellRay, maxDistance: number): SpellTarget | undefined => {
    if (Math.abs(ray.direction[2]) < 1e-8) return undefined;
    const distance = (z - ray.origin[2]) / ray.direction[2];
    const point = add(ray.origin, scale(ray.direction, distance));
    return distance > 0 &&
      distance <= maxDistance &&
      point[0] >= xMin &&
      point[0] <= xMax
      ? { point, normal: [0, 0, 1], distance }
      : undefined;
  };

test("pointer rays project back onto the cursor through the renderer's actual camera matrix", () => {
  const camera: AimCamera = {
    eye: [12, 7, 19],
    yaw: 0.8,
    pitch: -0.25,
    fov: 0.86,
    up: [0.1, 1, 0.2],
  };
  for (const aspect of [0.6, 1, 2.2])
    for (const [x, y] of [
      [0.63, -0.27],
      [-0.8, 0.9],
      [0, 0],
    ]) {
      const ray = pointerCameraRay(
        camera,
        aspect,
        { x, y, locked: false },
        true,
      );
      const screen = projected(
        camera,
        aspect,
        add(ray.origin, scale(ray.direction, 17)),
      );
      close(screen[0], x);
      close(screen[1], y);
      close(Math.hypot(...ray.direction), 1);
    }
});

test("locked third-person aim clears the avatar while first-person uses exact screen center", () => {
  for (const third of [false, true]) {
    const ray = pointerCameraRay(
      camera,
      16 / 9,
      { x: 0.7, y: -0.8, locked: true },
      third,
    );
    const screen = projected(
      camera,
      16 / 9,
      add(ray.origin, scale(ray.direction, 20)),
    );
    close(screen[0], 0);
    close(screen[1], third ? LOCKED_THIRD_PERSON_AIM_Y : 0);
  }
  assert.throws(() =>
    pointerCameraRay(camera, 0, { x: 0, y: 0, locked: false }, false),
  );
});

test("third-person parallax picks the visible wall and freezes a hand ray to that same point", () => {
  const viewRay = pointerCameraRay(
    camera,
    16 / 9,
    { x: 0, y: 0, locked: false },
    true,
  );
  const origin: V3 = [0, 1.6, 0];
  const result = resolveSpellTarget(viewRay, origin, 30.48, plane(-12));
  assert.ok(result);
  close(result.target.point[0], 5);
  close(result.target.point[1], 3);
  close(result.target.point[2], -12);
  const castHit = plane(-12)(result.ray, 30.48)!;
  castHit.point.forEach((v, i) => close(v, result.target.point[i]));
  viewRay.direction[0] = 1;
  origin[0] = 100;
  assert.deepEqual(result.ray.origin, [0, 1.6, 0]);
  close(plane(-12)(result.ray, 30.48)!.point[0], 5);
});

test("a wall between the hands and cursor target moves both marker and cast to the obstruction", () => {
  const query = (ray: SpellRay, distance: number) =>
    plane(-2, -1, 1)(ray, distance) ?? plane(-12)(ray, distance);
  const resolved = resolveSpellTarget(
    pointerCameraRay(camera, 16 / 9, { x: 0, y: 0, locked: false }, true),
    [0, 1.6, 0],
    30.48,
    query,
  );
  assert.ok(resolved);
  close(resolved.target.point[2], -2);
  assert.deepEqual(query(resolved.ray, 30.48), resolved.target);
});

test("ranges remain player-relative and camera queries respect the kernel's 80 m limit", () => {
  const ray: SpellRay = { origin: [0, 3, 40], direction: [0, 0, -1] };
  const queries: number[] = [];
  const resolved = resolveSpellTarget(
    ray,
    [0, 3, 0],
    80,
    (ray, maxDistance) => {
      queries.push(maxDistance);
      return plane(-70)(ray, maxDistance);
    },
  );
  assert.ok(resolved);
  close(resolved.target.distance, 70);
  assert.deepEqual(
    queries.slice(0, -1),
    [20.02, 20.02, 20.02, 20.02, 20.02, 20],
  );
  close(queries.at(-1)!, 70.02);
  assert.equal(
    resolveSpellTarget(ray, [0, 3, 0], 30.48, plane(-70)),
    undefined,
  );
  assert.equal(
    resolveSpellTarget(ray, [0, 3, 0], 80, () => undefined),
    undefined,
  );
  const nearEyeQueries: number[] = [];
  assert.equal(
    resolveSpellTarget(
      { origin: [0, 1.7, 0], direction: [0, 0, -1] },
      [0, 1.62, 0],
      80,
      (_ray, distance) => {
        nearEyeQueries.push(distance);
        return undefined;
      },
    ),
    undefined,
  );
  assert.deepEqual(nearEyeQueries, [20.02, 20.02, 20.02, 20.02, 0.1]);
  assert.equal(spellAimRange("lightning_bolt"), 30.48);
  assert.equal(spellAimRange("meteor_shower"), 80);
  assert.equal(spellAimRadius("meteor_shower"), 4);
  assert.equal(spellAimRadius("thunderbolt"), 3.4);
});

test("target rings lie on ground or wall planes at the actual AoE radius without inflating spell counts", () => {
  const sampler = new GameplayEffectSampler();
  for (const normal of [[0, 1, 0], [1, 0, 0], normalize([1, 2, 3])] as V3[]) {
    const point: V3 = [2, 3, -4];
    const sample = sampler.sample([], [0, 5, 5], [0, 0, -1], {
      point,
      normal,
      radius: 4,
    });
    assert.equal(sample.count, 384);
    assert.equal(sample.effectCount, 0);
    const center = add(point, scale(normal, 0.035));
    for (let vertex = 0; vertex < sample.count; vertex++) {
      const position = Array.from(
        sample.data.subarray(vertex * 10, vertex * 10 + 3),
      ) as V3;
      const delta = sub(position, center);
      close(dot(delta, normal), 0);
      const radius = Math.hypot(...delta);
      assert.ok(
        Math.abs(radius - 4) < 0.00001 || Math.abs(radius - 3.91) < 0.00001,
      );
    }
  }
  assert.equal(sampler.sample([], [0, 5, 5], [0, 0, -1]).count, 0);
});

test("submitted target triangles are counted separately and disappear without a marker", async () => {
  const previous = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUBufferUsage",
  );
  const previousStage = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUShaderStage",
  );
  Object.defineProperty(globalThis, "GPUShaderStage", {
    configurable: true,
    value: { VERTEX: 1 },
  });
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { VERTEX: 1, COPY_DST: 2, UNIFORM: 4 },
  });
  const draws: number[] = [];
  const drawn: {
    pipeline: GPURenderPipelineDescriptor;
    count: number;
    first: number;
  }[] = [];
  let currentPipeline: GPURenderPipelineDescriptor;
  const device = {
    createBindGroupLayout() {
      return {};
    },
    createPipelineLayout() {
      return {};
    },
    createShaderModule() {
      return {};
    },
    async createRenderPipelineAsync(descriptor: GPURenderPipelineDescriptor) {
      return {
        descriptor,
        getBindGroupLayout() {
          return {};
        },
      };
    },
    createBuffer() {
      return { destroy() {} };
    },
    createBindGroup() {
      return {};
    },
    queue: { writeBuffer() {} },
  } as unknown as GPUDevice;
  const pass = {
    setPipeline(pipeline: any) {
      currentPipeline = pipeline.descriptor;
    },
    setBindGroup() {},
    setVertexBuffer() {},
    draw(count: number, _instances = 1, first = 0) {
      draws.push(count);
      drawn.push({ pipeline: currentPipeline, count, first });
    },
  } as unknown as GPURenderPassEncoder;
  try {
    const effects = await GameplayEffects.create(device);
    effects.render(pass, [], new Float32Array(16), [0, 3, 2], [0, 0, -1], {
      point: [0, 0, 0],
      normal: [0, 1, 0],
      radius: 4,
    });
    assert.deepEqual(draws, [384]);
    assert.equal(effects.aimTriangles, 128);
    assert.equal(effects.triangles, 0);
    effects.render(pass, [], new Float32Array(16), [0, 3, 2], [0, 0, -1]);
    assert.equal(effects.aimTriangles, 0);
    assert.equal(draws.length, 1);
    assert.equal(drawn[0].pipeline.depthStencil?.depthCompare, "less-equal");
    const marker = {
      point: [0, 0, 0] as V3,
      normal: [0, 1, 0] as V3,
      radius: 4,
      overlay: true,
    };
    const sampled = new GameplayEffectSampler().sample(
      [],
      [0, 3, 2],
      [0, 0, -1],
      marker,
    );
    const radii = Array.from({ length: sampled.count }, (_, i) =>
      Math.hypot(sampled.data[i * 10], sampled.data[i * 10 + 2]),
    );
    close(Math.min(...radii), 3.84);
    close(Math.max(...radii), 4);
    effects.render(
      pass,
      [],
      new Float32Array(16),
      [0, 3, 2],
      [0, 0, -1],
      marker,
    );
    assert.equal(drawn.at(-1)?.pipeline.depthStencil?.depthCompare, "always");
    assert.equal(drawn.at(-1)?.pipeline.depthStencil?.depthWriteEnabled, false);
    assert.equal(effects.aimTriangles, 128);
    assert.equal(effects.triangles, 0);
    const impact: GameplayEffect = {
      id: 1,
      kind: "impact",
      origin: [0, 0, 0],
      target: [0, 1, 0],
      age: 0.2,
      life: 4,
      radius: 4,
      impactRecipe: {
        event: {
          id: 1,
          position: [0, 0, 0],
          normal: [0, 1, 0],
          intensity: 1,
          radius: 4,
          seed: 7,
          startTime: 0,
        },
        recipe: DEFAULT_METEOR_IMPACT_RECIPE,
      },
    };
    const before = drawn.length;
    effects.render(
      pass,
      [impact],
      new Float32Array(16),
      [0, 3, 2],
      [0, 0, -1],
      marker,
    );
    const mixed = drawn.slice(before);
    assert.equal(mixed.length, 3);
    assert.equal(mixed[0].pipeline.fragment?.entryPoint, "fs");
    assert.equal(mixed[0].pipeline.depthStencil?.depthCompare, "less-equal");
    assert.equal(mixed[1].pipeline.fragment?.entryPoint, "fsSmoke");
    assert.equal(mixed[1].pipeline.depthStencil?.depthCompare, "less-equal");
    assert.equal(mixed[2].pipeline.depthStencil?.depthCompare, "always");
    assert.equal(mixed[2].count, 384);
    assert.equal(effects.aimTriangles, 128);
    assert.equal(effects.triangles, (mixed[0].count + mixed[1].count) / 3);
    effects.dispose();
    const summary = {
      enabled: true,
      selected: "meteor_shower",
      destroyed: 0,
      debris: 0,
      effects: 0,
      casts: 0,
      impacts: 0,
      tick: 0,
    };
    assert(validDruidStatus({ ...summary, rendered: { aimTriangles: 128 } }));
    assert(!validDruidStatus({ ...summary, rendered: { aimTriangles: 129 } }));
  } finally {
    if (previous) Object.defineProperty(globalThis, "GPUBufferUsage", previous);
    else delete (globalThis as { GPUBufferUsage?: unknown }).GPUBufferUsage;
    if (previousStage)
      Object.defineProperty(globalThis, "GPUShaderStage", previousStage);
    else delete (globalThis as { GPUShaderStage?: unknown }).GPUShaderStage;
  }
});

test("current Rust/Wasm prepare and cast use the same wall point shown by cursor targeting", async () => {
  const bytes = await readFile(
    new URL("../public/kernel.wasm", import.meta.url),
  );
  for (const ability of [
    "lightning_bolt",
    "meteor_shower",
    "thunderbolt",
  ] as const) {
    const kernel = await RuntimeKernel.fromBytes(bytes);
    const request = (message: object) => {
      const value = kernel.request<any>(message);
      assert.equal(value.ok, true, JSON.stringify(value));
      return value;
    };
    request({
      type: "init",
      spawn: { position: [0, 0, 0], yaw: 0, pitch: 0, mode: "walk" },
      entities: [],
    });
    request({
      type: "set_static_collision",
      triangles: [
        [
          [-100, -100, -20],
          [100, -100, -20],
          [-100, 100, -20],
        ],
        [
          [100, -100, -20],
          [100, 100, -20],
          [-100, 100, -20],
        ],
      ],
    });
    request({ type: "gameplay_init", objects: [], seed: 12999 });
    const resolved = resolveSpellTarget(
      pointerCameraRay(camera, 16 / 9, { x: 0.2, y: 0.1, locked: false }, true),
      [0, 1.6, 0],
      spellAimRange(ability),
      (ray, maxDistance) =>
        request({ type: "gameplay_aim", ...ray, maxDistance }).aim ?? undefined,
    );
    assert.ok(resolved);
    assert.equal(resolved.target.nodeId, undefined);
    assert.equal(
      validDruidStatus({
        enabled: true,
        selected: ability,
        destroyed: 0,
        debris: 0,
        effects: 0,
        casts: 0,
        impacts: 0,
        tick: 0,
        target: resolved.target,
      }),
      true,
    );
    const cast = { castId: `cursor-${ability}`, ability, ...resolved.ray };
    const plan = request({ type: "gameplay_prepare_cast", ...cast });
    const applied = request({
      type: "gameplay_cast",
      ...cast,
      expectedVersion: plan.version,
      fragmentMasks: [],
    });
    applied.cast.aim.point.forEach((v: number, i: number) =>
      close(v, resolved.target.point[i], 0.00005),
    );
    assert.deepEqual(applied.player.position, [0, 0, 0]);
    assert.equal(applied.revision, 0);
  }
});
