import test from "node:test";
import assert from "node:assert/strict";
import {
  RuntimePropTable,
  type PropEntity,
  type PropVector,
} from "../packages/runtime/prop-table.ts";
import { modelMatrix, transformPoint } from "../packages/renderer/math.ts";

const prop = (id = "barrel", extra: Partial<PropEntity> = {}): PropEntity => ({
  id,
  kind: "barrel",
  position: [1, 2, 3],
  yaw: 0,
  scale: 1,
  collidable: true,
  halfExtents: [1, 2, 3],
  ...extra,
});
const activate = (table: RuntimePropTable, entities: PropEntity[]) => {
  const plan = table.prepare(entities);
  plan.activate();
  return plan;
};
const near = (
  actual: readonly number[],
  expected: readonly number[],
  epsilon = 0.0001,
) =>
  actual.forEach((value, i) =>
    assert(
      Math.abs(value - expected[i]) <= epsilon,
      `${actual} differs from ${expected} at ${i}`,
    ),
  );

test("derived props stage all views before activation, with cloned active records and stable handles", () => {
  const table = new RuntimePropTable(),
    input = prop(),
    plan = table.prepare([input]);
  assert.equal(table.snapshot().liveRecords, 0);
  assert.equal(table.handle(input.id), undefined);
  assert.equal(plan.ready(), true);
  assert.equal(plan.structural, true);
  assert.equal(plan.writes.length, 1);
  assert.deepEqual(plan.handle(input.id), { slot: 0, generation: 1 });
  const staged = plan.records()[0];
  assert.deepEqual(
    staged.matrix,
    new Float32Array(modelMatrix(input.position)),
  );
  assert.deepEqual(staged.physics, { min: [0, 2, 0], max: [2, 6, 6] });
  assert.deepEqual(staged.selection.bounds, staged.physics);
  staged.matrix.fill(99);
  staged.selection.position[0] = 99;
  input.position[0] = 500;
  input.halfExtents![0] = 50;
  plan.activate();
  assert.equal(plan.ready(), false);
  const record = table.resolve(table.handle("barrel")!)!;
  assert.equal(record.matrix[12], 1);
  assert.deepEqual(record.physics, { min: [0, 2, 0], max: [2, 6, 6] });
  record.handle.generation = 99;
  record.matrix[12] = 100;
  record.physics!.min[0] = -99;
  record.selection.position[0] = 400;
  assert.equal(table.resolve(table.handle("barrel")!)!.matrix[12], 1);
  assert.equal(table.records()[0].physics!.min[0], 0);
  assert.throws(() => plan.activate(), /newer prop change/);
  assert.throws(() => plan.writes, /newer prop change/);
});

test("one transform among 64 records reuses slots and typed buffers and uploads one matrix", () => {
  const table = new RuntimePropTable({ initialCapacity: 64 });
  const entities = Array.from({ length: 64 }, (_, i) =>
    prop(`prop-${i}`, { position: [i * 3, 0, 0] }),
  );
  activate(table, entities);
  const previousHandle = table.handle("prop-17"),
    unchanged = table.resolve(table.handle("prop-18")!)!.matrix;
  const before = table.snapshot();
  entities[17] = { ...entities[17], position: [2, 3, 4], yaw: 0.7, scale: 2 };
  const plan = table.prepare(entities);
  assert.equal(plan.structural, false);
  assert.equal(plan.writes.length, 1);
  assert.equal(plan.writes[0].id, "prop-17");
  assert.deepEqual(plan.handle("prop-17"), previousHandle);
  plan.activate();
  const after = table.snapshot();
  assert.equal(after.recordsValidated - before.recordsValidated, 64);
  assert.equal(after.recordsRebuilt - before.recordsRebuilt, 1);
  assert.equal(after.matricesRebuilt - before.matricesRebuilt, 1);
  assert.equal(after.physicsRebuilt - before.physicsRebuilt, 1);
  assert.equal(after.selectionRebuilt - before.selectionRebuilt, 1);
  assert.equal(after.bytesAllocated - before.bytesAllocated, 0);
  assert.equal(after.bytesCopied - before.bytesCopied, 108);
  assert.equal(after.uploadBytes - before.uploadBytes, 64);
  assert.deepEqual(table.resolve(table.handle("prop-18")!)!.matrix, unchanged);
});

test("unchanged props compute no matrices and metadata-only kind or collision edits reuse geometry transforms", () => {
  const table = new RuntimePropTable(),
    entity = prop();
  activate(table, [entity]);
  const before = table.snapshot(),
    original = table.handle(entity.id);
  let plan = table.prepare([structuredClone(entity)]);
  assert.equal(plan.writes.length, 0);
  assert.equal(plan.structural, false);
  plan.activate();
  assert.equal(table.snapshot().recordsRebuilt, before.recordsRebuilt);
  assert.equal(table.snapshot().bytesAllocated, before.bytesAllocated);
  plan = table.prepare([{ ...entity, kind: "crate" }]);
  assert.equal(plan.writes.length, 0);
  assert.equal(plan.structural, true);
  plan.activate();
  assert.deepEqual(table.handle(entity.id), original);
  assert.equal(table.resolve(original!)!.kind, "crate");
  const beforeBounds = table.snapshot();
  plan = table.prepare([{ ...entity, kind: "crate", halfExtents: [2, 3, 4] }]);
  assert.equal(plan.writes.length, 0);
  assert.equal(plan.structural, false);
  plan.activate();
  assert.equal(table.snapshot().matricesRebuilt, beforeBounds.matricesRebuilt);
  assert.equal(table.snapshot().bytesCopied - beforeBounds.bytesCopied, 24);
  assert.equal(table.resolve(original!)!.physics!.max[1], 8);
});

test("remove and re-add invalidate stale handles even when the same authored ID reuses its slot", () => {
  const table = new RuntimePropTable({ initialCapacity: 2, maxCapacity: 4 });
  activate(table, [prop("a"), prop("b")]);
  const a = table.handle("a")!,
    b = table.handle("b")!;
  const remove = table.prepare([prop("b")]);
  assert.deepEqual(remove.removed, [{ id: "a", slot: a.slot, handle: a }]);
  assert.equal(table.resolve(a)?.id, "a");
  remove.activate();
  assert.equal(table.resolve(a), undefined);
  activate(table, [prop("b"), prop("a")]);
  const replacement = table.handle("a")!;
  assert.equal(replacement.slot, a.slot);
  assert.equal(replacement.generation, a.generation + 1);
  assert.equal(table.resolve(a), undefined);
  assert.equal(table.resolve(replacement)?.id, "a");
  assert.deepEqual(table.handle("b"), b);
  activate(table, [prop("c"), prop("b")]);
  assert.equal(table.handle("c")!.slot, replacement.slot);
  assert.equal(table.handle("c")!.generation, replacement.generation + 1);
  assert.equal(table.resolve(replacement), undefined);
});

test("newer desired versions and discard invalidate old staging views without mutating active records", () => {
  const table = new RuntimePropTable(),
    original = prop();
  activate(table, [original]);
  const active = table.handle(original.id)!;
  const first = table.prepare([{ ...original, position: [10, 0, 0] }]);
  const heldWrite = first.writes[0];
  const second = table.prepare([{ ...original, position: [20, 0, 0] }]);
  assert.equal(first.ready(), false);
  assert.throws(() => heldWrite.matrix, /newer prop change/);
  assert.throws(() => first.writes, /newer prop change/);
  assert.throws(() => first.matrixRecords(), /newer prop change/);
  assert.throws(() => first.activate(), /newer prop change/);
  assert.equal(table.resolve(active)!.matrix[12], 1);
  first.discard();
  assert.equal(
    second.ready(),
    true,
    "Discarding an older candidate cannot cancel the new one.",
  );
  second.discard();
  second.discard();
  assert.throws(() => second.activate(), /newer prop change/);
  assert.equal(table.resolve(active)!.matrix[12], 1);
  const next = table.prepare([{ ...original, position: [30, 0, 0] }]);
  next.activate();
  assert.equal(table.resolve(active)!.matrix[12], 30);
});

test("all input validation precedes derived activation and a newer invalid request supersedes old candidates", () => {
  const table = new RuntimePropTable(),
    entity = prop();
  activate(table, [entity]);
  const active = table.handle(entity.id)!;
  const invalid: unknown[] = [
    null,
    {},
    { ...entity, id: "with spaces" },
    { ...entity, id: "a".repeat(97) },
    { ...entity, kind: "" },
    { ...entity, kind: "é".repeat(65) },
    { ...entity, position: [Infinity, 0, 0] },
    { ...entity, position: [100_001, 0, 0] },
    { ...entity, position: [0, 0] },
    { ...entity, yaw: NaN },
    { ...entity, yaw: 1e50 },
    { ...entity, scale: 0 },
    { ...entity, scale: 21 },
    { ...entity, collidable: "yes" },
    { ...entity, halfExtents: undefined },
    { ...entity, halfExtents: [0, 1, 1] },
    { ...entity, halfExtents: [1, 1001, 1] },
    { ...entity, collisionOffset: [NaN, 0, 0] },
  ];
  for (const bad of invalid) {
    const before = table.snapshot(),
      prior = table.prepare([{ ...entity, position: [8, 0, 0] }]);
    assert.throws(() => table.prepare([bad as PropEntity]));
    assert.equal(prior.ready(), false);
    assert.equal(table.snapshot().version, before.version);
    assert.equal(table.resolve(active)!.matrix[12], 1);
  }
  assert.throws(
    () => table.prepare([entity, entity]),
    /each authored prop ID once/,
  );
  assert.equal(table.snapshot().liveRecords, 1);
});

test("resident capacity grows by powers of two with explicit full-copy accounting and bounded limits", () => {
  const table = new RuntimePropTable({ initialCapacity: 2, maxCapacity: 8 });
  activate(table, [prop("a"), prop("b")]);
  const before = table.snapshot(),
    oldHandle = table.handle("a");
  const plan = table.prepare([prop("a"), prop("b"), prop("c")]);
  assert.equal(plan.capacity, 4);
  assert.equal(table.snapshot().capacity, 2);
  const matrices = plan.matrixRecords();
  assert.equal(matrices.length, 4 * 16);
  assert.deepEqual(
    matrices.subarray(0, 16),
    new Float32Array(modelMatrix(prop().position)),
  );
  matrices.fill(99);
  plan.activate();
  assert.equal(table.snapshot().capacity, 4);
  assert.deepEqual(table.handle("a"), oldHandle);
  assert.equal(table.resolve(oldHandle!)!.matrix[12], 1);
  assert(table.snapshot().bytesAllocated > before.bytesAllocated);
  assert(table.snapshot().bytesCopied > before.bytesCopied);
  const version = table.snapshot().version;
  assert.throws(
    () => table.prepare(Array.from({ length: 9 }, (_, i) => prop(`p${i}`))),
    /capacity/,
  );
  assert.equal(table.snapshot().version, version);
  assert.throws(
    () => new RuntimePropTable({ initialCapacity: 3 }),
    /power-of-two/,
  );
  assert.throws(
    () => new RuntimePropTable({ initialCapacity: 1, maxCapacity: 32_768 }),
    /power-of-two/,
  );
});

test("collision centers preserve legacy floor pivots or use explicit local centers under yaw and scale", () => {
  const table = new RuntimePropTable();
  activate(table, [
    prop("legacy"),
    prop("centered", { collisionOffset: [0, 0, 0] }),
    prop("offset", {
      position: [10, 5, 20],
      halfExtents: [1, 2, 3],
      collisionOffset: [4, -1, 2],
      yaw: Math.PI / 2,
      scale: 2,
    }),
    prop("ghost", { collidable: false }),
    prop("point", { halfExtents: undefined, collidable: false }),
  ]);
  assert.deepEqual(table.resolve(table.handle("legacy")!)!.physics, {
    min: [0, 2, 0],
    max: [2, 6, 6],
  });
  assert.deepEqual(table.resolve(table.handle("centered")!)!.physics, {
    min: [0, 0, 0],
    max: [2, 4, 6],
  });
  const offset = table.resolve(table.handle("offset")!)!;
  near(offset.physics!.min, [8, -1, 10]);
  near(offset.physics!.max, [20, 7, 14]);
  assert.deepEqual(offset.physics, offset.selection.bounds);
  const ghost = table.resolve(table.handle("ghost")!)!;
  assert.equal(ghost.physics, null);
  assert(ghost.selection.bounds);
  const point = table.resolve(table.handle("point")!)!;
  assert.equal(point.physics, null);
  assert.equal(point.selection.bounds, null);
});

test("derived physics encloses transformed local corners across seeded positions, offsets, scales, and yaws", () => {
  let seed = 0x315af;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x1_0000_0000;
  };
  const entities = Array.from({ length: 512 }, (_, i) =>
    prop(`random-${i}`, {
      position: [random() * 100 - 50, random() * 10, random() * 100 - 50],
      yaw: random() * Math.PI * 2 - Math.PI,
      scale: random() * 4 + 0.1,
      halfExtents: [random() * 2 + 0.1, random() * 2 + 0.1, random() * 2 + 0.1],
      ...(i % 2
        ? {
            collisionOffset: [
              random() * 4 - 2,
              random() * 4 - 2,
              random() * 4 - 2,
            ] as PropVector,
          }
        : {}),
    }),
  );
  const table = new RuntimePropTable();
  activate(table, entities);
  for (const entity of entities) {
    const result = table.resolve(table.handle(entity.id)!)!,
      half = entity.halfExtents!.map(Math.fround);
    const center = entity.collisionOffset?.map(Math.fround) ?? [0, half[1], 0];
    const matrix = modelMatrix(
      entity.position.map(Math.fround) as PropVector,
      Math.fround(entity.yaw!),
      Math.fround(entity.scale!),
    );
    const corners: PropVector[] = [];
    for (const x of [-1, 1])
      for (const y of [-1, 1])
        for (const z of [-1, 1])
          corners.push(
            transformPoint(matrix, [
              center[0] + half[0] * x,
              center[1] + half[1] * y,
              center[2] + half[2] * z,
            ]),
          );
    near(
      result.physics!.min,
      [0, 1, 2].map((axis) => Math.min(...corners.map((p) => p[axis]))),
    );
    near(
      result.physics!.max,
      [0, 1, 2].map((axis) => Math.max(...corners.map((p) => p[axis]))),
    );
  }
});

test("invalid generation handles never resolve recycled, empty, or unrelated records", () => {
  const table = new RuntimePropTable();
  activate(table, [prop()]);
  const current = table.handle("barrel")!;
  for (const invalid of [
    null,
    { ...current, generation: 0 },
    { ...current, generation: 2 },
    { ...current, slot: -1 },
    { ...current, slot: 32 },
    { ...current, slot: 0.5 },
    { ...current, generation: Infinity },
  ])
    assert.equal(table.resolve(invalid as never), undefined);
});
