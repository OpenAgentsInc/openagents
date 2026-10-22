import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SceneHistory,
  initialProject,
  readProject,
  decisionOperation,
  exactCoordinates,
  pumpkinCount,
  assetCount,
  ASSETS,
  type AssetKind,
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
    /2 more assets/,
  );
});

test("all asset kinds round-trip, render distinct meshes, and share editing and undo", async () => {
  const { houseGeometry } = await import("../src/scene.ts");
  const meshes = new Set<string>();
  for (const [kind, asset] of Object.entries(ASSETS)) {
    const s = new SceneHistory();
    const d = {
      action: "add_asset",
      asset: kind,
      placement: "coordinates",
      size: "normal",
      tint: "unsupported",
      confidence: 1,
      needs_review: false,
      model: "fixture",
      model_ms: 1,
    };
    const op = decisionOperation(
      d,
      `add a ${asset.label} at (1, 3)`,
      { x: 0, z: 6.8, yaw: 0, pitch: 0 },
      null,
      s.project,
    );
    apply(s, op);
    const e = s.project.entities.at(-1)!;
    assert.equal(e.kind, kind);
    assert.equal(
      readProject(JSON.parse(JSON.stringify(s.project))).entities.at(-1)!.kind,
      kind,
    );
    const mesh = houseGeometry([e]);
    assert.ok(mesh.length > 0 && mesh.every(Number.isFinite));
    meshes.add(JSON.stringify([...mesh]));
    apply(s, { kind: "move_asset", id: e.id, x: 1.5, z: 2 });
    apply(s, { kind: "resize_asset", id: e.id, scale: 1.8 });
    apply(s, { kind: "remove_asset", id: e.id });
    apply(s, { kind: "undo" });
    assert.deepEqual(s.project.entities.at(-1), {
      ...e,
      x: 1.5,
      z: 2,
      scale: 1.8,
    });
    assert.equal(
      assetCount(`add three ${asset.plural} at (1, 3)`, kind as AssetKind),
      3,
    );
    for (const noun of asset.aliases)
      assert.equal(
        assetCount(`add 2 small ${noun} ahead`, kind as AssetKind),
        2,
      );
  }
  assert.equal(meshes.size, Object.keys(ASSETS).length);
  const p = initialProject();
  assert.throws(() =>
    readProject({ ...p, entities: [{ ...p.entities[0], kind: "dragon" }] }),
  );
  assert.throws(() =>
    decisionOperation(
      {
        action: "add_asset",
        asset: "dragon",
        placement: "ahead",
        size: "normal",
        tint: "unsupported",
        confidence: 1,
        needs_review: false,
        model: "fixture",
        model_ms: 0,
      },
      "add a dragon",
      { x: 0, z: 6.8, yaw: 0, pitch: 0 },
      null,
    ),
  );
});

test("named asset edits cannot change a different selected kind", () => {
  const p = initialProject();
  p.entities.push({ id: "ghost-only", kind: "ghost", x: 1, z: 3, scale: 1 });
  const d = {
    action: "remove_asset",
    asset: "unsupported",
    placement: "unsupported",
    size: "unsupported",
    tint: "unsupported",
    confidence: 1,
    needs_review: false,
    model: "fixture",
    model_ms: 0,
  };
  const pose = { x: 0, z: 6.8, yaw: 0, pitch: 0 };
  assert.deepEqual(
    decisionOperation(d, "remove the ghost", pose, "pumpkin-1", p),
    { kind: "remove_asset", id: "ghost-only" },
  );
  assert.throws(
    () => decisionOperation(d, "remove a pumpkin", pose, "ghost-only", p),
    /Select a pumpkin/,
  );
  assert.deepEqual(
    decisionOperation(d, "remove the selected asset", pose, "ghost-only", p),
    { kind: "remove_asset", id: "ghost-only" },
  );
});
