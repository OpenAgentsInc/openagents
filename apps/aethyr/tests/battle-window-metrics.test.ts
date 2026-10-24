import { BattleFollowCamera } from "../packages/runtime/battle-view.ts";
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BATTLE_COVERAGE_VIEWS,
  battleParticipantWindow,
  battleParticipationAim,
  battleWindowAround60,
  battleAllViewsCoverage,
} from "../tools/battle-window-metrics.mjs";
const participant = (x: number, seq: number, drop = 0) => ({
  active: true,
  position: [x, 0, 0],
  alive: true,
  health: 200,
  maxHealth: 200,
  attackSequence: seq,
  droppedSeconds: drop,
});
const samples = (damage = 300) =>
  [0, 1, 2].map((i) => ({
    wallMs: i * 1000,
    battle: {
      participant: participant(i, i),
      stats: {
        participantAttacks: i,
        participantDamage: i * damage,
        participantKills: 0,
        participantHits: 0,
      },
    },
  }));
const row = (view = "overview") => ({
  count: 1e6,
  view,
  repeat: 0,
  observedMs: 30000,
  warmupMs: 5000,
  applicationPacing: {
    ok: false,
    summary: {
      acceptedRateHz: 59,
      completedRateHz: 58.5,
      wholeWindowDropped: { kernelDroppedSeconds: 0, hostClampedSeconds: 0 },
    },
  },
  pacing: { ok: false },
  battleContinuity: { noDroppedTime: true },
  statusFreshness: { ok: true },
  combatProgress: { ok: true },
  overflowFidelity: { ok: true },
  populations: {
    visible: { min: 1e6 },
    triangles: { min: 1e6 },
    near: { min: 20 },
  },
  participantEvidence: { ok: true },
});
test("standing or swinging without actual GPU damage does not prove participant fighting", () => {
  assert.equal(battleParticipantWindow(samples()).ok, true);
  assert.equal(battleParticipantWindow(samples(0)).ok, false);
  const standing = samples();
  standing.forEach((s) => (s.battle.participant.position = [0, 0, 0]));
  assert.equal(battleParticipantWindow(standing).ok, false);
  const lost = samples();
  lost[1].battle.participant.droppedSeconds = 0.01;
  lost[2].battle.participant.droppedSeconds = 0.01;
  assert.equal(battleParticipantWindow(lost).ok, false);
});
test("around60 mean is separate from strict pacing and never hides lost time or culled wide views", () => {
  const base = row();
  assert.equal(battleWindowAround60(base).ok, true);
  assert.equal(battleWindowAround60(base).strictPacingPassed, false);
  for (const change of [
    {
      applicationPacing: {
        summary: { ...base.applicationPacing.summary, completedRateHz: 57.99 },
      },
    },
    { battleContinuity: { noDroppedTime: false } },
    { overflowFidelity: { ok: false } },
    { populations: { ...base.populations, visible: { min: 949999 } } },
    { view: "third", participantEvidence: { ok: false } },
    { view: "first", populations: { ...base.populations, near: { min: 0 } } },
  ])
    assert.equal(battleWindowAround60({ ...base, ...change }).ok, false);
});
test("all-view result needs two distinct sustained windows for each real camera", () => {
  const windows = BATTLE_COVERAGE_VIEWS.flatMap((view) =>
    [0, 1].map((repeat) => ({ ...row(view), repeat, around60: { ok: true } })),
  );
  assert.equal(battleAllViewsCoverage(windows, 1e6).ok, true);
  assert.equal(
    battleAllViewsCoverage(
      windows.filter((w) => w.view !== "dolly"),
      1e6,
    ).ok,
    false,
  );
  const duplicate = structuredClone(windows);
  duplicate[1].repeat = 0;
  assert.equal(battleAllViewsCoverage(duplicate, 1e6).ok, false);
  const short = structuredClone(windows);
  short[0].observedMs = 29999;
  assert.equal(battleAllViewsCoverage(short, 1e6).ok, false);
  const fail = structuredClone(windows);
  fail[0].around60.ok = false;
  assert.equal(battleAllViewsCoverage(fail, 1e6).ok, false);
});

test("a life reset cannot erase observed lost simulation time", () => {
  const points = samples();
  points[1].battle.participant = {
    ...participant(1, 1, 0.1),
    alive: false,
    health: 0,
  };
  points[2].battle.participant = participant(50, 2, 0);
  const result = battleParticipantWindow(points, [
    { kind: "respawn-request", atMs: 1500 },
  ]);
  assert.equal(result.observedDeaths, 1);
  assert.equal(result.observedDroppedSeconds, 0.1);
  assert.equal(result.ok, false);
  assert(
    result.movementMetres < 0.5,
    "A respawn jump became controlled travel",
  );
});

test("third-person firing look is restored after every camera life reset", () => {
  const camera = new BattleFollowCamera();
  for (let life = 0; life < 3; life++) {
    camera.reset({ yaw: Math.PI / 2, pitch: 0 });
    const look = battleParticipationAim("third");
    camera.look(look.lookX, look.lookY);
    assert(
      Math.abs(camera.orbit.pitch + 0.05) < 1e-12,
      "Third-person ranged route still aims steeply into the ground",
    );
    camera.look(0.6, -0.3);
  }
  assert.deepEqual(battleParticipationAim("first"), { lookX: 0, lookY: 0 });
  assert.throws(() => battleParticipationAim("overview"));
});
