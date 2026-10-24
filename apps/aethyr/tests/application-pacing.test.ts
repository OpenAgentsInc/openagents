import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import test from "node:test";

const {
  evaluateApplicationPacing: evaluateRaw,
  applicationBrowserMode,
  observeApplicationBrowserMode,
} = await import(
  new URL("../tools/application-pacing.mjs", import.meta.url).href
);
const { evaluateFramePacing } = await import(
  new URL("../tools/performance-contract.mjs", import.meta.url).href
);

const evaluateApplicationPacing = (input: any, settings: any = {}) =>
  evaluateRaw(input, { productCounters: input.productCounters, ...settings });

function fixture(count = 1800, hz = 60) {
  const samples = Array.from({ length: count }, (_, index) => ({
    frame: index + 10,
    at: 10000 + (index * 1000) / hz,
    submitted: true,
    elapsedMs: 1000 / hz,
    fullGpuMs: 5 as number | null,
    gpuMs: 3,
    stepMs: 1,
    renderCpuMs: 3,
    commandEncodeMs: 2,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
  }));
  const accepted = samples.filter((sample) => sample.submitted);
  const beforeCounters = {
    frameAttempts: 0,
    submittedFrames: 0,
    skippedFrames: 0,
    rawCallbacks: 0,
    intentionalPacedCallbacks: 0,
    kernelDroppedSeconds: 0,
    hostClampedSeconds: 0,
  };
  const afterCounters = {
    ...beforeCounters,
    frameAttempts: count,
    submittedFrames: count,
    rawCallbacks: hz === 60 ? count * 2 : count,
    intentionalPacedCallbacks: hz === 60 ? count : 0,
  };
  return {
    productCounters: { before: beforeCounters, after: afterCounters },
    resources: {
      frameCounters: structuredClone(afterCounters),
      renderer: {
        fullGpuScope: "render-compute-pass-envelope",
        gpuTimingStatus: {
          scope: "render-compute-pass-envelope",
          availability: "available",
          observedFrames: accepted.length,
          validFrames: accepted.length,
          unavailableFrames: 0,
          pendingFrames: 0,
          lastFrame: accepted.at(-1)?.frame ?? null,
        },
      },
    },
    recording: false,
    truncated: false,
    sampleScope: "render-attempts",
    targetHz: hz === 60 ? 60 : "display",
    callbackCounts: {
      rawCallbacks: hz === 60 ? count * 2 : count,
      intentionalPacedCallbacks: hz === 60 ? count : 0,
      suspendedCallbacks: 0,
      renderAttempts: count,
    },
    samples,
    observation: {
      startedAtMs: 10000,
      stoppedAtMs: 10000 + (count * 1000) / hz,
    },
    completionTruncated: false,
    completionRange: {
      firstFrame: accepted[0]?.frame ?? null,
      lastFrame: accepted.at(-1)?.frame ?? null,
    },
    completedFrames: accepted.map((sample) => ({
      frame: sample.frame,
      submittedAt: sample.at + 0.1,
      submitReturnedAt: sample.at + 0.2,
      completedAt: sample.at + 2.1,
      latencyMs: 2,
      ok: true,
    })),
  };
}
const options = {
  mode: "qualification",
  warmupMs: 5000,
  minHz: 60,
  browserMode: "headed",
};
function fails(
  result: ReturnType<typeof evaluateApplicationPacing>,
  id: string,
) {
  assert.equal(
    result.checks.find((entry: { id: string }) => entry.id === id)?.ok,
    false,
    id,
  );
  assert.equal(result.ok, false);
  assert.equal(result.windowQualified, false);
}
function completionsFor(input: ReturnType<typeof fixture>, latency = 2) {
  const accepted = input.samples.filter((sample) => sample.submitted);
  input.completionRange = {
    firstFrame: accepted[0]?.frame ?? null,
    lastFrame: accepted.at(-1)?.frame ?? null,
  };
  input.productCounters.after.submittedFrames = accepted.length;
  input.productCounters.after.skippedFrames =
    input.samples.length - accepted.length;
  input.resources.frameCounters = structuredClone(input.productCounters.after);
  Object.assign(input.resources.renderer.gpuTimingStatus, {
    observedFrames: accepted.length,
    validFrames: accepted.length,
    lastFrame: accepted.at(-1)?.frame ?? null,
  });
  input.completedFrames = accepted.map((sample) => ({
    frame: sample.frame,
    submittedAt: sample.at + 0.1,
    submitReturnedAt: sample.at + 0.2,
    completedAt: sample.at + 0.1 + latency,
    latencyMs: latency,
    ok: true,
  }));
}

test("application windows retain exact rates, deliberate pacing, and unavailable display evidence", () => {
  const input = fixture(),
    before = structuredClone(input);
  const result = evaluateApplicationPacing(input, options);
  assert.equal(result.schema, "aethyrforge.application-pacing.v1");
  assert.equal(result.ok, true);
  assert.equal(result.windowQualified, true);
  assert.equal(result.gpuBudgetWindowQualified, true);
  assert.equal(result.qualified, false);
  assert.equal(result.summary.acceptedRateHz, 60);
  assert.equal(result.summary.completedRateHz, 60);
  assert.equal(result.summary.callbackCounts.rawCallbacks, 3600);
  assert.equal(result.summary.callbackCounts.intentionalPacedCallbacks, 1800);
  assert.equal(result.summary.rendererRejectedAttempts, 0);
  assert.equal(result.compositorPresentation, null);
  assert.equal(result.physicalPresentation, null);
  assert.deepEqual(input, before);
});

test("variable fence latency fails strict v1 while independently regular accepted opportunities pass", () => {
  const input = fixture();
  input.completedFrames.forEach((entry, index) => {
    entry.latencyMs = index % 2 ? 2 : 12;
    entry.completedAt = entry.submittedAt + entry.latencyMs;
  });
  const strict = evaluateFramePacing(input, options);
  const before = structuredClone(strict);
  assert.equal(strict.ok, false);
  assert.equal(
    strict.checks.find(
      (entry: { id: string }) => entry.id === "completion.interval-p95",
    ).ok,
    false,
  );
  const application = evaluateApplicationPacing(input, options);
  assert.equal(application.windowQualified, true);
  assert.ok(application.summary.fenceNotificationIntervalMs.p95 > 17.5);
  assert.ok(application.summary.acceptedOpportunityIntervalMs.p95 < 17.5);
  assert.deepEqual(strict, before);
});

test("batched fence callbacks and CPU enqueue jitter remain diagnostics with bounded latency", () => {
  const batched = fixture();
  batched.completedFrames.forEach((entry, index) => {
    if (index % 2 === 0)
      entry.completedAt = batched.completedFrames[index + 1].completedAt;
    entry.latencyMs = entry.completedAt - entry.submittedAt;
  });
  const result = evaluateApplicationPacing(batched, options);
  assert.equal(result.windowQualified, true);
  assert.equal(result.summary.equalFenceNotificationTimes, 900);
  assert.equal(evaluateFramePacing(batched, options).ok, false);
  const enqueued = fixture();
  enqueued.completedFrames.forEach((entry, index) => {
    entry.submittedAt += index % 2 ? 4 : 0;
    entry.submitReturnedAt = entry.submittedAt + 0.1;
    entry.completedAt = entry.submittedAt + 2;
  });
  const queuedResult = evaluateApplicationPacing(enqueued, options);
  assert.equal(queuedResult.windowQualified, true);
  assert.ok(queuedResult.summary.legacySubmissionStartIntervalMs.p95 > 20);
  assert.ok(queuedResult.summary.acceptedOpportunityIntervalMs.p95 < 17.5);
});

test("false callback accounting, capture suspensions, and inconsistent pacing targets fail", () => {
  for (const mutation of [
    (value: ReturnType<typeof fixture>) => {
      value.callbackCounts.rawCallbacks++;
    },
    (value: ReturnType<typeof fixture>) => {
      value.callbackCounts.renderAttempts--;
    },
    (value: ReturnType<typeof fixture>) => {
      value.callbackCounts.intentionalPacedCallbacks = -1;
    },
  ]) {
    const input = fixture();
    mutation(input);
    fails(evaluateApplicationPacing(input, options), "callbacks.accounted");
  }
  const suspended = fixture();
  suspended.callbackCounts.rawCallbacks++;
  suspended.callbackCounts.suspendedCallbacks++;
  fails(
    evaluateApplicationPacing(suspended, options),
    "callbacks.no-suspension",
  );
  const display = fixture();
  display.targetHz = "display";
  fails(
    evaluateApplicationPacing(display, options),
    "pacing.target-consistent",
  );
  const excess = fixture(3600, 120);
  excess.targetHz = 60;
  fails(evaluateApplicationPacing(excess, options), "pacing.target-consistent");
});

test("accepted cadence catches real busy skips without mislabeling intentional pacing", () => {
  const tooSlow = fixture();
  tooSlow.samples.forEach((sample, index) => {
    sample.submitted = index % 2 === 0;
  });
  completionsFor(tooSlow);
  const result = evaluateApplicationPacing(tooSlow, options);
  fails(result, "accepted.count");
  fails(result, "accepted.interval-p95");
  assert.equal(result.summary.rendererRejectedAttempts, 900);
  const display120 = fixture(3600, 120);
  display120.samples.forEach((sample, index) => {
    sample.submitted = index % 2 === 0;
  });
  completionsFor(display120);
  const sixtyAccepted = evaluateApplicationPacing(display120, options);
  assert.equal(sixtyAccepted.windowQualified, true);
  assert.equal(sixtyAccepted.summary.acceptedRateHz, 60);
  assert.equal(sixtyAccepted.summary.rendererRejectedAttempts, 1800);
  assert.equal(
    sixtyAccepted.summary.callbackCounts.intentionalPacedCallbacks,
    0,
  );
});

test("an accepted long gap fails even when a later burst preserves total frame count", () => {
  const input = fixture();
  for (let index = 900; index < 910; index++)
    input.samples[index].at += 50 - (index - 900) * 5;
  completionsFor(input);
  const result = evaluateApplicationPacing(input, options);
  assert.equal(
    result.checks.find((entry: { id: string }) => entry.id === "accepted.count")
      .ok,
    true,
  );
  fails(result, "accepted.interval-max");
  fails(result, "accepted.stalls");
});

test("missing, foreign, duplicate, failed, and wrong-range queue observations fail", () => {
  for (const mutation of [
    (value: ReturnType<typeof fixture>) => {
      value.completedFrames.pop();
    },
    (value: ReturnType<typeof fixture>) => {
      value.completedFrames[900].frame = 999999;
    },
    (value: ReturnType<typeof fixture>) => {
      value.completedFrames[900].frame = value.completedFrames[899].frame;
    },
  ]) {
    const input = fixture();
    mutation(input);
    fails(evaluateApplicationPacing(input, options), "completion.coverage");
  }
  const failed = fixture();
  failed.completedFrames[900].ok = false;
  fails(evaluateApplicationPacing(failed, options), "completion.success");
  const range = fixture();
  range.completionRange.lastFrame = range.completionRange.lastFrame! + 1;
  fails(evaluateApplicationPacing(range, options), "completion.range");
});

test("late queue completion and forged enqueue clocks fail despite a perfect opportunity schedule", () => {
  const late = fixture();
  completionsFor(late, 100);
  const result = evaluateApplicationPacing(late, options);
  fails(result, "completion.latency-p95");
  fails(result, "completion.latency-max");
  const detached = fixture();
  detached.completedFrames.forEach((entry) => {
    entry.submittedAt += 100000;
    entry.completedAt += 100000;
  });
  fails(evaluateApplicationPacing(detached, options), "completion.timeline");
  const inconsistent = fixture();
  inconsistent.completedFrames[900].latencyMs++;
  fails(evaluateApplicationPacing(inconsistent, options), "completion.valid");
});

test("GPU availability, budget, envelope scope, and simulation loss remain required", () => {
  const absent = fixture();
  absent.samples.forEach((sample) => {
    sample.fullGpuMs = null;
  });
  fails(evaluateApplicationPacing(absent, options), "gpu.availability");
  const unprofiled = evaluateApplicationPacing(absent, {
    ...options,
    requireFullGpu: false,
  });
  assert.equal(unprofiled.windowQualified, true);
  assert.equal(unprofiled.gpuBudgetWindowQualified, false);
  assert.equal(unprofiled.summary.fullGpuMs, null);
  const expensive = fixture();
  expensive.samples.forEach((sample) => {
    sample.fullGpuMs = 20;
  });
  fails(evaluateApplicationPacing(expensive, options), "gpu.budget-p95");
  const invalid = fixture();
  invalid.samples[900].fullGpuMs = 0;
  fails(evaluateApplicationPacing(invalid, options), "gpu.main-hdr-contained");
  for (const key of ["kernelDroppedSeconds", "hostClampedSeconds"] as const) {
    const input = fixture();
    input.samples.slice(900).forEach((sample) => {
      sample[key] = 0.01;
    });
    fails(evaluateApplicationPacing(input, options), `time.${key}`);
  }
});

test("CPU budgets are explicit diagnostics and do not get added to overlapping GPU scopes", () => {
  const input = fixture();
  input.samples.forEach((sample) => {
    sample.renderCpuMs = 20;
  });
  const result = evaluateApplicationPacing(input, options);
  assert.equal(result.windowQualified, true);
  assert.equal(result.summary.cpu.renderCpuMs.p95WithinTargetPeriod, false);
  assert.equal(result.summary.cpu.renderCpuMs.distribution.p95, 20);
  const malformed = fixture();
  malformed.samples[900].renderCpuMs = NaN;
  fails(
    evaluateApplicationPacing(malformed, options),
    "samples.cpu-values-valid",
  );
});

test("truncation, legacy scope, unbounded samples, and mismatched recording clocks cannot qualify", () => {
  for (const key of ["truncated", "completionTruncated"] as const) {
    const input = fixture();
    input[key] = true;
    fails(
      evaluateApplicationPacing(input, options),
      key === "truncated" ? "samples.complete" : "completion.complete",
    );
  }
  const legacy = fixture();
  legacy.sampleScope = "legacy-callback-samples";
  fails(evaluateApplicationPacing(legacy, options), "samples.scope");
  const huge = fixture(20001, 120);
  fails(evaluateApplicationPacing(huge, options), "samples.valid");
  const clock = fixture();
  clock.samples.forEach((sample) => {
    sample.at += 100000;
  });
  fails(evaluateApplicationPacing(clock, options), "samples.clock");
  fails(
    evaluateApplicationPacing(fixture(), { ...options, durationMs: 1000 }),
    "observation.duration-consistent",
  );
});

test("short windows remain diagnostic and headless never implies a physical or compositor claim", () => {
  const input = fixture(120);
  const diagnostic = evaluateApplicationPacing(input, {
    mode: "diagnostic",
    warmupMs: 500,
  });
  assert.equal(diagnostic.ok, true);
  assert.equal(diagnostic.windowQualified, false);
  fails(evaluateApplicationPacing(input, options), "qualification.duration");
  fails(
    evaluateApplicationPacing(fixture(), { ...options, warmupMs: 500 }),
    "qualification.warmup",
  );
  const headless = evaluateApplicationPacing(fixture(), {
    ...options,
    browserMode: "headless",
  });
  assert.equal(headless.windowQualified, true);
  assert.equal(headless.browserMode, "headless");
  assert.equal(headless.qualified, false);
  assert.equal(headless.compositorPresentation, null);
  assert.equal(headless.physicalPresentation, null);
  assert.throws(() => evaluateApplicationPacing(input, { minHz: Infinity }));
});

test("malformed supplied main-HDR values cannot support a GPU budget claim", () => {
  for (const value of [-1, NaN, "invalid"]) {
    const input = fixture();
    Object.assign(input.samples[900], { gpuMs: value });
    const result = evaluateApplicationPacing(input, options);
    fails(result, "gpu.main-hdr-values-valid");
    assert.equal(result.gpuBudgetWindowQualified, false);
    assert.equal(result.summary.malformedMainHdrSamples, 1);
    const optional = evaluateApplicationPacing(input, {
      ...options,
      requireFullGpu: false,
    });
    assert.equal(optional.windowQualified, true);
    assert.equal(optional.gpuBudgetWindowQualified, false);
  }
});

test("authoritative counters catch first-frame loss hidden from sampled deltas", () => {
  for (const key of ["kernelDroppedSeconds", "hostClampedSeconds"] as const) {
    const input = fixture();
    input.samples.forEach((sample) => {
      sample[key] = 0.01;
    });
    input.productCounters.after[key] = 0.01;
    input.resources.frameCounters[key] = 0.01;
    const result = evaluateApplicationPacing(input, options);
    assert.equal(result.summary.dropped[key], 0);
    assert.equal(result.summary.wholeWindowDropped[key], 0.01);
    fails(result, `time.whole-window.${key}`);
  }
  const absent = fixture();
  const missing = evaluateApplicationPacing(absent, {
    ...options,
    productCounters: undefined,
  });
  fails(missing, "counters.availability");
  const diagnostic = evaluateApplicationPacing(absent, {
    mode: "diagnostic",
    productCounters: undefined,
  });
  assert.equal(diagnostic.ok, true);
  assert.equal(diagnostic.windowQualified, false);
  assert.equal(diagnostic.summary.wholeWindowDropped, null);
  const tampered = fixture();
  tampered.productCounters.after.frameAttempts++;
  fails(evaluateApplicationPacing(tampered, options), "counters.valid");
  const unfrozen = fixture();
  unfrozen.resources.frameCounters.submittedFrames--;
  fails(evaluateApplicationPacing(unfrozen, options), "counters.frozen-end");
});

test("numeric HDR-only values cannot masquerade as render/compute GPU envelopes", () => {
  const wrong = fixture();
  wrong.resources.renderer.fullGpuScope = "main-HDR-pass-only";
  fails(evaluateApplicationPacing(wrong, options), "gpu.scope");
  for (const field of [
    "validFrames",
    "observedFrames",
    "pendingFrames",
    "unavailableFrames",
  ] as const) {
    const input = fixture();
    input.resources.renderer.gpuTimingStatus[field]++;
    fails(evaluateApplicationPacing(input, options), "gpu.status-consistent");
  }
  const range = fixture();
  range.resources.renderer.gpuTimingStatus.lastFrame =
    range.resources.renderer.gpuTimingStatus.lastFrame! + 1;
  fails(evaluateApplicationPacing(range, options), "gpu.status-consistent");
});

test("opportunity-to-enqueue deadlines reject regular rAF masking late CPU production", () => {
  const late = fixture();
  late.observation.stoppedAtMs += 1;
  late.completedFrames.forEach((entry) => {
    entry.submittedAt += 16.9;
    entry.submitReturnedAt += 16.9;
    entry.completedAt += 16.9;
  });
  const result = evaluateApplicationPacing(late, options);
  assert.equal(
    result.checks.find(
      (entry: { id: string }) => entry.id === "accepted.interval-p95",
    ).ok,
    true,
  );
  assert.equal(
    result.checks.find(
      (entry: { id: string }) => entry.id === "completion.timeline",
    ).ok,
    true,
  );
  fails(result, "enqueue.delay-p95");
  const last = fixture();
  last.observation.stoppedAtMs += 25;
  last.completedFrames.at(-1)!.submittedAt += 35;
  last.completedFrames.at(-1)!.submitReturnedAt += 35;
  last.completedFrames.at(-1)!.completedAt += 35;
  fails(evaluateApplicationPacing(last, options), "enqueue.delay-max");
});

test("browser mode comes from retained CDP evidence and remains unknown without it", () => {
  const evidence = {
    schema: "aethyrforge.browser-mode.v1",
    userAgent: "Chrome/155",
    commandLineAvailable: true,
    headlessArguments: [] as string[],
  };
  assert.equal(applicationBrowserMode(evidence), "headed");
  assert.equal(
    applicationBrowserMode({ ...evidence, commandLineAvailable: false }),
    "unknown",
  );
  assert.equal(
    applicationBrowserMode({
      ...evidence,
      commandLineAvailable: false,
      userAgent: "HeadlessChrome/155",
    }),
    "headless",
  );
  assert.equal(
    applicationBrowserMode({
      ...evidence,
      headlessArguments: ["--headless=new"],
    }),
    "headless",
  );
  assert.equal(applicationBrowserMode({ launcherHeaded: true }), "unknown");
});

test("missing or invalid post-submit evidence cannot qualify and never falls back to legacy timestamps", () => {
  const input = fixture();
  input.completedFrames.forEach((entry) => {
    delete (entry as any).submitReturnedAt;
  });
  fails(
    evaluateApplicationPacing(input, options),
    "enqueue.return-availability",
  );
  const diagnostic = evaluateApplicationPacing(input, { mode: "diagnostic" });
  assert.equal(diagnostic.ok, true);
  assert.equal(diagnostic.windowQualified, false);
  assert.equal(diagnostic.summary.postSubmitReturnEvidenceAvailable, false);
  assert.equal(diagnostic.summary.opportunityToEnqueueDelayMs, null);
  const invalid = fixture();
  invalid.completedFrames[900].submitReturnedAt =
    invalid.completedFrames[900].submittedAt - 1;
  fails(
    evaluateApplicationPacing(invalid, options),
    "enqueue.return-availability",
  );
});

test("browser observation retains only mode evidence, cleans up, and labels unavailable CDP unknown", async () => {
  let detached = 0;
  const evidence = await observeApplicationBrowserMode({
    newBrowserCDPSession: async () => ({
      send: async (method: string) =>
        method === "Browser.getVersion"
          ? { product: "Chrome/155", userAgent: "Chrome/155" }
          : {
              arguments: [
                "--enable-automation",
                "--user-data-dir=/private/profile",
                "--headless=new",
              ],
            },
      detach: async () => {
        detached++;
      },
    }),
  });
  assert.equal(evidence.mode, "headless");
  assert.equal(detached, 1);
  assert.deepEqual(evidence.headlessArguments, ["--headless=new"]);
  assert(!JSON.stringify(evidence).includes("/private/profile"));
  const unknown = await observeApplicationBrowserMode({
    newBrowserCDPSession: async () => {
      throw Error("CDP unavailable");
    },
  });
  assert.equal(unknown.mode, "unknown");
  assert.equal(unknown.commandLineAvailable, false);
  assert.equal(unknown.errors.length, 1);
});

test("legacy application v1 results retain their original serialized verdict", () => {
  for (const settings of [{ mode: "diagnostic" }, options]) {
    const result = evaluateApplicationPacing(fixture(), settings);
    assert.equal(
      awaitHash(result),
      settings.mode === "diagnostic"
        ? "8a10f65c1eb23b7cbeda0ab6b0543e66e88ee5053f372649b0d10b26834193be"
        : "a9ceb4106814737ac53694cc294fa9414669a3158091c7100c7f48e3b85f29fb",
    );
  }
});
function awaitHash(value: unknown) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

function fixtureV2() {
  const input: any = fixture();
  input.callbackClock = "entry";
  for (const sample of input.samples)
    sample.callbackEnteredAt = sample.at + 0.05;
  return input;
}
test("v2 binds CPU scheduling to explicit callback entries while retaining every opportunity and GPU requirement", () => {
  const input = fixtureV2(),
    before = structuredClone(input),
    result = evaluateApplicationPacing(input, options);
  assert.equal(result.schema, "aethyrforge.application-pacing.v2");
  assert.equal(result.windowQualified, true);
  assert.equal(result.gpuBudgetWindowQualified, true);
  assert.equal(result.summary.callbackEntryEvidenceAvailable, true);
  assert(result.summary.callbackEntryToEnqueueDelayMs.p95 < 0.151);
  assert.equal(result.summary.acceptedRateHz, 60);
  assert.equal(result.physicalPresentation, null);
  assert.equal(result.compositorPresentation, null);
  assert.deepEqual(input, before);
});
test("a later scheduled rAF timestamp is not an actual callback-entry ordering boundary", () => {
  const input = fixtureV2(),
    index = 900,
    previous = input.samples[index - 1];
  input.samples[index].at = previous.at + 8.3;
  input.samples[index].callbackEnteredAt = previous.at + 10.1;
  Object.assign(input.completedFrames[index - 1], {
    submittedAt: previous.at + 9,
    submitReturnedAt: previous.at + 10,
    completedAt: previous.at + 12,
    latencyMs: 3,
  });
  Object.assign(input.completedFrames[index], {
    submittedAt: previous.at + 10.2,
    submitReturnedAt: previous.at + 10.3,
    completedAt: previous.at + 12.2,
    latencyMs: 2,
  });
  const current = evaluateApplicationPacing(input, options);
  assert.equal(current.windowQualified, true);
  const legacy = structuredClone(input);
  delete legacy.callbackClock;
  for (const sample of legacy.samples) delete sample.callbackEnteredAt;
  const historical = evaluateApplicationPacing(legacy, options);
  assert.equal(historical.schema, "aethyrforge.application-pacing.v1");
  fails(historical, "completion.timeline");
  assert.equal(
    current.checks.find((c: any) => c.id === "completion.timeline").ok,
    true,
  );
});
test("v2 marker forbids missing or partial entry evidence and unknown clock scopes", () => {
  for (const mutation of [
    (input: any) => {
      for (const sample of input.samples) delete sample.callbackEnteredAt;
    },
    (input: any) => {
      delete input.samples[800].callbackEnteredAt;
    },
    (input: any) => {
      delete input.callbackClock;
      delete input.samples[800].callbackEnteredAt;
    },
  ]) {
    const input = fixtureV2();
    mutation(input);
    const result = evaluateApplicationPacing(input, options);
    assert.equal(result.schema, "aethyrforge.application-pacing.v2");
    fails(result, "callbacks.entry-valid");
  }
  const invalid = fixtureV2();
  invalid.callbackClock = "scheduled";
  fails(evaluateApplicationPacing(invalid, options), "callbacks.entry-scope");
});
test("forged entry clocks and actual CPU overlap cannot pass v2 ordering", () => {
  for (const value of [-1, NaN, Infinity, 9999, 40001]) {
    const input = fixtureV2();
    input.samples[800].callbackEnteredAt = value;
    fails(evaluateApplicationPacing(input, options), "callbacks.entry-valid");
  }
  const reversed = fixtureV2();
  reversed.samples[800].callbackEnteredAt =
    reversed.samples[799].callbackEnteredAt - 1;
  fails(evaluateApplicationPacing(reversed, options), "callbacks.entry-valid");
  const afterSubmit = fixtureV2();
  afterSubmit.samples[800].callbackEnteredAt =
    afterSubmit.completedFrames[800].submittedAt + 0.05;
  fails(evaluateApplicationPacing(afterSubmit, options), "completion.timeline");
  const overlap = fixtureV2();
  overlap.samples[801].callbackEnteredAt =
    overlap.completedFrames[800].submitReturnedAt - 0.05;
  fails(evaluateApplicationPacing(overlap, options), "enqueue.return-timeline");
});
test("v2 has its own CPU-entry deadlines and keeps independent scheduled-opportunity deadlines", () => {
  const late = fixtureV2();
  late.observation.startedAtMs -= 20;
  for (const sample of late.samples) sample.callbackEnteredAt = sample.at - 20;
  const result = evaluateApplicationPacing(late, options);
  fails(result, "enqueue.entry-delay-p95");
  assert.equal(
    result.checks.find((c: any) => c.id === "enqueue.delay-p95").ok,
    true,
  );
  const last = fixtureV2();
  last.observation.stoppedAtMs += 40;
  Object.assign(last.completedFrames.at(-1), {
    submitReturnedAt: last.completedFrames.at(-1).submittedAt + 35,
    completedAt: last.completedFrames.at(-1).submittedAt + 37,
    latencyMs: 37,
  });
  fails(evaluateApplicationPacing(last, options), "enqueue.entry-delay-max");
});
test("v2 does not bypass missing or failed fences, boundary loss, GPU scope or callback accounting", () => {
  for (const [id, change] of [
    ["completion.coverage", (input: any) => input.completedFrames.pop()],
    [
      "completion.success",
      (input: any) => {
        input.completedFrames[20].ok = false;
      },
    ],
    [
      "counters.samples-match",
      (input: any) => {
        input.callbackCounts.rawCallbacks++;
      },
    ],
    [
      "gpu.scope",
      (input: any) => {
        input.resources.renderer.fullGpuScope = "main-HDR-only";
      },
    ],
    [
      "gpu.availability",
      (input: any) => {
        input.samples[20].fullGpuMs = null;
      },
    ],
    [
      "enqueue.return-availability",
      (input: any) => {
        delete input.completedFrames[20].submitReturnedAt;
      },
    ],
    [
      "time.whole-window.kernelDroppedSeconds",
      (input: any) => {
        input.samples.forEach((s: any) => {
          s.kernelDroppedSeconds = 0.01;
        });
        input.productCounters.after.kernelDroppedSeconds = 0.01;
        input.resources.frameCounters.kernelDroppedSeconds = 0.01;
      },
    ],
  ] as const) {
    const input = fixtureV2();
    change(input);
    fails(evaluateApplicationPacing(input, options), id);
  }
});
