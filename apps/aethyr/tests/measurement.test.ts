import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import {
  distribution,
  statistics,
  summarizeResponseBodies,
  balancedOrder,
  summarizeFrames,
  archiveEvidence,
  createMeasurementReport,
  validateMeasurementReport,
  checkByteBudgets,
  type MeasurementReport,
  type FrameObservation,
  type PlayerByteBudget,
} from "../experiments/c-town/measurement.mjs";
const hash = (bytes: string | Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
const report = (): Omit<MeasurementReport, "schema" | "quantile"> => ({
  id: "owned-instrumentation-comparison",
  startedAt: "2026-10-23T20:00:00Z",
  finishedAt: "2026-10-23T20:00:01Z",
  ok: true,
  environment: { browser: null, os: "test", gpu: null },
  pins: {
    source: { revision: "a".repeat(40), dirty: false, patchSha256: hash("") },
    assets: { sha256: null, scope: "Owned CPU arrays; no asset package." },
    kernel: { sha256: null, abi: null },
    wgsl: { sha256: null, scope: "CPU-only; no shader." },
    quality: { sha256: hash("quality"), settings: { count: 1000 } },
    build: { mode: "cpu", sha256: null, toolchain: { runtime: "test" } },
  },
  cache: {
    state: "not-applicable",
    scope: "CPU arrays; no network. OS caches are not flushed.",
  },
  warmup: { durationMs: null, iterations: 5 },
  trialOrder: balancedOrder(["off", "on"], 2),
  phases: [
    {
      id: "active-euler",
      kind: "simulation-cpu",
      scope: "Fixed-step update; hash checking outside measured work.",
      metrics: { batchMs: 1, mainGpuMs: null },
    },
  ],
  evidence: [],
  limitations: [
    "Synthetic CPU workload; no rendering or presentation measurement.",
  ],
});

test("shared distributions preserve exact legacy interpolation, real zeroes, and unavailable observations", () => {
  const values = [1, 1, 1, 1, 80],
    result = distribution(values)!;
  assert.equal(result.p50, 1);
  assert.equal(result.p95, 64.19999999999999);
  assert.equal(statistics(values).median, result.p50);
  assert.equal(statistics(values).p95, result.p95);
  assert.deepEqual(distribution([null, 0, undefined, 10]), {
    count: 2,
    min: 0,
    p50: 5,
    p95: 9.5,
    p99: 9.9,
    max: 10,
    mean: 5,
    missing: 2,
  });
  assert.equal(distribution([]), null);
  assert.equal(distribution([null, undefined]), null);
  assert.equal(distribution([0])!.mean, 0);
  assert.deepEqual(values, [1, 1, 1, 1, 80]);
  for (const invalid of [[NaN], [Infinity], [-1]])
    assert.throws(() => distribution(invalid), /finite/);
  assert.throws(() => statistics([]));
  assert.throws(() => statistics([null as unknown as number]));
});

test("browser byte summaries reject negative or missing observations without inventing zero-byte responses", () => {
  assert.deepEqual(
    summarizeResponseBodies([
      { responseBodySize: 0 },
      { responseBodySize: 32 },
    ]),
    {
      responseBodyBytes: 32,
      knownResponseBodyBytes: 32,
      responses: 2,
      unavailableResponses: 0,
      invalidResponses: 0,
    },
  );
  assert.deepEqual(
    summarizeResponseBodies([
      { responseBodySize: -5287 },
      {},
      { responseBodySize: 12 },
    ]),
    {
      responseBodyBytes: null,
      knownResponseBodyBytes: 12,
      responses: 3,
      unavailableResponses: 2,
      invalidResponses: 1,
    },
  );
  assert.equal(
    summarizeResponseBodies([{ responseBodySize: 0 }]).responseBodyBytes,
    0,
  );
  assert.equal(summarizeResponseBodies([]).responseBodyBytes, 0);
  for (const invalid of [NaN, Infinity, -1, 1.5, "10"])
    assert.equal(
      summarizeResponseBodies([{ responseBodySize: invalid }])
        .responseBodyBytes,
      null,
    );
});

test("balanced paired trials provide ABBA and balance each candidate position across complete rotations", () => {
  assert.deepEqual(balancedOrder(["off", "on"], 4), [
    ["off", "on"],
    ["on", "off"],
    ["on", "off"],
    ["off", "on"],
  ]);
  const candidates = ["a", "b", "c"],
    orders = balancedOrder(candidates, 6);
  for (const candidate of candidates)
    for (let position = 0; position < 3; position++)
      assert.equal(
        orders.filter((order) => order[position] === candidate).length,
        2,
      );
  assert.deepEqual(candidates, ["a", "b", "c"]);
  assert.throws(() => balancedOrder(["same", "same"], 2));
  assert.throws(() => balancedOrder(["a"], 0));
});

test("frame summaries separate callbacks, accepted submissions, unavailable timers, and cumulative dropped time", () => {
  const samples: FrameObservation[] = [
    {
      at: 10,
      elapsedMs: 10,
      submitted: true,
      stepMs: null,
      renderCpuMs: 4,
      gpuMs: 9,
      kernelDroppedSeconds: 1,
      hostClampedSeconds: 0.1,
    },
    {
      at: 20,
      elapsedMs: 10,
      submitted: false,
      stepMs: null,
      renderCpuMs: 0,
      gpuMs: null,
      kernelDroppedSeconds: 2,
      hostClampedSeconds: 0.2,
    },
    {
      at: 30,
      elapsedMs: 10,
      submitted: true,
      stepMs: null,
      renderCpuMs: 6,
      gpuMs: 12,
      kernelDroppedSeconds: 3,
      hostClampedSeconds: 0.4,
    },
  ];
  const summary = summarizeFrames(samples);
  assert.equal(summary.callbacks, 3);
  assert.equal(summary.acceptedSubmissions, 2);
  assert.equal(summary.skippedCallbacks, 1);
  assert.equal(summary.acceptedSubmissionsPerSecond, 50);
  assert.equal(summary.submissionIntervalMs!.p50, 20);
  assert.equal(summary.metrics.stepMs, null);
  assert.equal(summary.metrics.commandEncodeMs, null);
  assert.equal(summary.metrics.gpuMs!.count, 2);
  assert.equal(summary.acceptedMetrics.renderCpuMs!.p50, 5);
  assert.equal(summary.dropped.kernelSeconds, 2);
  assert.ok(Math.abs(summary.dropped.hostClampedSeconds! - 0.3) < 1e-9);
  assert.equal(summary.presentationMs, null);
  assert.match(summary.scopes.commandEncodeMs, /contains nested/);
  assert.match(summary.scopes.wasmRequestMs, /not isolated simulation math/);
  assert.throws(() =>
    summarizeFrames([...samples, { at: 5, submitted: true }]),
  );
  assert.throws(() =>
    summarizeFrames([{ at: 0, submitted: false, gpuMs: NaN }]),
  );
  assert.throws(() =>
    summarizeFrames([
      { at: 0, submitted: false, kernelDroppedSeconds: 2 },
      { at: 1, submitted: true, kernelDroppedSeconds: 1 },
    ]),
  );
});

test("empty and partially unavailable windows never fabricate cadence, GPU, or dropped-time values", () => {
  const empty = summarizeFrames([]);
  assert.equal(empty.callbacks, 0);
  assert.equal(empty.metrics.gpuMs, null);
  assert.equal(empty.submissionIntervalMs, null);
  assert.equal(empty.acceptedSubmissionsPerSecond, null);
  assert.equal(empty.dropped.kernelSeconds, null);
  const partial = summarizeFrames([
    { at: 0, submitted: false, kernelDroppedSeconds: 1 },
    { at: 20, submitted: false },
  ]);
  assert.equal(partial.acceptedSubmissionsPerSecond, 0);
  assert.equal(partial.dropped.kernelSeconds, null);
});

test("native evidence retains its schema, exact byte identity, and original quantile limitations", () => {
  const bytes = new TextEncoder().encode('{"schema":"native.v1","p95":9}\n');
  const archived = archiveEvidence([
    {
      path: "issue4/raw.json",
      bytes,
      issue: 4,
      limitations: ["Original nearest-rank quantile; not recomputed."],
    },
  ]);
  assert.deepEqual(archived[0], {
    path: "issue4/raw.json",
    sha256: hash(bytes),
    bytes: bytes.length,
    schema: "native.v1",
    issue: 4,
    limitations: ["Original nearest-rank quantile; not recomputed."],
  });
  const changed = new TextEncoder().encode(
    new TextDecoder().decode(bytes) + " ",
  );
  assert.notEqual(
    archiveEvidence([
      {
        path: "issue4/raw.json",
        bytes: changed,
        limitations: ["Original evidence."],
      },
    ])[0].sha256,
    archived[0].sha256,
  );
  assert.throws(() => archiveEvidence([{ path: "x", bytes, limitations: [] }]));
  assert.throws(() =>
    archiveEvidence([
      {
        path: "x",
        bytes: new TextEncoder().encode("{}"),
        limitations: ["None"],
      },
    ]),
  );
});

test("versioned reports pin build, quality, cache, and trial context while preserving native details", () => {
  const input = report(),
    result = createMeasurementReport({
      ...input,
      details: { schema: "native.cpu.v1", warmup: 5 },
    });
  assert.equal(result.schema, "aethyrforge.measurement.v1");
  assert.equal(result.quantile, "linear-n-minus-one");
  result.pins.quality.settings.count = 2;
  assert.equal(input.pins.quality.settings.count, 1000);
  for (const change of [
    (v: MeasurementReport) => {
      v.pins.kernel.sha256 = "wrong";
    },
    (v: MeasurementReport) => {
      v.phases[0].metrics.batchMs = NaN;
    },
    (v: MeasurementReport) => {
      v.finishedAt = "2026-10-22T00:00:00Z";
    },
    (v: MeasurementReport) => {
      v.cache.scope = "";
    },
    (v: MeasurementReport) => {
      v.phases[0].scope = "";
    },
    (v: MeasurementReport) => {
      v.limitations = [];
    },
  ]) {
    const candidate = createMeasurementReport(report());
    change(candidate);
    assert.throws(() => validateMeasurementReport(candidate));
  }
});

test("shared byte-budget checks use exact audited sizes and fail excess, unavailable values, or invalid graphs", () => {
  const audit = {
    ok: true,
    requests: 4,
    totals: {
      js: { rawBytes: 100, gzipBytes: 50 },
      wasm: { rawBytes: 0, gzipBytes: 0 },
      assets: { rawBytes: 30, gzipBytes: 25 },
    },
  };
  const budget: PlayerByteBudget = {
    jsRawBytes: 100,
    jsGzipBytes: 50,
    wasmRawBytes: 0,
    assetRawBytes: 30,
    deliveredFiles: 4,
  };
  assert.equal(checkByteBudgets(audit, budget).ok, true);
  const excess = checkByteBudgets(audit, { ...budget, jsRawBytes: 99 });
  assert.equal(excess.ok, false);
  assert.match(excess.violations[0], /100 exceeds 99/);
  assert.equal(checkByteBudgets({ ...audit, ok: false }, budget).ok, false);
  assert.throws(() =>
    checkByteBudgets(audit, { ...budget, assetRawBytes: Infinity }),
  );
  assert.throws(() => checkByteBudgets({ ...audit, requests: NaN }, budget));
  assert.throws(() => checkByteBudgets(audit, {} as PlayerByteBudget));
});
