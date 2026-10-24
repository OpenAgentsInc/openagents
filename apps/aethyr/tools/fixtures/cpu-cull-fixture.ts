import { CpuCullExperiment } from "../../packages/runtime/cpu-cull.ts";
import {
  frustumVisibility,
  type InstanceBounds,
} from "../../packages/renderer/culling.ts";
import { lookAt, multiply, perspective } from "../../packages/renderer/math.ts";

const matrices = [
  multiply(perspective(1, 1.6, 0.1, 500), lookAt([0, 3, 10], [0, 3, -50])),
  multiply(
    perspective(0.8, 1.6, 0.1, 500),
    lookAt([45, 20, 35], [-20, 5, -120]),
  ),
  multiply(
    perspective(1.2, 1.6, 0.1, 500),
    lookAt([-40, 8, -100], [45, 20, -200]),
  ),
];
function boundsFor(count: number, seed: number): InstanceBounds {
  if (![1000, 10000, 100000].includes(count))
    throw Error("Choose a bounded fixture population.");
  let state = seed >>> 0;
  const random = () =>
    (state = (Math.imul(state, 1664525) + 1013904223) >>> 0) / 4294967296;
  const spheres = new Float64Array(count * 4),
    aabbs = new Float64Array(count * 6);
  for (let i = 0; i < count; i++) {
    const center = [
      (random() - 0.5) * 320,
      (random() - 0.5) * 120,
      20 - random() * 420,
    ];
    const half = [0.1 + random() * 7, 0.1 + random() * 7, 0.1 + random() * 7];
    spheres.set([...center, Math.hypot(...half)], i * 4);
    aabbs.set(
      [
        ...center.map((n, axis) => n - half[axis]),
        ...center.map((n, axis) => n + half[axis]),
      ],
      i * 6,
    );
  }
  return { count, spheres, aabbs };
}
async function sha(bytes: Uint8Array) {
  const hash = new Uint8Array(
    await crypto.subtle.digest("SHA-256", bytes.slice().buffer),
  );
  return [...hash].map((n) => n.toString(16).padStart(2, "0")).join("");
}
function equal(a: Uint8Array, b: Uint8Array) {
  if (a.length !== b.length)
    throw Error("The worker mask has the wrong length.");
  for (let i = 0; i < a.length; i++)
    if (a[i] !== b[i])
      throw Error(
        `Worker visibility differs from the serial reference at ${i}.`,
      );
}
async function run(
  count: number,
  workers: 0 | 1 | 2,
  iterations = 20,
  warmups = 5,
) {
  if (
    !Number.isInteger(iterations) ||
    iterations < 1 ||
    iterations > 100 ||
    !Number.isInteger(warmups) ||
    warmups < 1 ||
    warmups > 20
  )
    throw Error("Choose bounded warmup and sample counts.");
  const seed = 12999,
    bounds = boundsFor(count, seed);
  const references = matrices.map((matrix) =>
    frustumVisibility(bounds, matrix),
  );
  const referenceHashes = await Promise.all(references.map(sha));
  const pool = new CpuCullExperiment({ workers });
  const started = performance.now();
  try {
    const install = await pool.install(
      { datasetId: `seeded-${seed}-${count}`, generation: 1, bounds },
      { timeoutMs: 5000 },
    );
    const measurements = [];
    for (let i = 0; i < warmups + iterations; i++) {
      const matrixIndex = i % matrices.length;
      const result = await pool.run({
        matrix: matrices[matrixIndex],
        mode: workers ? "workers" : "serial",
        sequence: i + 1,
        timeoutMs: 5000,
      });
      equal(result.mask, references[matrixIndex]);
      if (i >= warmups) {
        const { mask, ...measured } = result;
        measurements.push({
          ...measured,
          matrixIndex,
          hash: await sha(mask),
          exact: true,
        });
      }
    }
    return {
      count,
      workers,
      seed,
      warmups,
      iterations,
      install,
      referenceHashes,
      visible: references.map((mask) => mask.reduce((sum, n) => sum + n, 0)),
      measurements,
      elapsedMs: performance.now() - started,
      snapshot: pool.snapshot(),
      boundary:
        "Run timings exclude reference comparison and SHA-256; elapsedMs includes them. Installation is separate. Byte counters cover the experiment buffers, not browser or process memory.",
    };
  } finally {
    pool.dispose();
  }
}
async function lifecycle() {
  const bounds = boundsFor(100000, 12999);
  const canceledPool = new CpuCullExperiment({ workers: 2 });
  await canceledPool.install(
    { datasetId: "abort-fixture", generation: 1, bounds },
    { timeoutMs: 5000 },
  );
  const controller = new AbortController();
  const canceled = canceledPool
    .run({
      matrix: matrices[0],
      mode: "workers",
      signal: controller.signal,
      timeoutMs: 5000,
    })
    .then(
      () => ({ resolved: true }),
      (error: unknown) => ({ resolved: false, error: String(error) }),
    );
  controller.abort();
  const abortResult = await canceled;
  canceledPool.dispose();
  if (abortResult.resolved)
    throw Error("An immediately aborted worker job resolved.");
  const closedPool = new CpuCullExperiment({ workers: 2 });
  await closedPool.install(
    { datasetId: "close-fixture", generation: 1, bounds },
    { timeoutMs: 5000 },
  );
  const closed = closedPool
    .run({ matrix: matrices[1], mode: "workers", timeoutMs: 5000 })
    .then(
      () => ({ resolved: true }),
      (error: unknown) => ({ resolved: false, error: String(error) }),
    );
  closedPool.dispose();
  const closeResult = await closed;
  if (closeResult.resolved) throw Error("A closed worker job resolved.");
  return {
    abortResult,
    closeResult,
    afterAbort: canceledPool.snapshot(),
    afterClose: closedPool.snapshot(),
  };
}

declare global {
  interface Window {
    cpuCullHarness: { run: typeof run; lifecycle: typeof lifecycle };
  }
}
window.cpuCullHarness = { run, lifecycle };
