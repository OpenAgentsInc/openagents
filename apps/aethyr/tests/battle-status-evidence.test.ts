import assert from "node:assert/strict";
import test from "node:test";
const { battleStatusFreshness, battleCombatProgress } = await import(
  new URL("../tools/battle-evidence.mjs", import.meta.url).href
);

const sample = (frame: number, serial: number, tick: number) => ({
  frame,
  battle: { stats: { sampleSerial: serial, simulationTicks: tick } },
});

test("video and benchmark cohorts require exact populations and direct damage/death progress", () => {
  const probe = (tick: number, n: number) => ({
    count: 125000,
    finite: true,
    tick,
    gpu: {
      ticks: tick,
      alive: 124900,
      dead: 100,
      attacks: n,
      hits: n,
      damage: n * 5,
      deaths: n,
    },
  });
  const before = probe(100, 1),
    after = probe(1900, 1000);
  assert.equal(battleCombatProgress(before, after, 125000).ok, true);
  assert.equal(battleCombatProgress(before, before, 125000).ok, false);
  assert.equal(
    battleCombatProgress(before, { ...after, count: 150000 }, 125000).ok,
    false,
  );
  for (const key of ["damage", "deaths", "hits", "attacks"] as const)
    assert.equal(
      battleCombatProgress(
        before,
        { ...after, gpu: { ...after.gpu, [key]: before.gpu[key] } },
        125000,
      ).ok,
      false,
    );
  assert.equal(
    battleCombatProgress(
      before,
      { ...after, gpu: { ...after.gpu, alive: 124899 } },
      125000,
    ).ok,
    false,
  );
});

test("population status must advance and remain within two simulated seconds of the frozen stop boundary", () => {
  const samples = [
    sample(100, 2, 90),
    sample(130, 3, 120),
    sample(160, 4, 150),
  ];
  const result = battleStatusFreshness(samples, {
    stoppedSimulationSteps: 180,
    directAfterTick: 220,
  });
  assert.equal(result.ok, true);
  assert.equal(result.latestTickLag, 30);
  assert.deepEqual(result.deltas, { frame: 60, serial: 2, tick: 60 });
  assert.equal(result.referenceKind, "benchmark-stop-simulation-steps");
  assert.equal(result.referenceTick, 180);
  assert.equal(result.directAfterTick, 220);
  assert.equal(result.postStopProbeTickAdvance, 40);
  assert.equal(
    battleStatusFreshness(samples, { stoppedSimulationSteps: 270 }).ok,
    true,
  );
  assert.equal(
    battleStatusFreshness(samples, { stoppedSimulationSteps: 271 }).ok,
    false,
  );
});

test("advancing direct GPU probes cannot make frozen HUD population samples valid", () => {
  const frozen = Array.from({ length: 120 }, () => sample(48, 1, 1));
  const result = battleStatusFreshness(frozen, {
    stoppedSimulationSteps: 2288,
    directAfterTick: 2473,
  });
  assert.equal(result.ok, false);
  assert.equal(result.latestTickLag, 2287);
  assert.equal(
    result.checks.filter((check: { ok: boolean }) => !check.ok).length,
    4,
  );
});

test("partial progress, regressions, missing counters and status ahead of its stop boundary are rejected", () => {
  for (const samples of [
    [sample(100, 2, 90), sample(130, 2, 120)],
    [sample(100, 2, 90), sample(130, 3, 90)],
    [sample(100, 2, 90), sample(99, 3, 120)],
    [sample(100, 2, 90), sample(130, 1, 120)],
    [sample(100, 2, 90), sample(130, 3, 80)],
    [sample(100, 2, 90)],
    [{ frame: 100, battle: { stats: {} } }, sample(130, 3, 120)],
  ])
    assert.equal(
      battleStatusFreshness(samples, { stoppedSimulationSteps: 150 }).ok,
      false,
    );
  assert.equal(
    battleStatusFreshness([sample(100, 2, 90), sample(130, 3, 120)], {
      stoppedSimulationSteps: 119,
    }).ok,
    false,
  );
});

test("later GPU and bridge work cannot falsely stale a fresh stopped window", () => {
  const result = battleStatusFreshness(
    [sample(5196, 15, 423), sample(6964, 74, 2227)],
    {
      stoppedSimulationSteps: 2285,
      directAfterTick: 2473,
    },
  );
  assert.equal(result.ok, true);
  assert.equal(result.latestTickLag, 58);
  assert.equal(result.postStopProbeTickAdvance, 188);
  const frozen = battleStatusFreshness([sample(48, 1, 1), sample(48, 1, 1)], {
    stoppedSimulationSteps: 2285,
    directAfterTick: 2473,
  });
  assert.equal(frozen.ok, false);
});

test("missing or invalid authoritative stop references cannot certify population freshness", () => {
  const samples = [sample(100, 2, 90), sample(130, 3, 120)];
  for (const stoppedSimulationSteps of [
    undefined,
    null,
    -1,
    NaN,
    Infinity,
    150.5,
    "150",
  ])
    assert.equal(
      battleStatusFreshness(samples, {
        stoppedSimulationSteps,
        directAfterTick: 150,
      }).ok,
      false,
    );
  assert.equal(battleStatusFreshness(samples).ok, false);
});
