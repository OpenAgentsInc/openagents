import assert from "node:assert/strict";
import test from "node:test";
import { battleAttackControl } from "../apps/editor/src/battle-ability-state.ts";
import {
  INITIAL_STATUS,
  type EngineStatus,
} from "../packages/editor-services/protocol.ts";

function state(): EngineStatus {
  return {
    ...INITIAL_STATUS,
    phase: "ready",
    mmorpg: { connection: "online", actors: [], events: [] },
    battle: {
      enabled: true,
      count: 10000,
      camera: "first",
      stats: { meteorCooldownTicks: 0 },
      participant: {
        active: true,
        ready: true,
        class: "paladin",
        team: 0,
        position: [0, 0, 0],
        yaw: 0,
        pitch: 0,
        health: 200,
        maxHealth: 200,
        alive: true,
        attackKind: "meteor_shower",
        attackSequence: 0,
        tick: 1,
        droppedSeconds: 0,
        target: { point: [10, 0, 10], normal: [0, 1, 0], distance: 14 },
      },
    },
  };
}

test("Meteor Cast uses admitted target and actual GPU cooldown rather than predicting a local timer", () => {
  const s = state();
  assert.deepEqual(battleAttackControl(s), {
    meteor: true,
    label: "Cast",
    seconds: 0,
    enabled: true,
    note: "Ready to cast",
  });
  s.battle!.stats.meteorCooldownTicks = 252;
  assert.deepEqual(battleAttackControl(s), {
    meteor: true,
    label: "Cast",
    seconds: 4.2,
    enabled: false,
    note: "Ready in 5s",
  });
  s.battle!.stats.meteorCooldownTicks = 1;
  assert.equal(battleAttackControl(s).enabled, false);
  s.battle!.stats.meteorCooldownTicks = 0;
  delete s.battle!.participant!.target;
  assert.equal(battleAttackControl(s).enabled, false);
  assert.equal(battleAttackControl(s).note, "Aim at the ground");
});

test("missing GPU counters and provisional/dead/offline/pending participants cannot cast", () => {
  const noCounters = state();
  delete noCounters.battle!.stats.meteorCooldownTicks;
  assert.equal(battleAttackControl(noCounters).note, "Preparing spell…");
  assert.equal(battleAttackControl(noCounters).enabled, false);
  for (const change of [
    (s: EngineStatus) => {
      s.phase = "loading";
    },
    (s: EngineStatus) => {
      s.mmorpg!.connection = "disconnected";
    },
    (s: EngineStatus) => {
      s.battle!.enabled = false;
    },
    (s: EngineStatus) => {
      s.battle!.camera = "dolly";
    },
    (s: EngineStatus) => {
      s.battle!.participant!.active = false;
    },
    (s: EngineStatus) => {
      s.battle!.participant!.ready = false;
    },
    (s: EngineStatus) => {
      s.battle!.participant!.alive = false;
    },
  ]) {
    const s = state();
    change(s);
    assert.equal(battleAttackControl(s).enabled, false);
  }
  assert.equal(battleAttackControl(state(), true).enabled, false);
});

test("Sword and Firebolt keep their Attack controls without a Meteor target or cooldown", () => {
  for (const attackKind of ["melee", "ranged"] as const) {
    const s = state();
    s.battle!.participant!.attackKind = attackKind;
    delete s.battle!.participant!.target;
    s.battle!.stats.meteorCooldownTicks = 252;
    const control = battleAttackControl(s);
    assert.equal(control.meteor, false);
    assert.equal(control.label, "Attack");
    assert.equal(control.enabled, true);
    assert.equal(control.note, undefined);
  }
});
