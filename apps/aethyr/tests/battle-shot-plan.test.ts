import assert from "node:assert/strict";
import test from "node:test";
const {
  battleParticipationPlan,
  battleParticipantEvidence,
  NEUTRAL_BATTLE_INPUT,
  battleRecordingLook,
} = await import(
  new URL("../tools/battle-shot-plan.mjs", import.meta.url).href
);

test("camera recording sends a smooth bounded look sweep without accumulating drift", () => {
  for (const view of ["first", "third"]) {
    let totalYaw = 0,
      totalPitch = 0,
      maxStep = 0;
    for (let ms = 250; ms <= 20000; ms += 250) {
      const input = battleRecordingLook(20, ms - 250, ms, view);
      totalYaw += input.lookX;
      totalPitch += input.lookY;
      maxStep = Math.max(maxStep, Math.abs(input.lookX));
      assert(Math.abs(input.lookX) <= 0.5 && Math.abs(input.lookY) <= 0.5);
    }
    assert(Math.abs(totalYaw) < 1e-12 && Math.abs(totalPitch) < 1e-12);
    assert(maxStep < 0.05 && maxStep > 0.01);
  }
  assert.throws(() => battleRecordingLook(0, 0, 0, "third"));
  assert.throws(() => battleRecordingLook(20, 500, 250, "third"));
  assert.throws(() => battleRecordingLook(20, 0, 250, "orbit"));
});

test("participation shots use genuine held movement and attack controls with neutral gaps", () => {
  for (const seconds of [20, 30]) {
    const plan = battleParticipationPlan(seconds);
    assert.deepEqual(plan[0].input, NEUTRAL_BATTLE_INPUT);
    assert.ok(
      plan.some(
        (phase: any) => phase.input.forward === 1 && phase.input.sprint,
      ),
    );
    assert.ok(plan.some((phase: any) => phase.input.attack));
    for (let i = 0; i < plan.length; i++) {
      assert.ok(plan[i].atMs >= 0 && plan[i].atMs < seconds * 1000);
      if (i) assert.ok(plan[i].atMs > plan[i - 1].atMs);
      assert.deepEqual(Object.keys(plan[i].input).sort(), [
        "attack",
        "forward",
        "sprint",
        "turn",
      ]);
    }
    assert.equal(plan[3].input.attack, false);
    assert.equal(plan[4].input.attack, true);
    assert.equal(plan[4].input.sprint, false);
  }
});

test("participation evidence requires actual travel and simulation attacks, excluding respawn jumps", () => {
  const sample = (
    at: number,
    x: number,
    attackSequence: number,
    health = 200,
  ) => ({
    at,
    battle: {
      participant: {
        active: true,
        alive: health > 0,
        position: [x, 0, 0],
        health,
        maxHealth: 200,
        attackSequence,
      },
    },
  });
  const real = battleParticipantEvidence([
    sample(0, 0, 0),
    sample(500, 1, 0),
    sample(1000, 1, 1, 195),
  ]);
  assert.equal(real.activeParticipation, true);
  assert.equal(real.damageTaken, 5);
  assert.equal(
    battleParticipantEvidence([sample(0, 0, 0), sample(500, 0, 1)])
      .activeParticipation,
    false,
  );
  assert.equal(
    battleParticipantEvidence(
      [sample(0, 0, 0), sample(500, 20, 1)],
      [{ atMs: 250, kind: "respawn-request" }],
    ).activeParticipation,
    false,
  );
});

test("both recording cohorts receive equal independent plans and invalid durations fail", () => {
  const a = battleParticipationPlan(30),
    b = battleParticipationPlan(30);
  assert.deepEqual(a, b);
  a[1].input.forward = 0;
  assert.equal(b[1].input.forward, 1);
  for (const seconds of [-1, 0, 61, NaN, Infinity])
    assert.throws(() => battleParticipationPlan(seconds));
});
