import assert from "node:assert/strict";
import test from "node:test";
import {
  compileSourceBatches,
  createCompiledBatchGatherer,
  MAX_COMPILED_BATCH_BYTES,
  MAX_COMPILED_BATCH_MEMBERSHIPS,
  type SourceBatchDraw,
} from "../packages/renderer/compiled-source-batches.ts";

function draws() {
  const base = {
    firstIndex: 0,
    indexCount: 3,
    baseVertex: -2,
    material: 0,
    wind: false,
    rasterOrientation: "positive" as const,
    label: "original metadata",
  };
  return [
    { ...base, firstInstance: 0, count: 2, sourceDrawIndex: 10 },
    {
      ...base,
      firstIndex: 3,
      material: 1,
      firstInstance: 2,
      count: 1,
      sourceDrawIndex: 11,
    },
    { ...base, firstInstance: 3, count: 2, sourceDrawIndex: 12 },
    {
      ...base,
      firstIndex: 6,
      material: 2,
      firstInstance: 0,
      count: 1,
      sourceDrawIndex: 13,
    },
    { ...base, firstInstance: 5, count: 1, sourceDrawIndex: 14 },
    { ...base, firstInstance: 1, count: 1, sourceDrawIndex: 15 },
  ];
}
function source() {
  const models = new Float32Array(6 * 16),
    wind = new Float32Array(6 * 4);
  for (let id = 0; id < 6; id++) {
    models.set(
      [
        id % 2 ? -1 : 1,
        0,
        0,
        0,
        0,
        id + 1,
        0,
        0,
        0,
        0,
        0.5,
        0,
        id * 2.5,
        -id,
        -0,
        1,
      ],
      id * 16,
    );
    wind.set([id * 0.25, -0, id + 1, id * 0.5], id * 4);
  }
  return {
    models,
    wind,
    visibility: new Uint8Array(6).fill(1),
    sourceVersion: 1,
  };
}
function fixture() {
  const original = draws(),
    result = compileSourceBatches(original, 6, {
      eligible: (draw) => draw.material !== 2,
    });
  assert.equal(result.status, "compiled");
  if (result.status !== "compiled") throw Error("Fixture did not compile.");
  return {
    original,
    plan: result.plan,
    gatherer: createCompiledBatchGatherer(result.plan),
    source: source(),
  };
}
const bits = (values: Float32Array) =>
  Array.from(new Uint32Array(values.buffer, values.byteOffset, values.length));
test("compatible primitive uses merge within eligible segments and preserve special-draw barriers", () => {
  const f = fixture();
  assert.deepEqual(f.plan.counts, {
    sourceInstances: 6,
    sourceDraws: 6,
    groupedDraws: 4,
    eligibleDraws: 5,
    mergedDraws: 2,
    memberships: 8,
  });
  assert.deepEqual([...f.plan.inputDrawToGroup], [0, 1, 0, 2, 3, 3]);
  assert.deepEqual([...f.plan.sourceDrawIds], [10, 11, 12, 13, 14, 15]);
  assert.deepEqual([...f.plan.groupDrawOffsets], [0, 2, 3, 4, 6]);
  assert.deepEqual([...f.plan.groupInputDrawIndices], [0, 2, 1, 3, 4, 5]);
  assert.deepEqual([...f.plan.memberships], [0, 1, 3, 4, 2, 0, 5, 1]);
  assert.deepEqual([...f.plan.groupOffsets], [0, 4, 5, 6, 8]);
  assert.deepEqual(
    f.plan.baseDraws.map((draw) => [
      draw.sourceDrawIndex,
      draw.firstInstance,
      draw.count,
    ]),
    [
      [0, 0, 4],
      [1, 4, 1],
      [2, 5, 1],
      [3, 6, 2],
    ],
  );
  assert.deepEqual(f.original, draws());
  assert.equal(f.plan.baseDraws[0].label, "original metadata");
  assert.notEqual(f.plan.baseDraws[0], f.original[0]);
});
test("gather copies original model/wind bits and source IDs for every primitive use", () => {
  const f = fixture(),
    result = f.gatherer.gather(f.source);
  assert.equal(result.count, 8);
  assert.deepEqual([...result.sourceIndices], [0, 1, 3, 4, 2, 0, 5, 1]);
  for (let use = 0; use < result.count; use++) {
    const id = result.sourceIndices[use];
    assert.deepEqual(
      bits(result.models.subarray(use * 16, use * 16 + 16)),
      bits(f.source.models.subarray(id * 16, id * 16 + 16)),
    );
    assert.deepEqual(
      bits(result.wind.subarray(use * 4, use * 4 + 4)),
      bits(f.source.wind.subarray(id * 4, id * 4 + 4)),
    );
  }
  assert.equal(
    [...result.sourceIndices].filter((id) => id === 0).length,
    2,
    "One placement legitimately uses two primitives.",
  );
});
test("visibility and destroyed IDs filter every use before stable gather without modifying source data", () => {
  const f = fixture(),
    beforeModels = bits(f.source.models),
    beforeWind = bits(f.source.wind);
  f.source.visibility[1] = 0;
  const excluded = new Set([3]);
  const result = f.gatherer.gather({ ...f.source, excluded });
  assert.deepEqual([...result.sourceIndices], [0, 4, 2, 0, 5]);
  assert.deepEqual(
    result.groupedDraws.map((draw) => [draw.firstInstance, draw.count]),
    [
      [0, 2],
      [2, 1],
      [3, 1],
      [4, 1],
    ],
  );
  assert.deepEqual(bits(f.source.models), beforeModels);
  assert.deepEqual(bits(f.source.wind), beforeWind);
  assert.deepEqual([...excluded], [3]);
});
test("persistent slabs, output records and plan identity survive empty views and restore", () => {
  const f = fixture(),
    models = f.gatherer.models,
    wind = f.gatherer.wind,
    ids = f.gatherer.sourceIndices;
  const full = f.gatherer.gather(f.source),
    records = full.groupedDraws.slice(),
    fullBits = bits(full.models);
  const empty = f.gatherer.gather({
    ...f.source,
    visibility: new Uint8Array(6),
  });
  assert.equal(empty, full);
  assert.equal(empty.count, 0);
  assert.equal(empty.models.length, 0);
  assert.equal(empty.visibleDraws.length, 0);
  assert(empty.groupedDraws.every((draw) => !draw.count));
  const restored = f.gatherer.gather(f.source);
  assert.deepEqual(bits(restored.models), fullBits);
  assert.equal(f.gatherer.models, models);
  assert.equal(f.gatherer.wind, wind);
  assert.equal(f.gatherer.sourceIndices, ids);
  records.forEach((record, index) =>
    assert.equal(restored.groupedDraws[index], record),
  );
  assert.equal(f.plan.version, 1);
  assert.equal(f.gatherer.plan.identity, f.plan.identity);
  assert.deepEqual(
    f.plan.baseDraws.map((draw) => draw.count),
    [4, 1, 1, 2],
  );
});
test("geometry, material, winding and wind distinctions never merge", () => {
  const base = draws()[0];
  for (const patch of [
    { firstIndex: 9 },
    { indexCount: 6 },
    { baseVertex: 1 },
    { material: 1 },
    { rasterOrientation: "negative" as const },
    { rasterOrientation: undefined },
    { wind: true },
    { originalFirstIndex: 0 },
  ]) {
    const result = compileSourceBatches(
      [base, { ...base, ...patch, sourceDrawIndex: 11 }],
      6,
      { eligible: () => true },
    );
    assert.equal(result.status, "fallback");
    if (result.status === "fallback")
      assert.equal(result.reason, "no-reduction");
  }
});
test("unsupported and over-budget plans fall back before membership/gather allocations", () => {
  const base = draws();
  for (const [options, reason] of [
    [{ enabled: false }, "disabled"],
    [{ eligible: () => false }, "no-reduction"],
    [{ maxMemberships: 7 }, "membership-limit"],
    [{ maxBytes: 1 }, "byte-limit"],
  ] as const) {
    const result = compileSourceBatches(base, 6, {
      eligible: () => true,
      ...options,
    });
    assert.equal(result.status, "fallback");
    if (result.status === "fallback") assert.equal(result.reason, reason);
  }
  assert.throws(
    () =>
      compileSourceBatches(base, 6, {
        eligible: () => true,
        maxMemberships: MAX_COMPILED_BATCH_MEMBERSHIPS + 1,
      }),
    /storage limits/,
  );
  assert.throws(
    () =>
      compileSourceBatches(base, 6, {
        eligible: () => true,
        maxBytes: MAX_COMPILED_BATCH_BYTES + 1,
      }),
    /storage limits/,
  );
  assert.throws(
    () => createCompiledBatchGatherer({ capacity: 0xffffffff } as any),
    /compiled within/,
  );
});
test("exact typed-array footprint includes plan mappings and the membership-sized gather capacity", () => {
  const f = fixture();
  assert.equal(f.plan.capacity, 8);
  const planBytes = [
    f.plan.memberships,
    f.plan.groupOffsets,
    f.plan.inputDrawToGroup,
    f.plan.sourceDrawIds,
    f.plan.groupDrawOffsets,
    f.plan.groupInputDrawIndices,
  ].reduce((sum, values) => sum + values.byteLength, 0);
  assert.equal(f.plan.footprint.planBytes, planBytes);
  assert.equal(f.plan.footprint.gatherBytes, 8 * 84);
  assert.equal(f.plan.footprint.totalBytes, planBytes + f.gatherer.memoryBytes);
});
test("invalid draw ranges and identities reject without changing the input", () => {
  const base = draws()[0];
  for (const patch of [
    { firstInstance: 6 },
    { count: -1 },
    { count: 0xffffffff },
    { firstIndex: -1 },
    { indexCount: 4 },
    { baseVertex: 0x80000000 },
    { material: -1 },
    { wind: 1 },
    { rasterOrientation: "other" },
    { sourceDrawIndex: -1 },
  ]) {
    const input = [{ ...base, ...patch }] as SourceBatchDraw[],
      before = structuredClone(input);
    assert.throws(
      () => compileSourceBatches(input, 6, { eligible: () => true }),
      /complete bounded/,
    );
    assert.deepEqual(input, before);
  }
});
test("invalid gather input preserves all previously gathered records and slabs", () => {
  const f = fixture();
  const first = f.gatherer.gather(f.source),
    before = bits(first.models),
    ids = [...first.sourceIndices],
    records = structuredClone(first.groupedDraws);
  const invalidVisibility = new Uint8Array(6).fill(2),
    invalidModels = f.source.models.slice();
  invalidModels[15] = 0;
  for (const patch of [
    { models: new Float32Array() },
    { visibility: invalidVisibility },
    { excluded: new Set([6]) },
    { models: invalidModels },
    { sourceVersion: -1 },
  ]) {
    assert.throws(() => f.gatherer.gather({ ...f.source, ...patch }));
    assert.deepEqual(bits(first.models), before);
    assert.deepEqual([...first.sourceIndices], ids);
    assert.deepEqual(first.groupedDraws, records);
  }
});
test("source version changes validate and copy new transforms; absent wind clears old output", () => {
  const f = fixture();
  f.gatherer.gather(f.source);
  f.source.models[12] = 42;
  let result = f.gatherer.gather({ ...f.source, sourceVersion: 2 });
  assert.equal(result.models[12], 42);
  result = f.gatherer.gather({
    ...f.source,
    wind: undefined,
    sourceVersion: 3,
  });
  assert(result.wind.every((value) => value === 0));
  f.source.models[12] = Number.NaN;
  assert.throws(
    () => f.gatherer.gather({ ...f.source, sourceVersion: 4 }),
    /finite/,
  );
});
test("a wind-bearing group requires its original per-instance inputs", () => {
  const base = { ...draws()[0], wind: true };
  const compiled = compileSourceBatches(
    [base, { ...base, sourceDrawIndex: 11 }],
    6,
    { eligible: () => true },
  );
  assert.equal(compiled.status, "compiled");
  if (compiled.status !== "compiled") return;
  const gatherer = createCompiledBatchGatherer(compiled.plan);
  assert.throws(
    () => gatherer.gather({ ...source(), wind: undefined }),
    /complete source arrays/,
  );
});
test("separate views own separate buffers and disposal releases held slabs", () => {
  const f = fixture(),
    other = createCompiledBatchGatherer(f.plan);
  assert.notEqual(f.gatherer.models, other.models);
  f.gatherer.gather(f.source);
  other.gather(f.source);
  f.gatherer.dispose();
  f.gatherer.dispose();
  assert.equal(f.gatherer.memoryBytes, 0);
  assert.throws(() => f.gatherer.gather(f.source), /closed/);
  assert.equal(other.gather(f.source).count, 8);
});
