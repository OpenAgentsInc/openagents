import { distribution } from "../experiments/c-town/measurement.mjs";

const finite = (value) => typeof value === "number" && Number.isFinite(value);
const nonnegative = (value) => finite(value) && value >= 0;
const frameId = (value) => Number.isSafeInteger(value) && value >= 0;
const object = (value) => value !== null && typeof value === "object";

/** Evaluate queue fences separately from GPU spans and unmeasured presentation. */
export function evaluateFramePacing(result, options = {}) {
  const mode = options.mode ?? "diagnostic";
  const minHz = options.minHz ?? 60;
  const warmupMs = options.warmupMs ?? 0;
  const requireFullGpu = options.requireFullGpu ?? mode === "qualification";
  if (
    !["diagnostic", "qualification"].includes(mode) ||
    !finite(minHz) ||
    minHz < 1 ||
    minHz > 240 ||
    !nonnegative(warmupMs) ||
    typeof requireFullGpu !== "boolean"
  )
    throw new Error("Choose a supported pacing mode, frame rate, and warmup.");

  const samples = Array.isArray(result?.samples) ? result.samples : [];
  const completions = Array.isArray(result?.completedFrames)
    ? result.completedFrames
    : [];
  const validSamples = samples.every(
    (sample, index) =>
      object(sample) &&
      frameId(sample.frame) &&
      nonnegative(sample.at) &&
      typeof sample.submitted === "boolean" &&
      (!index || sample.at >= samples[index - 1].at),
  );
  const observedMs =
    validSamples && samples.length > 1 ? samples.at(-1).at - samples[0].at : 0;
  const durationMs = options.durationMs ?? observedMs;
  if (!finite(durationMs) || durationMs <= 0)
    throw new Error("Choose a positive finite observation duration.");

  const factor = 60 / minHz;
  const limits = {
    minCompleted: Math.max(1, Math.floor((durationMs * minHz) / 1000) - 1),
    p95IntervalMs: 17.5 * factor,
    p99IntervalMs: 20 * factor,
    maxIntervalMs: 33.34 * factor,
    slowIntervalMs: 25 * factor,
    maxSlowFraction: 0.001,
    stallMs: 50 * factor,
    p95LatencyMs: 33.34 * factor,
    maxLatencyMs: 50 * factor,
    gpuP95Ms: 1000 / minHz,
  };
  const checks = [];
  const check = (id, ok, actual, limit, required = true) => {
    checks.push({ id, ok: Boolean(ok), actual, limit, required });
  };
  check(
    "samples.valid",
    Array.isArray(result?.samples) && validSamples && samples.length > 1,
    samples.length,
    "At least two finite, ordered callback samples",
  );
  check(
    "samples.complete",
    result?.truncated === false,
    result?.truncated ?? null,
    false,
  );
  check(
    "samples.stopped",
    result?.recording === false,
    result?.recording ?? null,
    false,
  );
  check(
    "samples.coverage",
    observedMs >= durationMs - limits.maxIntervalMs,
    observedMs,
    { minimumMs: Math.max(0, durationMs - limits.maxIntervalMs) },
  );

  const submitted = validSamples
    ? samples.filter((sample) => sample.submitted)
    : [];
  const submittedIds = new Set(submitted.map((sample) => sample.frame));
  check(
    "submissions.unique",
    submittedIds.size === submitted.length &&
      submitted.every(
        (sample, index) => !index || sample.frame > submitted[index - 1].frame,
      ),
    { count: submitted.length, unique: submittedIds.size },
    "Unique increasing submitted frame IDs",
  );

  const range = result?.completionRange;
  const firstFrame = submitted[0]?.frame ?? null;
  const lastFrame = submitted.at(-1)?.frame ?? null;
  check(
    "completion.range",
    object(range) &&
      range.firstFrame === firstFrame &&
      range.lastFrame === lastFrame,
    range ?? null,
    { firstFrame, lastFrame },
  );
  check(
    "completion.complete",
    result?.completionTruncated === false,
    result?.completionTruncated ?? null,
    false,
  );
  const validCompletions = completions.every(
    (entry, index) =>
      object(entry) &&
      frameId(entry.frame) &&
      nonnegative(entry.submittedAt) &&
      nonnegative(entry.completedAt) &&
      entry.completedAt >= entry.submittedAt &&
      nonnegative(entry.latencyMs) &&
      Math.abs(entry.latencyMs - (entry.completedAt - entry.submittedAt)) <=
        0.001 &&
      (!index ||
        (entry.completedAt >= completions[index - 1].completedAt &&
          entry.submittedAt >= completions[index - 1].submittedAt)),
  );
  check(
    "completion.valid",
    Array.isArray(result?.completedFrames) && validCompletions,
    completions.length,
    "Finite ordered fence times with consistent latency",
  );
  const completionIds = new Set(completions.map((entry) => entry?.frame));
  check(
    "completion.unique",
    completionIds.size === completions.length,
    { count: completions.length, unique: completionIds.size },
    "One completion per frame",
  );
  const missing = [...submittedIds].filter((id) => !completionIds.has(id));
  const extra = [...completionIds].filter((id) => !submittedIds.has(id));
  check(
    "completion.coverage",
    missing.length === 0 &&
      extra.length === 0 &&
      completions.length === submitted.length,
    {
      missing,
      extra,
      submitted: submitted.length,
      completed: completions.length,
    },
    "Exact submitted-frame coverage",
  );
  const failed = completions.filter((entry) => entry?.ok !== true).length;
  check("completion.success", failed === 0, failed, 0);
  check(
    "completion.count",
    completions.length >= limits.minCompleted,
    completions.length,
    { minimum: limits.minCompleted, boundaryAllowance: 1 },
  );
  const observation = result?.observation;
  const validObservation =
    object(observation) &&
    nonnegative(observation.startedAtMs) &&
    nonnegative(observation.stoppedAtMs) &&
    observation.stoppedAtMs > observation.startedAtMs;
  const samplesByFrame = new Map(
    samples.flatMap((sample, index) =>
      sample?.submitted
        ? [[sample.frame, { sample, next: samples[index + 1] }]]
        : [],
    ),
  );
  const mismatchedTimeline = validCompletions
    ? completions.filter((entry) => {
        const point = samplesByFrame.get(entry.frame);
        return (
          !validObservation ||
          !point ||
          entry.submittedAt < observation.startedAtMs ||
          entry.submittedAt > observation.stoppedAtMs ||
          entry.submittedAt < point.sample.at - 0.5 ||
          entry.submittedAt >
            (point.next?.at ?? point.sample.at + limits.maxLatencyMs) + 0.5
        );
      }).length
    : completions.length;
  check(
    "completion.timeline",
    validObservation && mismatchedTimeline === 0,
    {
      observation: validObservation ? observation : null,
      mismatched: mismatchedTimeline,
    },
    "Submission times belong to the real recorder window and matching callback interval; final callback allows 50ms at 60Hz",
  );

  const intervals = validCompletions
    ? completions
        .slice(1)
        .map(
          (entry, index) => entry.completedAt - completions[index].completedAt,
        )
    : [];
  const latency = validCompletions
    ? distribution(completions.map((entry) => entry.latencyMs))
    : null;
  const cadence = distribution(intervals);
  const equalTimes = intervals.filter((value) => value === 0).length;
  const slowIntervals = intervals.filter(
    (value) => value > limits.slowIntervalMs,
  ).length;
  const slowFraction = intervals.length
    ? slowIntervals / intervals.length
    : null;
  const stalls = intervals.filter((value) => value > limits.stallMs).length;
  check(
    "completion.cadence-evidence",
    Boolean(cadence) && equalTimes === 0,
    { intervals: intervals.length, equalTimes },
    "Strictly increasing host fence notifications",
  );
  check(
    "completion.interval-p95",
    cadence && cadence.p95 <= limits.p95IntervalMs,
    cadence?.p95 ?? null,
    limits.p95IntervalMs,
  );
  check(
    "completion.interval-p99",
    cadence && cadence.p99 <= limits.p99IntervalMs,
    cadence?.p99 ?? null,
    limits.p99IntervalMs,
  );
  check(
    "completion.interval-max",
    cadence && cadence.max <= limits.maxIntervalMs,
    cadence?.max ?? null,
    limits.maxIntervalMs,
  );
  check(
    "completion.slow-fraction",
    slowFraction !== null && slowFraction <= limits.maxSlowFraction,
    slowFraction,
    {
      intervalAboveMs: limits.slowIntervalMs,
      maximumFraction: limits.maxSlowFraction,
    },
  );
  check("completion.stalls", Boolean(cadence) && stalls === 0, stalls, {
    aboveMs: limits.stallMs,
    maximumCount: 0,
  });
  check(
    "completion.latency-p95",
    latency && latency.p95 <= limits.p95LatencyMs,
    latency?.p95 ?? null,
    limits.p95LatencyMs,
  );
  check(
    "completion.latency-p99",
    latency && latency.p99 <= limits.maxLatencyMs,
    latency?.p99 ?? null,
    limits.maxLatencyMs,
  );
  check(
    "completion.latency-max",
    latency && latency.max <= limits.maxLatencyMs,
    latency?.max ?? null,
    limits.maxLatencyMs,
  );

  const dropped = {};
  for (const key of ["kernelDroppedSeconds", "hostClampedSeconds"]) {
    const values = samples.map((sample) => sample?.[key]);
    const valid =
      values.length > 1 &&
      values.every(
        (value, index) =>
          nonnegative(value) && (!index || value >= values[index - 1]),
      );
    dropped[key] = valid ? values.at(-1) - values[0] : null;
    check(`time.${key}`, valid && dropped[key] === 0, dropped[key], 0);
  }

  check(
    "qualification.duration",
    durationMs >= 30_000,
    durationMs,
    30_000,
    mode === "qualification",
  );
  check(
    "qualification.warmup",
    warmupMs >= 5_000,
    warmupMs,
    5_000,
    mode === "qualification",
  );
  const queueChecks = checks.filter((entry) => entry.required);
  const queuePassed = queueChecks.every((entry) => entry.ok);

  const gpuValues = submitted.map((sample) => sample.fullGpuMs ?? null);
  const malformedGpu = gpuValues.filter(
    (value) => value !== null && !nonnegative(value),
  ).length;
  const gpu = malformedGpu ? null : distribution(gpuValues);
  const gpuComplete =
    submitted.length > 0 && !malformedGpu && gpu?.count === submitted.length;
  const gpuScopeViolations = submitted.filter(
    (sample) =>
      nonnegative(sample.fullGpuMs) &&
      nonnegative(sample.gpuMs) &&
      sample.fullGpuMs + 0.05 < sample.gpuMs,
  ).length;
  check(
    "gpu.availability",
    gpuComplete,
    {
      measured: gpu?.count ?? 0,
      submitted: submitted.length,
      malformed: malformedGpu,
    },
    "One all-pass GPU timestamp span per submitted frame",
    requireFullGpu,
  );
  check(
    "gpu.budget-p95",
    gpuComplete && gpu.p95 <= limits.gpuP95Ms,
    gpu?.p95 ?? null,
    limits.gpuP95Ms,
    requireFullGpu,
  );
  check(
    "gpu.main-hdr-contained",
    gpuScopeViolations === 0,
    gpuScopeViolations,
    { maximumViolations: 0, toleranceMs: 0.05 },
    requireFullGpu,
  );
  const gpuPassed =
    gpuComplete && gpuScopeViolations === 0 && gpu.p95 <= limits.gpuP95Ms;
  const ok = checks.every((entry) => !entry.required || entry.ok);
  const queueQualified = mode === "qualification" && queuePassed;
  const gpuBudgetQualified = queueQualified && Boolean(gpuPassed);
  const completionSpanMs =
    validCompletions && completions.length > 1
      ? completions.at(-1).completedAt - completions[0].completedAt
      : 0;
  return {
    ok,
    qualified: mode === "qualification" && ok,
    queueQualified,
    gpuBudgetQualified,
    checks,
    summary: {
      mode,
      minHz,
      durationMs,
      warmupMs,
      requireFullGpu,
      observationMs: validObservation
        ? observation.stoppedAtMs - observation.startedAtMs
        : null,
      callbacks: samples.length,
      callbackScope: result?.sampleScope ?? "legacy-callback-samples",
      renderAttempts: samples.length,
      rawCallbacks: result?.callbackCounts?.rawCallbacks ?? null,
      intentionalPacedCallbacks:
        result?.callbackCounts?.intentionalPacedCallbacks ?? null,
      targetHz: result?.targetHz ?? "display",
      submitted: submitted.length,
      completed: completions.length,
      completionRateHz: (completions.length * 1000) / durationMs,
      fenceSpanRateHz:
        completionSpanMs > 0
          ? ((completions.length - 1) * 1000) / completionSpanMs
          : null,
      completionIntervalMs: cadence,
      completionLatencyMs: latency,
      equalCompletionTimes: equalTimes,
      slowIntervals,
      slowIntervalFraction: slowFraction,
      stalls,
      fullGpuMs: gpu,
      missingFullGpuSamples: submitted.length - (gpu?.count ?? 0),
      fullGpuScopeViolations: gpuScopeViolations,
      mainHdrGpuMs: validSamples
        ? distribution(
            submitted.map((sample) =>
              nonnegative(sample.gpuMs) ? sample.gpuMs : null,
            ),
          )
        : null,
      dropped,
      presentationMs: null,
    },
    limitations: [
      "Queue fences report when the host observes submitted GPU work complete; they do not measure compositor presentation or physical displayed FPS.",
      "Equal fence timestamps can result from batched host notifications. They make smooth cadence evidence insufficient, without proving the display was unsmooth.",
      "Completion latency includes queueing and host notification delay. Its limits conservatively bound outstanding work, not isolated GPU execution.",
      "Qualification requires two independent repeats in the caller; this evaluator checks one observation window only.",
      "Dropped time uses the last minus first cumulative callback counters; time before the observed window is excluded.",
      ...(gpuComplete
        ? []
        : [
            "All-pass GPU timing is incomplete or unavailable. No all-pass GPU-budget claim is supported.",
          ]),
      ...(gpuScopeViolations
        ? [
            "All-pass GPU spans were shorter than their enclosed main-HDR spans. Those timestamps do not support a GPU-budget claim.",
          ]
        : []),
      ...(mode === "diagnostic"
        ? [
            "A diagnostic observation never qualifies the 30-second sustained target.",
          ]
        : []),
    ],
  };
}
