import assert from "node:assert/strict";
import test from "node:test";
import {
  validQuickCombat,
  validCombatRendering,
  combatEnemyPoses,
  zombieSpawns,
} from "../packages/runtime/quick-combat.ts";
import {
  isPreviewEvent,
  PREVIEW_PROTOCOL,
  INITIAL_STATUS,
} from "../packages/editor-services/protocol.ts";

function fixture() {
  return {
    player: {
      health: 200,
      maxHealth: 200,
      alive: true,
      state: "idle",
      actionTime: 0,
      cooldown: 0,
      attacks: 0,
      hits: 0,
    },
    actors: [
      {
        id: 0,
        position: [0, 0, -2],
        yaw: Math.PI,
        health: 20,
        maxHealth: 50,
        alive: true,
        state: "walk",
        actionTime: 0.5,
        animationTime: 0.5,
      },
    ],
    total: 1,
    alive: 1,
    killed: 0,
  };
}
test("combat snapshots validate health, alive counts, poses, and source state before UI admission", () => {
  const f = fixture();
  assert(validQuickCombat(f));
  assert(
    isPreviewEvent(
      {
        protocol: PREVIEW_PROTOCOL,
        sessionId: "test",
        type: "status",
        status: { ...INITIAL_STATUS, combat: f },
      },
      "test",
    ),
  );
  for (const bad of [
    { ...f, player: { ...f.player, health: 201 } },
    { ...f, player: { ...f.player, alive: false } },
    { ...f, player: { ...f.player, state: "walk" } },
    { ...f, actors: [f.actors[0], f.actors[0]], total: 2, alive: 2 },
    { ...f, actors: [{ ...f.actors[0], position: [NaN, 0, 0] }] },
    { ...f, actors: [{ ...f.actors[0], actionTime: -1 }] },
    { ...f, alive: 0 },
  ])
    assert.equal(validQuickCombat(bad), false);
});
test("dead combat actors retain their authored clock and health in renderer poses", () => {
  const f = fixture();
  f.actors[0] = {
    ...f.actors[0],
    health: 0,
    alive: false,
    state: "death",
    actionTime: 1.4,
    animationTime: 1.4,
  };
  f.alive = 0;
  f.killed = 1;
  assert(validQuickCombat(f));
  const poses = combatEnemyPoses(f);
  assert.equal(poses[0].animation, "death");
  assert.equal(poses[0].deathSeconds, 1.4);
  assert.equal(poses[0].health, 0);
  assert.equal(poses[0].maxHealth, 50);
  poses[0].position[0] = 10;
  assert.equal(f.actors[0].position[0], 0);
});
test("the encounter spawns eight separated actors inside source acquisition range", () => {
  const actors = zombieSpawns([6, 0, 20], 0.7);
  assert.equal(actors.length, 8);
  for (const actor of actors) {
    const d = Math.hypot(actor.position[0] - 6, actor.position[2] - 20);
    assert(d >= 6 && d <= 8);
    for (const other of actors)
      if (actor.id !== other.id)
        assert(
          Math.hypot(
            actor.position[0] - other.position[0],
            actor.position[2] - other.position[2],
          ) > 0.6,
        );
  }
});
test("combat draw counters admit the actual actor and health-bar budgets", () => {
  const v = {
    modelLoaded: true,
    submittedActors: 8,
    submittedTriangles: 99_888,
    submittedHealthBars: 8,
    submittedHealthBarTriangles: 16,
    geometryBytes: 70e6,
  };
  assert(validCombatRendering(v));
  assert(
    validCombatRendering({
      ...v,
      submittedActors: 16,
      submittedHealthBars: 16,
      submittedHealthBarTriangles: 64,
    }),
  );
  assert.equal(validCombatRendering({ ...v, submittedActors: 17 }), false);
  assert.equal(
    validCombatRendering({ ...v, submittedHealthBarTriangles: Infinity }),
    false,
  );
});
