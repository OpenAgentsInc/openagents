import assert from "node:assert/strict";
import test from "node:test";
const { evaluateBattleMotion, evaluateBattleConvoy } = await import(
  new URL("../tools/battle-motion-metrics.mjs", import.meta.url).href
);

const policy = {
  radii: { paladin: 0.63, sorceror: 0.63, zombie: 0.64, warrok: 1.33 },
  speeds: { paladin: 3.4, sorceror: 3.4, zombie: 1.666667, warrok: 1.666667 },
};
const actor = (id: number, x = id * 4, team = id ? 1 : 0) => ({
  id,
  position: [x, 0, 0],
  kind: "paladin",
  team,
  health: 100,
  yaw: 0,
  state: 1,
  actionTime: 0.5,
  animationTime: 0.5,
  target: id ? 0 : 1,
});
const advance = (a: any) => ({
  ...a,
  position: [a.position[0], 0, a.position[2] - 0.02],
  yaw: a.yaw + 0.03,
  actionTime: a.actionTime + 0.01,
  animationTime: a.animationTime + 0.01,
});
const frames = (actors = [actor(0), actor(1)]) => [
  { tick: 40, actors },
  { tick: 41, actors: actors.map(advance) },
];

test("motion evidence uses shortest-angle turns and actual class-specific travel bounds", () => {
  const input = frames();
  input[0].actors[0].yaw = Math.PI - 0.01;
  input[1].actors[0].yaw = -Math.PI + 0.01;
  const result = evaluateBattleMotion(input, policy);
  assert.equal(result.ok, true);
  assert.equal(result.summary.comparedLives, 2);
  assert.equal(result.summary.targetRetainedIntervals, 2);
  assert.ok(Math.abs(result.summary.maxYawDelta - 0.03) < 1e-10);
  assert.ok(Math.abs(result.summary.maxTravel - 0.02) < 1e-10);
});

test("180-degree snap, teleport, and same-state action/gait resets fail independently", () => {
  const controls: [string, (a: any) => void][] = [
    [
      "boundedTurning",
      (a) => {
        a.yaw = Math.PI;
      },
    ],
    [
      "boundedTravel",
      (a) => {
        a.position[2] = 1;
      },
    ],
    [
      "animationContinuity",
      (a) => {
        a.actionTime = 0;
      },
    ],
    [
      "animationContinuity",
      (a) => {
        a.animationTime = 0;
      },
    ],
  ];
  for (const [check, corrupt] of controls) {
    const input = frames();
    corrupt(input[1].actors[0]);
    const result = evaluateBattleMotion(input, policy);
    assert.equal(result.ok, false, check);
    assert.equal(result.checks[check], false, check);
    assert.ok(result.failures.some((failure: any) => failure.check === check));
  }
});

test("monster travel uses its slower source speed rather than the player limit", () => {
  const input = frames();
  input[0].actors[0].kind = "zombie";
  input[1].actors[0].kind = "zombie";
  input[1].actors[0].position[2] = -0.05;
  assert.equal(evaluateBattleMotion(input, policy).checks.boundedTravel, false);
  input[0].actors[0].kind = "paladin";
  input[1].actors[0].kind = "paladin";
  assert.equal(evaluateBattleMotion(input, policy).checks.boundedTravel, true);
});

test("core overlap is measured pairwise from actual class radii, including initial failures", () => {
  const a = actor(0, 0),
    b = { ...actor(1, 1.6), kind: "warrok" };
  const failed = evaluateBattleMotion(frames([a, b]), policy);
  assert.equal(failed.checks.coreClearance, false);
  assert.equal(failed.summary.maxOverlappingPairs, 1);
  assert.ok(Math.abs(failed.summary.maxCorePenetration - 0.36) < 1e-10);
  assert.equal(failed.failures[0].radiusSum, 1.96);
  const separated = evaluateBattleMotion(
    frames([a, { ...b, position: [2.1, 0, 0] }]),
    policy,
  );
  assert.equal(separated.ok, true);
  assert.equal(separated.summary.maxOverlappingPairs, 0);
  assert.equal(separated.contactFrames[0].contacts, 1);
});

test("stable-target evidence rejects equal-distance churn and accepts a substantially closer living opponent", () => {
  const input = frames([actor(0), actor(1, 4), actor(2, -4)]);
  input[1].actors[0].target = 2;
  assert.equal(evaluateBattleMotion(input, policy).checks.stableTargets, false);
  const closer = frames([actor(0), actor(1, 8), actor(2, -4)]);
  closer[1].actors[0].target = 2;
  const result = evaluateBattleMotion(closer, policy);
  assert.equal(result.ok, true);
  assert.equal(result.summary.validTargetSwitches, 1);
  const dead = frames([actor(0), actor(1, 4), actor(2, -4)]);
  dead[1].actors[0].target = 2;
  dead[1].actors[1] = {
    ...dead[1].actors[1],
    health: 0,
    state: 3,
    actionTime: 0,
  };
  assert.equal(evaluateBattleMotion(dead, policy).checks.stableTargets, true);
});

test("state transitions can begin an attack clock, but repeated attack resets cannot", () => {
  const input = frames();
  input[1].actors[0] = { ...input[1].actors[0], state: 2, actionTime: 0 };
  assert.equal(evaluateBattleMotion(input, policy).ok, true);
  input.push({
    tick: 42,
    actors: input[1].actors.map((a) => ({
      ...a,
      actionTime: a.actionTime + 1 / 60,
    })),
  });
  assert.equal(evaluateBattleMotion(input, policy).ok, true);
  input.push({ tick: 43, actors: input[2].actors.map((a) => ({ ...a })) });
  input[3].actors[0].actionTime = 0;
  assert.equal(
    evaluateBattleMotion(input, policy).checks.animationContinuity,
    false,
  );
});

test("real death and replenishment are reported without treating respawn travel as teleportation", () => {
  const input = frames();
  input[0].actors[0] = {
    ...input[0].actors[0],
    state: 3,
    health: 0,
    actionTime: 8,
  };
  input[1].actors[0] = {
    ...input[1].actors[0],
    position: [-40, 0, 90],
    actionTime: 0,
    animationTime: 0,
  };
  const result = evaluateBattleMotion(input, policy);
  assert.equal(result.ok, true);
  assert.equal(result.summary.respawns, 1);
  assert.equal(result.summary.comparedLives, 1);
});

test("skipped ticks, duplicate IDs and nonfinite samples fail instead of being silently filtered", () => {
  const skipped = frames();
  skipped[1].tick = 43;
  assert.equal(
    evaluateBattleMotion(skipped, policy).checks.consecutiveTicks,
    false,
  );
  const duplicate = frames();
  duplicate[1].actors[1].id = 0;
  assert.equal(
    evaluateBattleMotion(duplicate, policy).checks.validSamples,
    false,
  );
  const invalid = frames();
  invalid[1].actors[0].position[0] = NaN;
  assert.equal(
    evaluateBattleMotion(invalid, policy).checks.validSamples,
    false,
  );
});

const convoyFrames = (travel: (second: number) => number) =>
  Array.from({ length: 6 }, (_, second) => ({
    tick: 1 + second * 60,
    actors: Array.from({ length: 10_000 }, (_, id) => {
      const rearMelee = id > 0 && id < 32;
      return {
        ...actor(id),
        kind: rearMelee || id === 0 ? "paladin" : "zombie",
        team: rearMelee || id === 0 ? 0 : 1,
        position:
          rearMelee || id === 0
            ? [-97 + travel(second), 0, id * 4]
            : [20, 0, 1000 + id * 2],
      };
    }),
  }));

test("rear convoy checks actual forward displacement over five seconds and the final second", () => {
  const result = evaluateBattleConvoy(convoyFrames((second) => second * 3));
  assert.equal(result.ok, true);
  assert.equal(result.summary.selected, 24); // Source ranged-role IDs are excluded, not stationary followers.
  assert.equal(result.summary.eligible, 24);
  assert.equal(result.summary.progressed, 24);
  assert.equal(result.summary.minimumNet, 15);
  assert.equal(result.summary.minimumLate, 3);
});

test("early convoy movement cannot hide a near-total late stall", () => {
  const frozen = evaluateBattleConvoy(convoyFrames(() => 0));
  assert.equal(frozen.checks.lateForwardProgress, false);
  assert.equal(frozen.summary.eligible, 24);
  const earlyOnly = evaluateBattleConvoy(
    convoyFrames((second) => Math.min(second, 4) * 3),
  );
  assert.equal(earlyOnly.checks.lateForwardProgress, false);
  assert.equal(earlyOnly.summary.minimumNet, 12);
  assert.equal(earlyOnly.summary.minimumLate, 0);
});

test("convoy excludes real opposing contact without discarding stalled survivors or malformed checkpoints", () => {
  const input = convoyFrames((second) => second * 3);
  input[5].actors[32].position = [-80, 0, 4];
  const contact = evaluateBattleConvoy(input);
  assert.equal(contact.ok, true);
  assert.equal(
    contact.actors.find((a: any) => a.id === 1).exclusion,
    "opposing-contact",
  );
  assert.equal(contact.summary.eligible, 23);
  input[5].actors.pop();
  assert.equal(evaluateBattleConvoy(input).checks.validSamples, false);
});
