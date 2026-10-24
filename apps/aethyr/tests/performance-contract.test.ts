import assert from "node:assert/strict";
import test from "node:test";

const { evaluateFramePacing } = await import(
  new URL("../tools/performance-contract.mjs", import.meta.url).href
);

function fixture(count = 1800, hz = 60) {
  const interval = 1000 / hz;
  const samples = Array.from({ length: count }, (_, index) => ({
    frame: index + 10,
    at: 10_000 + index * interval,
    elapsedMs: interval,
    submitted: true,
    fullGpuMs: 2 as number | null,
    gpuMs: 1,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
  }));
  return {
    recording: false,
    truncated: false,
    samples,
    completionTruncated: false,
    completionRange: { firstFrame: 10, lastFrame: count + 9 },
    observation: {
      startedAtMs: 10_000,
      stoppedAtMs: 10_000 + count * interval,
    },
    completedFrames: samples.map((sample) => ({
      frame: sample.frame,
      submittedAt: sample.at + 0.1,
      completedAt: sample.at + 2.1,
      latencyMs: 2,
      ok: true,
    })),
  };
}
const options = {
  durationMs: 30_000,
  warmupMs: 5_000,
  mode: "qualification",
  minHz: 60,
};
function failed(result: ReturnType<typeof evaluateFramePacing>, id: string) {
  assert.equal(
    result.checks.find((entry: { id: string }) => entry.id === id)?.ok,
    false,
    id,
  );
}

test("strict sustained qualification preserves exact rates, GPU scopes, and input observations", () => {
  const input = fixture(),
    before = structuredClone(input);
  const result = evaluateFramePacing(input, options);
  assert.equal(result.ok, true);
  assert.equal(result.qualified, true);
  assert.equal(result.queueQualified, true);
  assert.equal(result.gpuBudgetQualified, true);
  assert.equal(result.summary.completionRateHz, 60);
  assert.equal(result.summary.fullGpuMs.p50, 2);
  assert.equal(result.summary.mainHdrGpuMs.p50, 1);
  assert.equal(result.summary.presentationMs, null);
  assert.ok(
    result.limitations.some((item: string) =>
      item.includes("two independent repeats"),
    ),
  );
  assert.deepEqual(input, before);
});

test("paced samples keep legacy counts explicitly scoped and preserve actual raw callback counts", () => {
  const input = Object.assign(fixture(), {
    targetHz: 60,
    sampleScope: "render-attempts",
    callbackCounts: {
      rawCallbacks: 3600,
      intentionalPacedCallbacks: 1800,
      renderAttempts: 1800,
    },
  });
  const result = evaluateFramePacing(input, options);
  assert.equal(result.qualified, true);
  assert.equal(result.summary.callbacks, 1800);
  assert.equal(result.summary.callbackScope, "render-attempts");
  assert.equal(result.summary.renderAttempts, 1800);
  assert.equal(result.summary.rawCallbacks, 3600);
  assert.equal(result.summary.intentionalPacedCallbacks, 1800);
  assert.equal(result.summary.targetHz, 60);
});
test("120 queued frames per second with only 30 completed cannot pass", () => {
  const input = fixture(3600, 120);
  input.completedFrames = input.completedFrames.filter(
    (_, index) => index % 4 === 0,
  );
  const result = evaluateFramePacing(input, options);
  assert.equal(result.ok, false);
  assert.equal(result.qualified, false);
  assert.equal(result.summary.completionRateHz, 30);
  failed(result, "completion.coverage");
  failed(result, "completion.count");
  failed(result, "completion.interval-p95");
});

test("long-tail stalls, slow fractions, and deep queues fail despite enough completions", () => {
  const stalled = fixture();
  for (const frame of stalled.completedFrames.slice(900)) {
    frame.completedAt += 40;
    frame.latencyMs += 40;
  }
  const stallResult = evaluateFramePacing(stalled, options);
  assert.equal(stallResult.ok, false);
  failed(stallResult, "completion.interval-max");
  failed(stallResult, "completion.stalls");
  const slow = fixture();
  let delay = 0;
  slow.completedFrames.forEach((frame, index) => {
    if ([500, 1000, 1500].includes(index)) delay += 12;
    frame.completedAt += delay;
    frame.latencyMs += delay;
  });
  const slowResult = evaluateFramePacing(slow, options);
  failed(slowResult, "completion.slow-fraction");
  const queued = fixture();
  for (const frame of queued.completedFrames) {
    frame.completedAt += 100;
    frame.latencyMs += 100;
  }
  const queueResult = evaluateFramePacing(queued, options);
  assert.equal(queueResult.summary.completionIntervalMs.p95 < 17.5, true);
  failed(queueResult, "completion.latency-p95");
  failed(queueResult, "completion.latency-p99");
  failed(queueResult, "completion.latency-max");
});

test("missing, duplicate, failed, inconsistent, out-of-range, and truncated completion evidence fails", () => {
  for (const kind of [
    "missing",
    "duplicate",
    "outside",
    "range",
    "failed",
    "latency",
    "truncated",
    "order",
    "recording",
  ] as const) {
    const input = fixture();
    if (kind === "missing") input.completedFrames.pop();
    if (kind === "duplicate")
      input.completedFrames[10].frame = input.completedFrames[9].frame;
    if (kind === "outside") input.completedFrames[10].frame = 9000;
    if (kind === "range") input.completionRange.lastFrame++;
    if (kind === "failed") input.completedFrames[10].ok = false;
    if (kind === "latency") input.completedFrames[10].latencyMs = 200;
    if (kind === "truncated") input.completionTruncated = true;
    if (kind === "order")
      [input.completedFrames[9], input.completedFrames[10]] = [
        input.completedFrames[10],
        input.completedFrames[9],
      ];
    if (kind === "recording") input.recording = true;
    const result = evaluateFramePacing(input, options);
    assert.equal(result.ok, false, kind);
    assert.equal(result.qualified, false, kind);
  }
});

test("batched equal fence times are insufficient evidence of smooth queue pacing", () => {
  const input = fixture();
  input.completedFrames.forEach((frame, index) => {
    frame.completedAt =
      input.samples[Math.min(input.samples.length - 1, index | 1)].at + 2.1;
    frame.latencyMs = frame.completedAt - frame.submittedAt;
  });
  const result = evaluateFramePacing(input, options);
  failed(result, "completion.cadence-evidence");
  assert.equal(result.summary.equalCompletionTimes, 900);
  assert.ok(
    result.limitations.some((item: string) =>
      item.includes("without proving the display was unsmooth"),
    ),
  );
});

test("unavailable all-pass GPU timestamps stay null and cannot establish a GPU-budget result", () => {
  const input = fixture();
  input.samples.forEach((sample) => {
    sample.fullGpuMs = null;
  });
  const strict = evaluateFramePacing(input, options);
  assert.equal(strict.ok, false);
  assert.equal(strict.qualified, false);
  assert.equal(strict.queueQualified, true);
  assert.equal(strict.gpuBudgetQualified, false);
  assert.equal(strict.summary.fullGpuMs, null);
  assert.equal(strict.summary.missingFullGpuSamples, 1800);
  assert.equal(strict.summary.mainHdrGpuMs.p50, 1);
  failed(strict, "gpu.availability");
  const queueOnly = evaluateFramePacing(input, {
    ...options,
    requireFullGpu: false,
  });
  assert.equal(queueOnly.ok, true);
  assert.equal(queueOnly.qualified, true);
  assert.equal(queueOnly.gpuBudgetQualified, false);
  assert.equal(
    queueOnly.checks.find(
      (item: { id: string }) => item.id === "gpu.availability",
    ).required,
    false,
  );
  const expensive = fixture();
  expensive.samples.forEach((sample) => {
    sample.fullGpuMs = 25;
  });
  const gpuResult = evaluateFramePacing(expensive, options);
  assert.equal(gpuResult.queueQualified, true);
  failed(gpuResult, "gpu.budget-p95");
});

test("short diagnostics never qualify and exact boundary counts are not rounded to 60", () => {
  const short = fixture(60);
  const diagnostic = evaluateFramePacing(short, {
    durationMs: 1000,
    warmupMs: 500,
    mode: "diagnostic",
  });
  assert.equal(diagnostic.ok, true);
  assert.equal(diagnostic.qualified, false);
  assert.equal(diagnostic.queueQualified, false);
  const attempted = evaluateFramePacing(short, {
    ...options,
    durationMs: 1000,
    warmupMs: 500,
  });
  assert.equal(attempted.qualified, false);
  failed(attempted, "qualification.duration");
  failed(attempted, "qualification.warmup");
  const boundary = fixture(1799);
  const result = evaluateFramePacing(boundary, options);
  assert.equal(result.ok, true);
  assert.equal(result.summary.completionRateHz, 1799 / 30);
  assert.notEqual(result.summary.completionRateHz, 60);
});

test("missing or increasing dropped-time counters and incomplete callback coverage fail", () => {
  const dropped = fixture();
  dropped.samples.at(-1)!.kernelDroppedSeconds = 0.01;
  dropped.samples.at(-1)!.hostClampedSeconds = 0.02;
  const result = evaluateFramePacing(dropped, options);
  failed(result, "time.kernelDroppedSeconds");
  failed(result, "time.hostClampedSeconds");
  const missing = fixture();
  Reflect.deleteProperty(missing.samples[10], "kernelDroppedSeconds");
  failed(evaluateFramePacing(missing, options), "time.kernelDroppedSeconds");
  const coverage = fixture(1800, 120);
  failed(evaluateFramePacing(coverage, options), "samples.coverage");
});

test("completion timestamps must belong to their real window and matching submitted callbacks", () => {
  for (const offset of [-1000, 1000]) {
    const input = fixture();
    input.completedFrames.forEach((frame) => {
      frame.submittedAt += offset;
      frame.completedAt += offset;
    });
    const result = evaluateFramePacing(input, options);
    failed(result, "completion.timeline");
    assert.equal(result.qualified, false);
  }
  const missing = fixture();
  Reflect.deleteProperty(missing, "observation");
  failed(evaluateFramePacing(missing, options), "completion.timeline");
});

test("all-pass GPU timing must contain every paired main-HDR interval", () => {
  const input = fixture();
  input.samples.forEach((sample) => {
    sample.fullGpuMs = 0;
    sample.gpuMs = 22.9;
  });
  for (const mode of ["diagnostic", "qualification"]) {
    const result = evaluateFramePacing(input, {
      ...options,
      mode,
      requireFullGpu: true,
    });
    assert.equal(result.ok, false);
    assert.equal(result.qualified, false);
    assert.equal(result.gpuBudgetQualified, false);
    assert.equal(result.summary.fullGpuScopeViolations, 1800);
    failed(result, "gpu.main-hdr-contained");
  }
  input.samples.forEach((sample) => {
    sample.fullGpuMs = 0.96;
    sample.gpuMs = 1;
  });
  assert.equal(evaluateFramePacing(input, options).ok, true);
  input.samples[13].fullGpuMs = 0.9;
  const isolated = evaluateFramePacing(input, options);
  failed(isolated, "gpu.main-hdr-contained");
  assert.equal(isolated.summary.fullGpuScopeViolations, 1);
});
