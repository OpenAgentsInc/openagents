import assert from "node:assert/strict";
import { test } from "node:test";
import {
  WindShadowPlanner,
  compactWindFrames,
  expandWindBounds,
  WIND_SHADOW_LIMITS,
  type WindShadowView,
} from "../packages/renderer/wind-shadow-plan.ts";
import {
  buildInstanceBounds,
  compactInstanceDraws,
  frustumVisibility,
} from "../packages/renderer/culling.ts";
import { modelMatrix, orthographic } from "../packages/renderer/math.ts";
const view = (
  id: string,
  dynamicCasters: number[] = [],
  extra: Partial<WindShadowView> = {},
): WindShadowView => ({
  id,
  width: 512,
  height: 512,
  projection: modelMatrix([0, 0, 0]),
  staticRevision: "day:geometry1:alpha1:shader1:partition1",
  animationRevision: "time0:wind1",
  dynamicCasters: Uint32Array.from(dynamicCasters),
  ...extra,
});
const active = (plan: ReturnType<WindShadowPlanner["plan"]>) =>
  plan.views.filter(
    (v) => v.renderStatic || v.restoreStatic || v.dynamicCasterIndices.length,
  );
test("only moving views allocate copies and animate; fixed-time replay and unaffected views do no work", () => {
  const planner = new WindShadowPlanner();
  let inputs = [
      view("near", [3, 8]),
      view("far"),
      view("local0", [8]),
      view("local1"),
    ],
    plan = planner.plan(inputs);
  assert.deepEqual(
    plan.allocations.map((v) => v.id),
    ["near", "local0"],
  );
  assert.equal(plan.staticCacheBytes, 2 * 512 * 512 * 4);
  assert.equal(active(plan).length, 4);
  assert.equal(plan.views[0].renderStatic, "cache");
  assert.equal(plan.views[1].renderStatic, "main");
  planner.commit(plan);
  plan = planner.plan(inputs);
  assert.equal(active(plan).length, 0);
  planner.commit(plan);
  inputs = inputs.map((v) => ({ ...v, animationRevision: "time1:wind1" }));
  plan = planner.plan(inputs);
  assert.deepEqual(
    active(plan).map((v) => v.id),
    ["near", "local0"],
  );
  assert.ok(active(plan).every((v) => !v.renderStatic && v.restoreStatic));
  assert.equal(plan.allocations.length, 0);
  planner.commit(plan);
  inputs[0] = view("near", [], { animationRevision: "time2:wind1" });
  plan = planner.plan(inputs);
  assert.equal(plan.views[0].restoreStatic, true);
  assert.equal(plan.views[0].renderStatic, false);
  assert.equal(plan.views[0].dynamicCasterIndices.length, 0);
  planner.commit(plan);
  assert.equal(active(planner.plan(inputs)).length, 0);
});
test("projection, mode/partition and geometry changes rebuild only the relevant static copy", () => {
  const planner = new WindShadowPlanner(),
    original = [view("near", [1]), view("local0", [2])];
  planner.commit(planner.plan(original));
  const moved = [
      { ...original[0], projection: modelMatrix([1, 0, 0]) },
      original[1],
    ],
    plan = planner.plan(moved);
  assert.deepEqual(
    active(plan).map((v) => v.id),
    ["near"],
  );
  assert.equal(plan.views[0].renderStatic, "cache");
  planner.commit(plan);
  const changed = moved.map((v) => ({
    ...v,
    staticRevision: "night:geometry2:alpha1:shader1:partition2",
  }));
  assert.ok(
    planner.plan(changed).views.every((v) => v.renderStatic === "cache"),
  );
});
test("failed submissions do not publish cache validity; stale plans cannot overwrite newer state", () => {
  const planner = new WindShadowPlanner(),
    first = planner.plan([view("near", [1])]),
    retry = planner.plan([view("near", [1])]);
  assert.equal(retry.views[0].renderStatic, "cache");
  assert.equal(retry.allocations.length, 1);
  planner.commit(retry);
  assert.throws(() => planner.commit(first));
  const beforeReset = planner.plan([view("near", [1])]);
  planner.reset();
  assert.throws(() => planner.commit(beforeReset));
  assert.doesNotThrow(() => planner.commit(planner.plan([view("near", [1])])));
});
test("lazy copies remain within136MiB; removal/resize release resources without leaking old moving shadows", () => {
  const planner = new WindShadowPlanner(),
    all = [
      view("near", [1], { width: 4096, height: 4096 }),
      view("far", [1], { width: 2048, height: 2048 }),
      view("distant", [1], { width: 2048, height: 2048 }),
      ...Array.from({ length: 40 }, (_, i) => view("local" + i, [1])),
    ];
  let plan = planner.plan(all);
  assert.equal(plan.staticCacheBytes, 136 * 1024 * 1024);
  planner.commit(plan);
  assert.throws(() =>
    planner.plan(all.map((v) => ({ ...v, width: 4096, height: 4096 }))),
  );
  assert.throws(() => planner.plan([...all, view("extra")]));
  plan = planner.plan([view("near", [1], { width: 1024, height: 1024 })]);
  assert.equal(plan.releases.length, 43);
  assert.deepEqual(plan.allocations, [
    {
      id: "near",
      width: 1024,
      height: 1024,
      bytes: 1024 * 1024 * 4,
      replace: true,
    },
  ]);
  planner.commit(plan);
  assert.equal(planner.plan([]).releases[0], "near");
  assert.throws(() => planner.plan([view("dup"), view("dup")]));
  assert.throws(() => planner.plan([view("bad", [2, 2])]));
});
test("pivot/radius/up metadata follows exactly the same compacted source order as all shared primitive batches", () => {
  const models = new Float32Array(
      Array.from({ length: 5 }, (_, i) => modelMatrix([i, 0, 0])).flat(),
    ),
    frames = new Float32Array(
      Array.from({ length: 5 }, (_, i) => [
        i,
        10 + i,
        20 + i,
        0.5 + i,
        0,
        1,
        0,
        0,
      ]).flat(),
    ),
    visibility = Uint8Array.from([0, 1, 0, 1, 1]);
  const draws = [
      { firstInstance: 0, count: 5, material: 2, firstIndex: 0 },
      { firstInstance: 0, count: 5, material: 7, firstIndex: 99 },
    ],
    matrices = compactInstanceDraws(models, draws, visibility),
    data = compactWindFrames(frames, visibility);
  assert.deepEqual([...data.sourceIndices], [1, 3, 4]);
  assert.equal(data.frames.length / 8, matrices.models.length / 16);
  for (let i = 0; i < data.sourceIndices.length; i++)
    assert.equal(data.frames[i * 8], matrices.models[i * 16 + 12]);
  assert.deepEqual(
    matrices.draws.map((d) => [
      d.firstInstance,
      d.count,
      d.material,
      d.firstIndex,
    ]),
    [
      [0, 3, 2, 0],
      [0, 3, 7, 99],
    ],
  );
  assert.deepEqual([...frames.subarray(8, 16)], [1, 11, 21, 1.5, 0, 1, 0, 0]);
  assert.throws(() =>
    compactWindFrames(frames, Uint8Array.from([0, 2, 0, 1, 1])),
  );
});
test("deformation bounds prevent frustum popping and never alter undeformed source bounds", () => {
  const source = buildInstanceBounds({
      meshes: [{ bounds: { min: [-0.1, -0.1, -0.1], max: [0.1, 0.1, 0.1] } }],
      instances: [{ mesh: 0, model: modelMatrix([1.3, 0, -1]) }],
    } as any),
    before = source.aabbs.slice(),
    projection = orthographic(-1, 1, -1, 1, 0.1, 10);
  assert.equal(frustumVisibility(source, projection)[0], 0);
  const expanded = expandWindBounds(source, new Float64Array([0.3]));
  assert.equal(frustumVisibility(expanded, projection)[0], 1);
  assert.deepEqual(source.aabbs, before);
  assert.equal(expanded.spheres[3], source.spheres[3] + 0.3);
  assert.throws(() => expandWindBounds(source, new Float64Array([-1])));
  assert.equal(WIND_SHADOW_LIMITS.views, 43);
});
