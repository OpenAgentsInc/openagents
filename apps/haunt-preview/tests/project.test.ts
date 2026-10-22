import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SceneHistory,
  initialProject,
  readProject,
  decisionOperation,
  exactCoordinates,
  pumpkinCount,
  walkable,
  PROJECT_ID,
  upgradeLegacyShader,
  type Operation,
  type Transaction,
} from "../src/project.ts";
const tx = (
  store: SceneHistory,
  operation: Operation,
  id = crypto.randomUUID(),
): Transaction => ({
  id,
  project: PROJECT_ID,
  baseRevision: store.project.revision,
  operation,
});
const apply = (s: SceneHistory, op: Operation) => {
  const p = s.prepare(tx(s, op));
  p.commit();
  return p.project;
};
const add: Operation = {
  kind: "add_pumpkin",
  entity: { id: "pumpkin-added", kind: "pumpkin", x: 1, z: 3, scale: 1 },
};
test("transactions validate before committing and undo/redo keep IDs with increasing revisions", () => {
  const s = new SceneHistory();
  const original = structuredClone(s.project);
  const pending = s.prepare(tx(s, add));
  assert.deepEqual(s.project, original);
  pending.commit();
  assert.equal(s.project.entities.at(-1)?.id, "pumpkin-added");
  assert.equal(s.project.revision, 1);
  apply(s, { kind: "move_pumpkin", id: "pumpkin-added", x: 2, z: 1 });
  apply(s, { kind: "undo" });
  assert.equal(s.project.entities.at(-1)?.x, 1);
  assert.equal(s.project.revision, 3);
  apply(s, { kind: "redo" });
  assert.equal(s.project.entities.at(-1)?.x, 2);
  assert.equal(s.project.revision, 4);
  apply(s, { kind: "remove_pumpkin", id: "pumpkin-added" });
  apply(s, { kind: "undo" });
  assert.equal(s.project.entities.at(-1)?.id, "pumpkin-added");
});
test("invalid targets, replays, stale revisions and failed batches leave scene/history unchanged", () => {
  const s = new SceneHistory();
  const first = tx(s, add);
  s.prepare(first).commit();
  const before = structuredClone(s.project),
    count = s.past.length;
  for (const bad of [
    first,
    { ...tx(s, { kind: "set_tint", tint: "blue" }), baseRevision: 0 },
    { ...tx(s, { kind: "undo" }), project: "another" },
    tx(s, { kind: "move_pumpkin", id: "missing", x: 1, z: 2 }),
    tx(s, { kind: "move_pumpkin", id: "pumpkin-1", x: 99, z: 2 }),
    tx(s, { kind: "resize_pumpkin", id: "pumpkin-1", scale: NaN }),
    tx(s, { kind: "replace", project: initialProject() }),
  ])
    assert.throws(() => s.prepare(bad as Transaction));
  assert.deepEqual(s.project, before);
  assert.equal(s.past.length, count);
});
test("saved data round trips, strips runtime fields, and refuses versions, IDs and resource excess", () => {
  const p = initialProject();
  const q = readProject(
    JSON.parse(
      JSON.stringify({
        ...p,
        device: { secret: "never saved" },
        pose: { x: 9 },
      }),
    ),
  );
  assert.deepEqual(q, p);
  assert.equal("device" in q, false);
  for (const bad of [
    { ...p, schema: 2 },
    { ...p, material: "../anything" },
    { ...p, entities: [p.entities[0], p.entities[0]] },
    { ...p, entities: Array(65).fill(p.entities[0]) },
  ])
    assert.throws(() => readProject(bad));
  const s = new SceneHistory();
  for (let i = 0; i < 50; i++)
    apply(s, { kind: "set_tint", tint: i % 2 ? "blue" : "warm" });
  assert.equal(s.past.length, 32);
});
test("coordinates come from source spans, and decisions cannot invent coordinates or execute code", () => {
  assert.deepEqual(exactCoordinates("place at x=-1.25, z=3.4"), {
    x: -1.25,
    z: 3.4,
  });
  assert.deepEqual(exactCoordinates("put at (1, -2)"), { x: 1, z: -2 });
  assert.equal(exactCoordinates("put two somewhere"), undefined);
  const pose = { x: 0, z: 5, yaw: 0, pitch: 0 };
  const d = {
    action: "add_pumpkin",
    placement: "coordinates",
    size: "normal",
    tint: "unsupported",
    confidence: 0.99,
    needs_review: false,
    model: "fixture",
    model_ms: 10,
  };
  const op = decisionOperation(d, "add at (1, -2)", pose, null);
  assert.equal(op.kind, "add_pumpkin");
  assert.equal((op as any).entity.x, 1);
  assert.throws(() => decisionOperation(d, "add somewhere", pose, null));
  assert.throws(() =>
    decisionOperation({ ...d, action: "eval" }, "run code", pose, null),
  );
  assert.throws(() =>
    decisionOperation({ ...d, confidence: 0.5 }, "add at (1, -2)", pose, null),
  );
  assert.deepEqual(
    decisionOperation(
      {
        ...d,
        action: "set_tint",
        tint: "blue",
        placement: "unsupported",
        size: "unsupported",
      },
      "blue",
      pose,
      null,
    ),
    { kind: "set_tint", tint: "blue" },
  );
});

test("grouped changes validate atomically and undo as one change", () => {
  const s = new SceneHistory();
  const before = structuredClone(s.project);
  assert.throws(() =>
    s.prepare(
      tx(s, {
        kind: "batch",
        operations: [add, { kind: "move_pumpkin", id: "missing", x: 1, z: 2 }],
      }),
    ),
  );
  assert.deepEqual(s.project, before);
  assert.equal(s.past.length, 0);
  apply(s, {
    kind: "batch",
    operations: [add, { kind: "set_tint", tint: "blue" }],
  });
  assert.equal(s.project.revision, 1);
  assert.equal(s.project.tint, "blue");
  assert.equal(s.project.entities.length, 6);
  apply(s, { kind: "undo" });
  assert.equal(s.project.entities.length, 5);
  assert.equal(s.project.tint, "warm");
});

test("legacy shader upgrade is idempotent and preserves custom/current shader code", () => {
  const old =
    "// Lantern Hall — original demo\nstruct Scene {\n  light: vec4<f32>,\n}\nlet color = LANTERN_TINT * toon;";
  const upgraded = upgradeLegacyShader(old);
  assert.ok(upgraded.includes("scene.tint.xyz"));
  assert.equal(upgradeLegacyShader(upgraded), upgraded);
  const authored = upgraded.replace(
    "LANTERN_TINT * scene.tint.xyz * toon",
    "LANTERN_TINT * toon",
  );
  assert.equal(upgradeLegacyShader(authored), authored);
  assert.equal(
    upgradeLegacyShader(
      old.replace("// Lantern Hall — original demo", "// My custom shader"),
    ),
    old.replace("// Lantern Hall — original demo", "// My custom shader"),
  );
});

test("exact plural counts and huge groups preserve placement, capacity, and one-step undo", () => {
  for (const text of [
    "add 5 huge pumpkins to my left",
    "put five big pumpkins on my right",
    "spawn 5 pumpkins at (1, 2)",
  ])
    assert.equal(pumpkinCount(text), 5);
  assert.equal(pumpkinCount("add a big pumpkin at (5, 2)"), 1);
  for (const text of [
    "add 0 pumpkins",
    "add -2 pumpkins",
    "add 1.5 pumpkins",
    "add 9 pumpkins",
    "add twenty pumpkins",
    "add twenty five pumpkins",
    "add one hundred pumpkins",
    "add many pumpkins",
  ])
    assert.throws(() => pumpkinCount(text), /1.*8/);
  const d = {
    action: "add_pumpkin",
    placement: "left",
    size: "huge",
    tint: "unsupported",
    confidence: 0.99,
    needs_review: false,
    model: "fixture",
    model_ms: 0,
  };
  for (const pose of [
    { x: 0, z: 6.8, yaw: 0, pitch: 0 },
    { x: 0, z: 5.8, yaw: 0, pitch: 0 },
    { x: 0, z: -2.8, yaw: Math.PI, pitch: 0 },
  ]) {
    const store = new SceneHistory();
    const before = structuredClone(store.project);
    const op = decisionOperation(
      d,
      "add 5 huge pumpkins to my left",
      pose,
      "pumpkin-1",
      store.project,
    );
    assert.equal(op.kind, "batch");
    apply(store, op);
    const added = store.project.entities.slice(before.entities.length);
    assert.equal(added.length, 5);
    assert.equal(new Set(added.map((e) => e.id)).size, 5);
    for (const e of added) {
      assert.equal(e.scale, 2.5);
      assert.ok(walkable(e.x, e.z));
      for (const existing of before.entities)
        assert.ok(
          Math.hypot(e.x - existing.x, e.z - existing.z) >=
            1 + existing.scale * 0.4 + 0.1,
        );
      assert.ok(
        (e.x - pose.x) * Math.cos(pose.yaw) +
          (e.z - pose.z) * Math.sin(pose.yaw) <
          0,
        "pumpkin must be on the player's left",
      );
      for (const other of added)
        if (other !== e)
          assert.ok(Math.hypot(e.x - other.x, e.z - other.z) >= 2.1 - 0.00001);
    }
    assert.equal(store.project.revision, 1);
    apply(store, { kind: "undo" });
    assert.deepEqual(store.project.entities, before.entities);
    apply(store, { kind: "redo" });
    assert.deepEqual(store.project.entities.slice(5), added);
  }
  const full = initialProject();
  full.entities = Array.from({ length: 62 }, (_, i) => ({
    ...full.entities[0],
    id: `pumpkin-${i}`,
  }));
  assert.throws(
    () =>
      decisionOperation(
        d,
        "add 5 huge pumpkins to my left",
        { x: 0, z: 6.8, yaw: 0, pitch: 0 },
        null,
        full,
      ),
    /2 more pumpkins/,
  );
});
