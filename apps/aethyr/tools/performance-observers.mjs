import assert from "node:assert/strict";
import { balancedOrder } from "../experiments/c-town/measurement.mjs";

/** Optional profiler cohorts are selectable only for unqualified diagnostics. */
export function performanceObserverPlan(mode, selection = "on", repeats = 2) {
  const observers = selection.split(",");
  assert(
    observers.length &&
      new Set(observers).size === observers.length &&
      observers.every((value) => value === "off" || value === "on"),
    "Choose distinct off/on performance observer cohorts.",
  );
  assert(Number.isInteger(repeats) && repeats >= 1 && repeats <= 4);
  assert(
    mode === "diagnostic" || (observers.length === 1 && observers[0] === "on"),
    "Optional profiling can be disabled only in diagnostic mode; qualification requires on.",
  );
  return { observers, schedule: balancedOrder(observers, repeats) };
}
