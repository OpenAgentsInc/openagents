import assert from "node:assert/strict";
import { test } from "node:test";
import { GhostSimulation } from "../src/behaviors.ts";
import {
  SceneHistory,
  initialProject,
  readProject,
  decisionOperation,
  PROJECT_ID,
  walkable,
  type Operation,
  type SceneAsset,
} from "../src/project.ts";

const ghost = (id = "ghost-one", x = 1, z = 2): SceneAsset => ({
  id,
  kind: "ghost",
  x,
  z,
  scale: 1,
});
const apply = (s: SceneHistory, operation: Operation) =>
  s
    .prepare({
      id: crypto.randomUUID(),
      project: PROJECT_ID,
      baseRevision: s.project.revision,
      operation,
    })
    .commit();
const pose = { x: 0, z: 6.8, yaw: 0, pitch: 0 };

test("behavior edits are bounded, ghost-only, saved and undone as one group", () => {
  const p = initialProject();
  p.entities.push(ghost(), ghost("ghost-two", -1, 3));
  const s = new SceneHistory(p);
  const before = structuredClone(s.project);
  for (const operation of [
    { kind: "set_behavior", ids: ["pumpkin-1"], behavior: "chase" },
    { kind: "set_behavior", ids: ["missing"], behavior: "float" },
    { kind: "set_behavior", ids: [], behavior: "float" },
    {
      kind: "set_behavior",
      ids: ["ghost-one", "ghost-one"],
      behavior: "float",
    },
    { kind: "set_behavior", ids: ["ghost-one"], behavior: "execute_code" },
  ])
    assert.throws(() => apply(s, operation as Operation));
  assert.deepEqual(s.project, before);
  apply(s, {
    kind: "set_behavior",
    ids: ["ghost-one", "ghost-two"],
    behavior: "float",
  });
  assert.equal(s.project.revision, 1);
  assert.ok(s.project.entities.slice(-2).every((e) => e.behavior === "float"));
  assert.deepEqual(
    readProject(JSON.parse(JSON.stringify(s.project))),
    s.project,
  );
  apply(s, { kind: "undo" });
  assert.deepEqual(s.project.entities, before.entities);
  apply(s, { kind: "redo" });
  assert.ok(s.project.entities.slice(-2).every((e) => e.behavior === "float"));
  assert.throws(() =>
    readProject({ ...p, entities: [{ ...ghost(), behavior: "attack" }] }),
  );
});

test("plural behavior requests address all ghosts; that ghost requires a selected ghost", () => {
  const p = initialProject();
  p.entities.push(ghost(), ghost("ghost-two", -1, 3));
  const d = {
    action: "set_behavior",
    behavior: "float",
    target_scope: "all_ghosts",
    placement: "unsupported",
    size: "unsupported",
    tint: "unsupported",
    confidence: 1,
    needs_review: false,
    model: "fixture",
    model_ms: 0,
  };
  assert.deepEqual(
    decisionOperation(d, "make the ghosts float around", pose, "pumpkin-1", p),
    {
      kind: "set_behavior",
      ids: ["ghost-one", "ghost-two"],
      behavior: "float",
    },
  );
  assert.deepEqual(
    decisionOperation(
      { ...d, behavior: "chase", target_scope: "selected" },
      "make that ghost chase me",
      pose,
      "ghost-two",
      p,
    ),
    { kind: "set_behavior", ids: ["ghost-two"], behavior: "chase" },
  );
  assert.throws(
    () =>
      decisionOperation(
        { ...d, target_scope: "selected" },
        "make that ghost chase me",
        pose,
        "pumpkin-1",
        p,
      ),
    /Select a ghost/,
  );
  assert.throws(() =>
    decisionOperation(
      { ...d, behavior: "attack" },
      "make ghosts attack",
      pose,
      "ghost-one",
      p,
    ),
  );
  assert.throws(
    () =>
      decisionOperation(
        d,
        "make the ghosts float around",
        pose,
        null,
        initialProject(),
      ),
    /Add a ghost/,
  );
});

test("floating changes live transforms without mutating saved positions or escaping rooms", () => {
  const assets = [{ ...ghost(), behavior: "float" as const }];
  const before = structuredClone(assets),
    sim = new GhostSimulation();
  sim.sync(assets);
  for (let i = 0; i < 1000; i++) {
    sim.step(pose, 1 / 60);
    for (const m of sim.snapshot())
      assert.ok(walkable(m.x, m.z) && Number.isFinite(m.y));
  }
  const m = sim.snapshot()[0];
  assert.ok(Math.hypot(m.x - assets[0].x, m.z - assets[0].z) > 0.1);
  assert.ok(m.y > 0);
  assert.deepEqual(assets, before);
});

test("chase crosses the corridor, respects its stop distance, caps time jumps and freezes", () => {
  const asset = { ...ghost("ghost-one", 2.5, 0), behavior: "chase" as const };
  const sim = new GhostSimulation();
  sim.sync([asset]);
  const player = { ...pose, z: -14 };
  sim.step(player, 1000);
  assert.ok(
    Math.hypot(sim.snapshot()[0].x - asset.x, sim.snapshot()[0].z - asset.z) <=
      0.061,
  );
  for (let i = 0; i < 2400; i++) sim.step(player, 1 / 60);
  const m = sim.snapshot()[0];
  assert.ok(Math.hypot(m.x - player.x, m.z - player.z) < 1.16);
  assert.ok(Math.hypot(m.x - player.x, m.z - player.z) >= 1.149);
  const position = sim.positions([asset.id])[0];
  sim.sync([{ ...asset, ...position, behavior: "still" }]);
  const frozen = sim.snapshot();
  for (let i = 0; i < 100; i++) sim.step(pose, 1 / 60);
  sim.step(pose, NaN);
  assert.deepEqual(sim.snapshot(), frozen);
  sim.sync([]);
  assert.equal(sim.snapshot().length, 0);
});
