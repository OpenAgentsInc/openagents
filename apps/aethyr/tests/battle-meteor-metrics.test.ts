import assert from "node:assert/strict";
import test from "node:test";
import { evaluateBattleMeteors } from "../tools/battle-meteor-metrics.mjs";

function timeline() {
  return Array.from({ length: 254 }, (_, n) => {
    const tick = n + 1;
    const samples = Array.from({ length: 6 }, (_, id) => {
      const launch = 151 + Math.round(id * 0.13 * 60),
        impact = launch + 60,
        target = [id * 0.5, 0, 0];
      const age = (tick - launch) / 60;
      return {
        id,
        kind: tick >= impact ? 4 : 3,
        ownerEpoch: 2,
        age: tick >= impact ? (tick - impact) / 60 : age,
        target,
        position: age < 0 ? [target[0] - 12, 80, -8] : target,
        damage: 140,
        flight: 1,
        radius: 4,
      };
    }).filter((p) => p.kind === 3 || p.age < 0.4);
    const impacts = Array.from(
      { length: 6 },
      (_, id) => 211 + Math.round(id * 0.13 * 60),
    ).filter((at) => tick >= at).length;
    return {
      tick,
      ordnance: { samples },
      probe: {
        gpu: {
          meteorCasts: 1,
          meteorImpacts: impacts,
          participantDamage: impacts * 20,
          meteorHits: impacts,
          participantKills: tick >= 212 ? 1 : 0,
        },
      },
    };
  });
}
const options = {
  epoch: 2,
  castTick: 1,
  aim: [0, 0, 0],
  castsBefore: 0,
  impactsBefore: 0,
  damageBefore: 0,
  hitsBefore: 0,
  killsBefore: 0,
  requireKill: true,
};
test("meteor evidence requires real scheduled, falling, and impact states with source timing", () => {
  const result = evaluateBattleMeteors(timeline(), options);
  assert.equal(result.ok, true, JSON.stringify(result.failures));
  assert.equal(result.summary.impacts, 6);
  assert.equal(result.summary.damageRoll, 140);
});
test("counter-only, early damage, retargeting, and duplicate casts cannot pass meteor evidence", () => {
  for (const mutate of [
    (frames: ReturnType<typeof timeline>) => {
      frames.forEach((f) => {
        f.ordnance.samples = [];
      });
    },
    (frames: ReturnType<typeof timeline>) => {
      frames[100].probe.gpu.participantDamage = 1;
    },
    (frames: ReturnType<typeof timeline>) => {
      frames[160].ordnance.samples[0].target = [99, 0, 0];
    },
    (frames: ReturnType<typeof timeline>) => {
      frames[230].probe.gpu.meteorCasts = 2;
    },
    (frames: ReturnType<typeof timeline>) => {
      frames[10].ordnance.samples[0].damage = 1;
    },
  ]) {
    const frames = timeline();
    mutate(frames);
    assert.equal(evaluateBattleMeteors(frames, options).ok, false);
  }
});
