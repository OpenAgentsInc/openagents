import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  ACTIVE_ALGORITHM,
  ACTIVE_DT,
  ACTIVE_SEED,
  createActiveEntities,
  stepActiveEntities,
  refreshActiveBounds,
  referenceActiveEntities,
  activeStateBytes,
  verifyActiveState,
  activeStorage,
} from "../experiments/c-town/active-entities.ts";
import { runActiveBenchmark } from "../tools/fixtures/active-entities.ts";

const hash = (state: Parameters<typeof activeStateBytes>[0]) =>
  createHash("sha256").update(activeStateBytes(state)).digest("hex");
test("seeded active populations match independent initialization and 70 moving steps across steering changes", () => {
  for (const count of [1, 17, 1000]) {
    const state = createActiveEntities(count),
      initial = hash(state);
    assert.equal(hash(referenceActiveEntities({ count, steps: 0 })), initial);
    const positions = state.positions.slice();
    for (let i = 0; i < 70; i++) {
      stepActiveEntities(state);
      refreshActiveBounds(state);
    }
    const reference = referenceActiveEntities({ count, steps: 70 });
    assert.deepEqual(verifyActiveState(state, reference), {
      ok: true,
      mismatches: 0,
      movingEntities: count,
    });
    assert.equal(hash(state), hash(reference));
    assert.notEqual(hash(state), initial);
    assert.ok(state.positions.some((value, i) => value !== positions[i]));
  }
});

test("all supported stress sizes update their last entity and expose separate derived-bound work", () => {
  for (const count of [1000, 10000, 100000]) {
    const state = createActiveEntities(count),
      last = state.positions.slice(-3);
    stepActiveEntities(state);
    assert.notDeepEqual(state.positions.slice(-3), last);
    assert.equal(state.boundsTick, 0);
    const reference = referenceActiveEntities({ count, steps: 1 });
    assert.equal(verifyActiveState(state, reference).ok, false);
    refreshActiveBounds(state);
    assert.equal(verifyActiveState(state, reference).ok, true);
    assert.equal(activeStorage(state).residentTypedBytes, count * 124);
    assert.equal(activeStorage(state).timedLoopAllocatedTypedBytes, 0);
  }
});

test("one known three-axis boundary crossing clamps position and reverses Float32 velocity", () => {
  const state = createActiveEntities(1);
  state.positions.set([-159.5, 59.5, -399.5]);
  state.velocities.set([-2, 2, -3]);
  state.halfExtents.fill(0.5);
  state.acceleration.fill(0);
  stepActiveEntities(state);
  refreshActiveBounds(state);
  assert.deepEqual([...state.positions], [-159.5, 59.5, -399.5]);
  assert.deepEqual(
    [...state.velocities],
    [1.600000023841858, -1.59333336353302, 2.4000000953674316],
  );
  assert.equal(state.bounds.aabbs[0], -160);
  assert.equal(state.bounds.aabbs[4], 60);
  assert.equal(state.bounds.aabbs[2], -400);
});

test("newly recomputed hashes cannot bless changed, skipped, stale, wrong-seed or wrong-timestep state", () => {
  const count = 17,
    reference = referenceActiveEntities({ count, steps: 1 });
  const referenceHash = hash(reference);
  for (const fault of ["changed", "skipped", "stale", "seed", "dt"] as const) {
    const state = createActiveEntities(
      count,
      fault === "seed" ? ACTIVE_SEED + 1 : ACTIVE_SEED,
      fault === "dt" ? Math.fround(1 / 30) : ACTIVE_DT,
    );
    if (fault === "skipped") {
      // Simulate an off-by-one loop while keeping the declared population intact.
      Object.assign(state, { count: count - 1 });
      stepActiveEntities(state);
      Object.assign(state, { count });
    } else if (fault === "stale") state.tick = 1;
    else stepActiveEntities(state);
    if (fault === "changed") state.positions[3] += 1;
    refreshActiveBounds(state);
    const actual = verifyActiveState(state, reference);
    assert.equal(actual.ok, false, fault);
    assert.ok(actual.mismatches > 0);
    assert.notEqual(hash(state), referenceHash, fault);
  }
});

test("canonical bytes bind algorithm format, population, seed, tick and Float32 time with exact little endian layout", () => {
  const state = createActiveEntities(1, 0),
    bytes = activeStateBytes(state),
    view = new DataView(bytes.buffer);
  assert.equal(state.algorithm, ACTIVE_ALGORITHM);
  assert.equal(bytes.byteLength, 68);
  assert.deepEqual([...bytes.slice(0, 8)], [65, 70, 65, 67, 84, 86, 49, 0]);
  assert.equal(view.getUint32(8, true), 1);
  assert.equal(view.getUint32(12, true), 0);
  assert.equal(view.getUint32(16, true), 0);
  assert.equal(view.getFloat32(20, true), ACTIVE_DT);
  assert.equal(view.getFloat32(24, true), state.positions[0]);
  bytes.fill(0);
  assert.equal(
    hash(state),
    hash(referenceActiveEntities({ count: 1, seed: 0, steps: 0 })),
  );
  for (const args of [
    [0, 1, ACTIVE_DT],
    [100001, 1, ACTIVE_DT],
    [1, -1, ACTIVE_DT],
    [1, 0.1, ACTIVE_DT],
    [1, 1, 1 / 60],
    [1, 1, NaN],
  ])
    assert.throws(() =>
      createActiveEntities(...(args as [number, number, number])),
    );
});

test("CPU fixture records balanced whole batches and keeps verification and phase samples outside the off interval", async () => {
  const report = await runActiveBenchmark({
    counts: [1000],
    warmupSteps: 1,
    steps: 2,
    rounds: 2,
  });
  assert.equal(report.ok, true);
  assert.deepEqual(report.orders, [
    ["off", "on"],
    ["on", "off"],
  ]);
  assert.equal(report.samples.length, 4);
  for (const sample of report.samples) {
    assert.equal(sample.stateHash, report.populations[0].referenceHash);
    assert.equal(sample.positionChangedEntities, 1000);
    assert.equal(sample.frames.length, sample.instrumentation === "on" ? 2 : 0);
    assert.ok(sample.batchWallMs >= 0);
    assert.equal(sample.observerRecordsAllocated, sample.frames.length);
    for (const frame of sample.frames) {
      assert.ok(frame.simulationMs >= 0 && frame.boundsMs >= 0);
      assert.ok(
        Math.abs(frame.simulationMs + frame.boundsMs - frame.observedFrameMs) <
          1e-6,
      );
    }
  }
  assert.equal(report.populations[0].negatives.length, 5);
  assert.ok(
    report.populations[0].negatives.every(
      (negative) => negative.rejected && negative.hashRecomputedAfterFault,
    ),
  );
  await assert.rejects(runActiveBenchmark({ counts: [1000], rounds: 3 }));
  await assert.rejects(runActiveBenchmark({ counts: [1000, 1000] }));
});
