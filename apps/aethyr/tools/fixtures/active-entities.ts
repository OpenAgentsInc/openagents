import {
  ACTIVE_ALGORITHM,
  ACTIVE_COUNTS,
  ACTIVE_DT,
  ACTIVE_SEED,
  createActiveEntities,
  stepActiveEntities,
  refreshActiveBounds,
  referenceActiveEntities,
  activeStateBytes,
  verifyActiveState,
  activeStorage,
} from "../../experiments/c-town/active-entities.ts";

export interface ActiveBenchmarkOptions {
  counts?: number[];
  warmupSteps?: number;
  steps?: number;
  rounds?: number;
  budgetMs?: number;
}
type Instrumentation = "off" | "on";
async function sha(bytes: Uint8Array<ArrayBuffer>) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}
function clock() {
  let previous = performance.now(),
    minimum = Infinity,
    positive = 0;
  for (let i = 0; i < 10_000; i++) {
    const now = performance.now(),
      elapsed = now - previous;
    if (elapsed > 0) {
      minimum = Math.min(minimum, elapsed);
      positive++;
    }
    previous = now;
  }
  return {
    api: "performance.now",
    timeOrigin: performance.timeOrigin,
    resolutionProbeReads: 10_000,
    minimumPositiveDeltaMs: positive ? minimum : null,
    positiveDeltas: positive,
    crossOriginIsolated: globalThis.crossOriginIsolated === true,
  };
}
function integer(value: number, low: number, high: number) {
  if (!Number.isSafeInteger(value) || value < low || value > high)
    throw new Error(
      "Choose bounded active benchmark steps, trials, and time budget.",
    );
  return value;
}
function checkOptions(options: ActiveBenchmarkOptions) {
  const counts = options.counts ?? [...ACTIVE_COUNTS];
  if (
    !Array.isArray(counts) ||
    !counts.length ||
    new Set(counts).size !== counts.length ||
    counts.some(
      (count) =>
        !ACTIVE_COUNTS.includes(count as (typeof ACTIVE_COUNTS)[number]),
    )
  )
    throw new Error("Choose distinct active populations of 1k, 10k, or 100k.");
  if (![2, 4, 6].includes(options.rounds ?? 4))
    throw new Error("Choose 2, 4, or 6 balanced benchmark rounds.");
  return {
    counts: [...counts],
    warmupSteps: integer(options.warmupSteps ?? 5, 1, 20),
    steps: integer(options.steps ?? 32, 1, 120),
    rounds: integer(options.rounds ?? 4, 2, 6),
    budgetMs: integer(options.budgetMs ?? 10_000, 100, 30_000),
  };
}

async function negativeControls(count: number) {
  const reference = referenceActiveEntities({ count, steps: 1 }),
    expectedHash = await sha(activeStateBytes(reference));
  const results = [];
  for (const name of [
    "changed-entity",
    "skipped-last-entity",
    "stale-state-new-tick",
    "wrong-seed",
    "wrong-dt",
  ] as const) {
    const state = createActiveEntities(
      count,
      name === "wrong-seed" ? ACTIVE_SEED + 1 : ACTIVE_SEED,
      name === "wrong-dt" ? Math.fround(1 / 30) : ACTIVE_DT,
    );
    if (name === "skipped-last-entity") {
      Object.assign(state, { count: count - 1 });
      stepActiveEntities(state);
      Object.assign(state, { count });
    } else if (name === "stale-state-new-tick") state.tick = 1;
    else stepActiveEntities(state);
    if (name === "changed-entity") state.positions[0] += 1;
    refreshActiveBounds(state);
    const verification = verifyActiveState(state, reference),
      actualHash = await sha(activeStateBytes(state));
    if (verification.ok || actualHash === expectedHash)
      throw new Error(`The independent ${name} negative control was accepted.`);
    results.push({
      name,
      rejected: true,
      actualHash,
      expectedHash,
      mismatches: verification.mismatches,
      firstField: verification.firstMismatch?.field,
      hashRecomputedAfterFault: true,
    });
  }
  return results;
}

let active = false;
let latest: unknown;
/** CPU-only baseline. No renderer, inference, Wasm engine, scene, or worker is opened. */
export async function runActiveBenchmark(options: ActiveBenchmarkOptions = {}) {
  if (active) throw new Error("Finish the current active benchmark first.");
  const configuration = checkOptions(options),
    started = performance.now();
  const orders: Instrumentation[][] = [
    ["off", "on"],
    ["on", "off"],
    ["on", "off"],
    ["off", "on"],
    ["off", "on"],
    ["on", "off"],
  ].slice(0, configuration.rounds) as Instrumentation[][];
  const report = {
    schema: "aethyrforge.active-entities-benchmark.v1",
    ok: false,
    startedAt: new Date().toISOString(),
    completedAt: null as string | null,
    algorithm: {
      id: ACTIVE_ALGORITHM,
      seed: ACTIVE_SEED,
      dt: ACTIVE_DT,
      dtFloat32Bits: new Uint32Array(new Float32Array([ACTIVE_DT]).buffer)[0],
      stateFormat: "AFACTV1-little-endian-f32",
      steeringPeriodSteps: 64,
      scope:
        "Every entity updates velocity and position using Float32 Euler arithmetic, enclosure reflection and restitution. Derived bounds refresh separately. No neighbors, gameplay engine, rendering or GPU work is represented.",
    },
    configuration,
    orders,
    timing: clock(),
    samples: [] as {
      count: number;
      round: number;
      order: number;
      instrumentation: Instrumentation;
      batchWallMs: number;
      msPerStep: number;
      frames: {
        simulationMs: number;
        boundsMs: number;
        observedFrameMs: number;
      }[];
      stateHash: string;
      referenceHash: string;
      exact: boolean;
      movingEntities: number;
      positionChangedEntities: number;
      observerRecordsAllocated: number;
      storage: ReturnType<typeof activeStorage>;
      additionalHeldVerificationTypedBytes: number;
    }[],
    populations: [] as {
      count: number;
      referenceHash: string;
      verificationMs: number;
      negatives: Awaited<ReturnType<typeof negativeControls>>;
    }[],
    failures: [] as string[],
    elapsedMs: 0,
    boundaries: [
      "Batch wall cost includes the complete simulation, derived-bound refresh, optional per-step timers and sample writes. Off/on receive identical seed, warmup and measured steps; each condition starts with fresh state.",
      "Per-step off values are batch averages, not distributions of individual frame latency. Instrumented phase samples measure different intervals and must not be summed with inclusive batch costs.",
      "Independent object-reference simulation, finite/state/bounds checks, SHA-256, negative controls and event-loop yields are outside timed intervals. Overall elapsed includes those costs.",
      "Typed-buffer counters cover explicit simulation/bounds storage and known verification copies. JavaScript object allocation, GC, browser heap and process memory are unavailable. This is not a 100k gameplay/rendering capability claim.",
    ],
  };
  latest = report;
  active = true;
  const budget = () => {
    if (performance.now() - started > configuration.budgetMs)
      throw new Error(
        "The active benchmark exceeded its declared time budget; completed samples were retained.",
      );
  };
  try {
    for (const count of configuration.counts) {
      budget();
      const verificationStarted = performance.now();
      const reference = referenceActiveEntities({
        count,
        steps: configuration.warmupSteps + configuration.steps,
      });
      const expectedBytes = activeStateBytes(reference),
        referenceHash = await sha(expectedBytes);
      const initial = createActiveEntities(count).positions;
      const negatives = await negativeControls(count);
      const verificationMs = performance.now() - verificationStarted;
      for (let round = 0; round < orders.length; round++)
        for (let order = 0; order < orders[round].length; order++) {
          budget();
          const instrumentation = orders[round][order],
            state = createActiveEntities(count);
          for (let i = 0; i < configuration.warmupSteps; i++) {
            stepActiveEntities(state);
            refreshActiveBounds(state);
          }
          const frames = new Array<{
            simulationMs: number;
            boundsMs: number;
            observedFrameMs: number;
          }>(instrumentation === "on" ? configuration.steps : 0);
          const batchStarted = performance.now();
          for (let step = 0; step < configuration.steps; step++) {
            if (instrumentation === "on") {
              const start = performance.now();
              stepActiveEntities(state);
              const middle = performance.now();
              refreshActiveBounds(state);
              const end = performance.now();
              frames[step] = {
                simulationMs: middle - start,
                boundsMs: end - middle,
                observedFrameMs: end - start,
              };
            } else {
              stepActiveEntities(state);
              refreshActiveBounds(state);
            }
          }
          const batchWallMs = performance.now() - batchStarted;
          const checked = verifyActiveState(state, reference),
            stateHash = await sha(activeStateBytes(state));
          if (!checked.ok || stateHash !== referenceHash)
            throw new Error(
              `The ${count} ${instrumentation} active state differs from its independent reference.`,
            );
          let positionChangedEntities = 0;
          for (let i = 0; i < count; i++)
            if (
              [0, 1, 2].some(
                (axis) =>
                  state.positions[i * 3 + axis] !== initial[i * 3 + axis],
              )
            )
              positionChangedEntities++;
          if (positionChangedEntities !== count)
            throw new Error("An active entity never changed position.");
          report.samples.push({
            count,
            round,
            order,
            instrumentation,
            batchWallMs,
            msPerStep: batchWallMs / configuration.steps,
            frames,
            stateHash,
            referenceHash,
            exact: true,
            movingEntities: checked.movingEntities,
            positionChangedEntities,
            observerRecordsAllocated: frames.length,
            storage: activeStorage(state),
            additionalHeldVerificationTypedBytes:
              expectedBytes.byteLength + initial.byteLength,
          });
          await new Promise((resolve) => setTimeout(resolve, 0));
        }
      report.populations.push({
        count,
        referenceHash,
        verificationMs,
        negatives,
      });
    }
    budget();
    report.ok = true;
    return report;
  } catch (error) {
    report.failures.push(String(error));
    throw error;
  } finally {
    report.completedAt = new Date().toISOString();
    report.elapsedMs = performance.now() - started;
    active = false;
  }
}

declare global {
  interface Window {
    activeEntityHarness: {
      run: typeof runActiveBenchmark;
      latest: () => unknown;
    };
  }
}
if (typeof window !== "undefined")
  window.activeEntityHarness = {
    run: runActiveBenchmark,
    latest: () => latest,
  };
