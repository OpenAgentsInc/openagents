import test from "node:test";
import assert from "node:assert/strict";
import { performanceObserverPlan } from "../tools/performance-observers.mjs";

test("profiler defaults preserve on-only qualification and diagnostic windows", () => {
  for (const mode of [null, "diagnostic", "qualification"] as const)
    assert.deepEqual(performanceObserverPlan(mode), {
      observers: ["on"],
      schedule: [["on"], ["on"]],
    });
});
test("diagnostic cohorts retain requested first order and reverse repeated pairs", () => {
  assert.deepEqual(
    performanceObserverPlan("diagnostic", "off,on", 2).schedule,
    [
      ["off", "on"],
      ["on", "off"],
    ],
  );
  assert.deepEqual(
    performanceObserverPlan("diagnostic", "on,off", 1).schedule,
    [["on", "off"]],
  );
  assert.deepEqual(performanceObserverPlan("diagnostic", "off", 2).schedule, [
    ["off"],
    ["off"],
  ]);
});
test("off profiling cannot enter qualification or an implicit legacy performance mode", () => {
  for (const mode of [null, "qualification"] as const)
    for (const selection of ["off", "off,on", "on,off"])
      assert.throws(
        () => performanceObserverPlan(mode, selection),
        /diagnostic mode/,
      );
  for (const selection of ["", "off,off", "on,on", "none", "off,on,extra"])
    assert.throws(() => performanceObserverPlan("diagnostic", selection));
});
