import { distribution } from "../experiments/c-town/measurement.mjs";

const finite = (value) => typeof value === "number" && Number.isFinite(value);
const nonnegative = (value) => finite(value) && value >= 0;
const integer = (value) => Number.isSafeInteger(value) && value >= 0;
const object = (value) => value !== null && typeof value === "object";
const MAX_SAMPLES = 20000;
const CPU_STAGES = [
  "stepMs",
  "renderCpuMs",
  "commandEncodeMs",
  "cullMs",
  "clusterMs",
  "bridgeEncodeMs",
  "wasmRequestMs",
  "bridgeDecodeMs",
  "bridgeOverheadMs",
];

/** Application evidence only. GPU completion notifications are not display events. */
export function evaluateApplicationPacing(result, options = {}) {
  const mode = options.mode ?? "diagnostic";
  const minHz = options.minHz ?? 60;
  const warmupMs = options.warmupMs ?? 0;
  const requireFullGpu = options.requireFullGpu ?? mode === "qualification";
  const browserMode = options.browserMode ?? "unknown";
  if (
    !["diagnostic", "qualification"].includes(mode) ||
    !finite(minHz) ||
    minHz < 1 ||
    minHz > 240 ||
    !nonnegative(warmupMs) ||
    typeof requireFullGpu !== "boolean" ||
    !["headed", "headless", "unknown"].includes(browserMode) ||
    (options.durationMs !== undefined &&
      (!finite(options.durationMs) || options.durationMs <= 0))
  )
    throw new Error(
      "Choose a supported application pacing mode, browser mode, frame rate, and warmup.",
    );

  const factor = 60 / minHz,
    budget = 1000 / minHz;
  const limits = {
    p95IntervalMs: 17.5 * factor,
    p99IntervalMs: 20 * factor,
    maxIntervalMs: 33.34 * factor,
    slowIntervalMs: 25 * factor,
    maxSlowFraction: 0.001,
    stallMs: 50 * factor,
    p95LatencyMs: 33.34 * factor,
    maxLatencyMs: 50 * factor,
    gpuP95Ms: budget,
  };
  const checks = [];
  const check = (id, ok, actual, expected, required = true) =>
    checks.push({ id, ok: Boolean(ok), actual, expected, required });
  const observation = result?.observation;
  const validObservation =
    object(observation) &&
    nonnegative(observation.startedAtMs) &&
    nonnegative(observation.stoppedAtMs) &&
    observation.stoppedAtMs > observation.startedAtMs;
  const durationMs = validObservation
    ? observation.stoppedAtMs - observation.startedAtMs
    : null;
  check(
    "observation.valid",
    validObservation,
    validObservation ? observation : null,
    "Finite positive recorder boundaries in the worker performance clock",
  );
  check(
    "observation.duration-consistent",
    options.durationMs === undefined ||
      (validObservation && Math.abs(durationMs - options.durationMs) <= 0.001),
    { observedMs: durationMs, requestedMs: options.durationMs ?? null },
    "An explicit caller duration matches the actual recorder span within 0.001 ms",
  );

  const samples =
    Array.isArray(result?.samples) && result.samples.length <= MAX_SAMPLES
      ? result.samples
      : [];
  const hasEntryMarker =
    object(result) && Object.hasOwn(result, "callbackClock");
  const entryClock =
    hasEntryMarker ||
    samples.some(
      (sample) => object(sample) && Object.hasOwn(sample, "callbackEnteredAt"),
    );
  const validSamples =
    samples.length >= 2 &&
    samples.every(
      (sample, index) =>
        object(sample) &&
        integer(sample.frame) &&
        nonnegative(sample.at) &&
        typeof sample.submitted === "boolean" &&
        (!index || sample.at >= samples[index - 1].at),
    );
  check(
    "samples.valid",
    validSamples &&
      Array.isArray(result?.samples) &&
      result.samples.length <= MAX_SAMPLES,
    samples.length,
    "Two to 20,000 finite ordered render-attempt samples",
  );
  check(
    "samples.scope",
    result?.sampleScope === "render-attempts",
    result?.sampleScope ?? null,
    "render-attempts",
  );
  check(
    "samples.stopped",
    result?.recording === false,
    result?.recording ?? null,
    false,
  );
  check(
    "samples.complete",
    result?.truncated === false,
    result?.truncated ?? null,
    false,
  );
  const callbackCounts = result?.callbackCounts;
  const validCounts =
    object(callbackCounts) &&
    [
      "rawCallbacks",
      "intentionalPacedCallbacks",
      "suspendedCallbacks",
      "renderAttempts",
    ].every(
      (key) => integer(callbackCounts[key]) && callbackCounts[key] <= 200000,
    );
  const accounted =
    validCounts &&
    callbackCounts.rawCallbacks ===
      callbackCounts.intentionalPacedCallbacks +
        callbackCounts.suspendedCallbacks +
        callbackCounts.renderAttempts &&
    callbackCounts.renderAttempts === samples.length;
  check(
    "callbacks.accounted",
    accounted,
    validCounts ? callbackCounts : null,
    "Raw callbacks equal intentional pacing plus capture suspensions plus the exact sampled attempts",
  );
  check(
    "callbacks.no-suspension",
    validCounts && callbackCounts.suspendedCallbacks === 0,
    validCounts ? callbackCounts.suspendedCallbacks : null,
    0,
  );
  const targetHz = result?.targetHz;
  check(
    "pacing.target-valid",
    targetHz === "display" || targetHz === 60,
    targetHz ?? null,
    "display or 60",
  );
  check(
    "pacing.target-consistent",
    validCounts &&
      (targetHz === 60
        ? validObservation &&
          minHz <= 60 &&
          callbackCounts.renderAttempts <=
            Math.ceil((durationMs * 60) / 1000) + 1
        : targetHz === "display" &&
          callbackCounts.intentionalPacedCallbacks === 0),
    {
      targetHz: targetHz ?? null,
      minHz,
      attempts: validCounts ? callbackCounts.renderAttempts : null,
      paced: validCounts ? callbackCounts.intentionalPacedCallbacks : null,
    },
    "Display mode has no intentional pacing; a 60 Hz profile does not claim a higher schedule",
  );

  const sampleClockValid =
    validSamples &&
    validObservation &&
    samples.every(
      (sample, index) =>
        sample.at >= observation.startedAtMs - budget &&
        sample.at <= observation.stoppedAtMs &&
        (index === 0 || sample.at >= observation.startedAtMs),
    );
  check(
    "samples.clock",
    sampleClockValid,
    validSamples ? { firstAt: samples[0].at, lastAt: samples.at(-1).at } : null,
    "Only the first rAF opportunity may precede recorder start, by at most one target period; all submissions are separately bound to the recorder",
  );
  const validEntries =
    entryClock &&
    validSamples &&
    validObservation &&
    samples.every(
      (sample, index) =>
        nonnegative(sample.callbackEnteredAt) &&
        sample.callbackEnteredAt >= observation.startedAtMs &&
        sample.callbackEnteredAt <= observation.stoppedAtMs &&
        (!index ||
          sample.callbackEnteredAt >= samples[index - 1].callbackEnteredAt),
    );
  if (entryClock) {
    check(
      "callbacks.entry-scope",
      !hasEntryMarker || result.callbackClock === "entry",
      result?.callbackClock ?? null,
      "entry, or an unmarked complete set of explicit entry observations",
    );
    check(
      "callbacks.entry-valid",
      validEntries,
      {
        observed: samples.filter((sample) =>
          nonnegative(sample?.callbackEnteredAt),
        ).length,
        attempts: samples.length,
      },
      "A finite ordered performance.now callback entry inside the recorder for every render attempt",
    );
  }
  const accepted = validSamples
    ? samples.filter((sample) => sample.submitted)
    : [];
  const acceptedIds = new Set(accepted.map((sample) => sample.frame));
  check(
    "accepted.unique",
    acceptedIds.size === accepted.length &&
      accepted.every(
        (sample, index) => !index || sample.frame > accepted[index - 1].frame,
      ),
    { accepted: accepted.length, unique: acceptedIds.size },
    "Unique increasing accepted frame IDs",
  );
  const minimumCount = validObservation
    ? Math.max(1, Math.floor((durationMs * minHz) / 1000) - 1)
    : null;
  check(
    "accepted.count",
    minimumCount !== null && accepted.length >= minimumCount,
    accepted.length,
    { minimum: minimumCount, boundaryAllowance: 1 },
  );
  const acceptedSpan =
    accepted.length > 1 ? accepted.at(-1).at - accepted[0].at : 0;
  check(
    "accepted.coverage",
    validObservation &&
      accepted.length > 1 &&
      accepted[0].at - observation.startedAtMs <= limits.maxIntervalMs &&
      observation.stoppedAtMs - accepted.at(-1).at <= limits.maxIntervalMs &&
      acceptedSpan >= durationMs - limits.maxIntervalMs,
    {
      spanMs: acceptedSpan,
      firstAt: accepted[0]?.at ?? null,
      lastAt: accepted.at(-1)?.at ?? null,
    },
    "Accepted opportunities span the real recorder window within the explicit boundary allowance",
  );
  const opportunityIntervals = validSamples
    ? accepted.slice(1).map((sample, index) => sample.at - accepted[index].at)
    : [];
  const opportunityCadence = distribution(opportunityIntervals);
  const equalOpportunities = opportunityIntervals.filter(
    (value) => value === 0,
  ).length;
  const slowIntervals = opportunityIntervals.filter(
    (value) => value > limits.slowIntervalMs,
  ).length;
  const slowFraction = opportunityIntervals.length
    ? slowIntervals / opportunityIntervals.length
    : null;
  const stalls = opportunityIntervals.filter(
    (value) => value > limits.stallMs,
  ).length;
  check(
    "accepted.cadence-evidence",
    opportunityCadence && equalOpportunities === 0,
    { intervals: opportunityIntervals.length, equalTimes: equalOpportunities },
    "Distinct ordered accepted rAF opportunities",
  );
  for (const [quantile, limit] of [
    ["p95", limits.p95IntervalMs],
    ["p99", limits.p99IntervalMs],
    ["max", limits.maxIntervalMs],
  ])
    check(
      `accepted.interval-${quantile}`,
      opportunityCadence && opportunityCadence[quantile] <= limit,
      opportunityCadence?.[quantile] ?? null,
      limit,
    );
  check(
    "accepted.slow-fraction",
    slowFraction !== null && slowFraction <= limits.maxSlowFraction,
    slowFraction,
    {
      intervalAboveMs: limits.slowIntervalMs,
      maximumFraction: limits.maxSlowFraction,
    },
  );
  check("accepted.stalls", opportunityCadence && stalls === 0, stalls, {
    aboveMs: limits.stallMs,
    maximumCount: 0,
  });

  const completions =
    Array.isArray(result?.completedFrames) &&
    result.completedFrames.length <= MAX_SAMPLES
      ? result.completedFrames
      : [];
  const validCompletions = completions.every(
    (entry, index) =>
      object(entry) &&
      integer(entry.frame) &&
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
    Array.isArray(result?.completedFrames) &&
      result.completedFrames.length <= MAX_SAMPLES &&
      validCompletions,
    completions.length,
    "Finite ordered queue observations with consistent latency",
  );
  check(
    "completion.complete",
    result?.completionTruncated === false,
    result?.completionTruncated ?? null,
    false,
  );
  const completionIds = new Set(completions.map((entry) => entry?.frame));
  const missing = [...acceptedIds].filter((id) => !completionIds.has(id));
  const foreign = [...completionIds].filter((id) => !acceptedIds.has(id));
  check(
    "completion.coverage",
    completionIds.size === completions.length &&
      completions.length === accepted.length &&
      missing.length === 0 &&
      foreign.length === 0,
    {
      accepted: accepted.length,
      completed: completions.length,
      missing,
      foreign,
      unique: completionIds.size,
    },
    "Exactly one completion for every accepted frame, with no missing or foreign frames",
  );
  const firstFrame = accepted[0]?.frame ?? null,
    lastFrame = accepted.at(-1)?.frame ?? null;
  check(
    "completion.range",
    object(result?.completionRange) &&
      result.completionRange.firstFrame === firstFrame &&
      result.completionRange.lastFrame === lastFrame,
    result?.completionRange ?? null,
    { firstFrame, lastFrame },
  );
  const failed = completions.filter((entry) => entry?.ok !== true).length;
  check("completion.success", failed === 0, failed, 0);
  const samplesByFrame = new Map(
    validSamples
      ? samples.flatMap((sample, index) =>
          sample.submitted
            ? [[sample.frame, { sample, next: samples[index + 1] }]]
            : [],
        )
      : [],
  );
  const timelineMismatches = validCompletions
    ? completions.filter((entry) => {
        const point = samplesByFrame.get(entry.frame);
        return (
          !validObservation ||
          !point ||
          entry.submittedAt < observation.startedAtMs ||
          entry.submittedAt > observation.stoppedAtMs ||
          (entryClock
            ? !validEntries ||
              entry.submittedAt < point.sample.callbackEnteredAt ||
              entry.submittedAt >
                (point.next?.callbackEnteredAt ?? observation.stoppedAtMs)
            : entry.submittedAt < point.sample.at - 0.5 ||
              entry.submittedAt >
                (point.next?.at ?? point.sample.at + limits.maxLatencyMs) + 0.5)
        );
      }).length
    : completions.length;
  check(
    "completion.timeline",
    validObservation && timelineMismatches === 0,
    timelineMismatches,
    entryClock
      ? "Pre-finish submission belongs to its actual callback-entry interval; scheduled rAF time is separately checked as opportunity evidence"
      : "Legacy pre-finish submission observation belongs to the recorder and callback interval; post-submit deadlines are checked separately",
  );
  const latency = validCompletions
    ? distribution(completions.map((entry) => entry.latencyMs))
    : null;
  for (const [quantile, limit] of [
    ["p95", limits.p95LatencyMs],
    ["p99", limits.maxLatencyMs],
    ["max", limits.maxLatencyMs],
  ])
    check(
      `completion.latency-${quantile}`,
      latency && latency[quantile] <= limit,
      latency?.[quantile] ?? null,
      limit,
    );
  const postSubmitPresent = completions.some(
    (entry) => object(entry) && Object.hasOwn(entry, "submitReturnedAt"),
  );
  const postSubmitRequired = mode === "qualification" || postSubmitPresent;
  const postSubmitValid =
    validCompletions &&
    accepted.length > 0 &&
    completions.length === accepted.length &&
    completions.every(
      (entry, index) =>
        nonnegative(entry.submitReturnedAt) &&
        entry.submitReturnedAt >= entry.submittedAt &&
        entry.submitReturnedAt <= entry.completedAt &&
        (!index ||
          entry.submitReturnedAt >= completions[index - 1].submitReturnedAt),
    );
  check(
    "enqueue.return-availability",
    postSubmitValid,
    {
      observed: completions.filter((entry) =>
        nonnegative(entry?.submitReturnedAt),
      ).length,
      accepted: accepted.length,
    },
    "A finite successful queue.submit return timestamp for every accepted frame",
    postSubmitRequired,
  );
  const returnTimelineMismatches = postSubmitValid
    ? completions.filter((entry) => {
        const point = samplesByFrame.get(entry.frame);
        return (
          !validObservation ||
          !point ||
          entry.submitReturnedAt < observation.startedAtMs ||
          entry.submitReturnedAt > observation.stoppedAtMs ||
          entry.submitReturnedAt < point.sample.at - 0.5 ||
          (entryClock &&
            (!validEntries ||
              entry.submitReturnedAt < point.sample.callbackEnteredAt ||
              entry.submitReturnedAt >
                (point.next?.callbackEnteredAt ?? observation.stoppedAtMs)))
        );
      }).length
    : completions.length;
  check(
    "enqueue.return-timeline",
    postSubmitValid && returnTimelineMismatches === 0,
    returnTimelineMismatches,
    entryClock
      ? "Post-submit returns follow actual callback entry and precede the next actual entry, while retaining the independent scheduled-opportunity bound"
      : "Post-submit returns belong to the recorder and follow the corresponding accepted opportunity",
    postSubmitRequired,
  );
  const enqueueDelays = postSubmitValid
    ? completions.map((entry) => {
        const point = samplesByFrame.get(entry.frame);
        return point
          ? Math.max(0, entry.submitReturnedAt - point.sample.at)
          : null;
      })
    : [];
  const enqueueDelay = distribution(enqueueDelays);
  check(
    "enqueue.delay-coverage",
    enqueueDelay?.count === accepted.length && accepted.length > 0,
    enqueueDelay?.count ?? 0,
    accepted.length,
    postSubmitRequired,
  );
  check(
    "enqueue.delay-p95",
    enqueueDelay && enqueueDelay.p95 <= budget,
    enqueueDelay?.p95 ?? null,
    budget,
    postSubmitRequired,
  );
  check(
    "enqueue.delay-max",
    enqueueDelay && enqueueDelay.max <= limits.maxIntervalMs,
    enqueueDelay?.max ?? null,
    limits.maxIntervalMs,
    postSubmitRequired,
  );
  const callbackEntryDelay =
    entryClock && validEntries && postSubmitValid
      ? distribution(
          completions.map((entry) => {
            const point = samplesByFrame.get(entry.frame);
            return point &&
              entry.submitReturnedAt >= point.sample.callbackEnteredAt
              ? entry.submitReturnedAt - point.sample.callbackEnteredAt
              : null;
          }),
        )
      : null;
  if (entryClock) {
    check(
      "enqueue.entry-delay-coverage",
      callbackEntryDelay?.count === accepted.length && accepted.length > 0,
      callbackEntryDelay?.count ?? 0,
      accepted.length,
    );
    check(
      "enqueue.entry-delay-p95",
      callbackEntryDelay && callbackEntryDelay.p95 <= budget,
      callbackEntryDelay?.p95 ?? null,
      budget,
    );
    check(
      "enqueue.entry-delay-max",
      callbackEntryDelay && callbackEntryDelay.max <= limits.maxIntervalMs,
      callbackEntryDelay?.max ?? null,
      limits.maxIntervalMs,
    );
  }
  const submitReturnIntervals = postSubmitValid
    ? completions
        .slice(1)
        .map(
          (entry, index) =>
            entry.submitReturnedAt - completions[index].submitReturnedAt,
        )
    : [];
  const finishAndSubmitCpu = postSubmitValid
    ? distribution(
        completions.map((entry) => entry.submitReturnedAt - entry.submittedAt),
      )
    : null;
  const fenceIntervals = validCompletions
    ? completions
        .slice(1)
        .map(
          (entry, index) => entry.completedAt - completions[index].completedAt,
        )
    : [];
  const enqueueIntervals = validCompletions
    ? completions
        .slice(1)
        .map(
          (entry, index) => entry.submittedAt - completions[index].submittedAt,
        )
    : [];

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

  const counterFields = [
    "frameAttempts",
    "submittedFrames",
    "skippedFrames",
    "rawCallbacks",
    "intentionalPacedCallbacks",
    "kernelDroppedSeconds",
    "hostClampedSeconds",
  ];
  const productCounters = options.productCounters;
  const hasCounters = productCounters !== undefined && productCounters !== null;
  const snapshotValid = (value) =>
    object(value) &&
    counterFields.every((key) =>
      key.endsWith("Seconds") ? nonnegative(value[key]) : integer(value[key]),
    ) &&
    value.frameAttempts === value.submittedFrames + value.skippedFrames;
  const countersValid =
    hasCounters &&
    snapshotValid(productCounters.before) &&
    snapshotValid(productCounters.after) &&
    counterFields.every(
      (key) => productCounters.after[key] >= productCounters.before[key],
    );
  const countersRequired = mode === "qualification" || hasCounters;
  check(
    "counters.availability",
    hasCounters,
    hasCounters,
    "Authoritative snapshots returned by benchmark start and stop",
    mode === "qualification",
  );
  check(
    "counters.valid",
    countersValid,
    countersValid,
    "Finite monotonic whole-window counters with attempts equal submissions plus rejections",
    countersRequired,
  );
  const counterDelta = countersValid
    ? Object.fromEntries(
        counterFields.map((key) => [
          key,
          productCounters.after[key] - productCounters.before[key],
        ]),
      )
    : null;
  const countersMatch =
    countersValid &&
    validCounts &&
    counterDelta.frameAttempts === samples.length &&
    counterDelta.submittedFrames === accepted.length &&
    counterDelta.skippedFrames === samples.length - accepted.length &&
    counterDelta.rawCallbacks === callbackCounts.rawCallbacks &&
    counterDelta.intentionalPacedCallbacks ===
      callbackCounts.intentionalPacedCallbacks &&
    counterDelta.rawCallbacks -
      counterDelta.intentionalPacedCallbacks -
      counterDelta.frameAttempts ===
      callbackCounts.suspendedCallbacks;
  check(
    "counters.samples-match",
    countersMatch,
    counterDelta,
    "Whole-window callback/submission deltas match all recorded samples and callback accounting",
    countersRequired,
  );
  const frozenCounters = result?.resources?.frameCounters;
  check(
    "counters.frozen-end",
    countersValid &&
      snapshotValid(frozenCounters) &&
      counterFields.every(
        (key) => frozenCounters[key] === productCounters.after[key],
      ),
    object(frozenCounters)
      ? Object.fromEntries(
          counterFields.map((key) => [
            key,
            nonnegative(frozenCounters[key]) ? frozenCounters[key] : null,
          ]),
        )
      : null,
    "The stopped result retains the exact authoritative end counters",
    countersRequired,
  );
  const wholeWindowDropped = countersValid
    ? Object.fromEntries(
        ["kernelDroppedSeconds", "hostClampedSeconds"].map((key) => [
          key,
          counterDelta[key],
        ]),
      )
    : null;
  for (const key of ["kernelDroppedSeconds", "hostClampedSeconds"]) {
    check(
      `time.whole-window.${key}`,
      countersValid && counterDelta[key] === 0,
      wholeWindowDropped?.[key] ?? null,
      0,
      countersRequired,
    );
    check(
      `time.counter-bounds.${key}`,
      countersValid &&
        samples.every(
          (sample) =>
            nonnegative(sample?.[key]) &&
            sample[key] >= productCounters.before[key] &&
            sample[key] <= productCounters.after[key],
        ),
      countersValid,
      "Every sampled cumulative counter lies between the authoritative start and stop snapshots",
      countersRequired,
    );
  }
  const fullGpuScope = result?.resources?.renderer?.fullGpuScope ?? null;
  const scopeValid = fullGpuScope === "render-compute-pass-envelope";
  check(
    "gpu.scope",
    scopeValid,
    typeof fullGpuScope === "string" ? fullGpuScope : null,
    "render-compute-pass-envelope",
    requireFullGpu,
  );
  const timingStatus = result?.resources?.renderer?.gpuTimingStatus;
  const timingStatusPresent =
    timingStatus !== undefined && timingStatus !== null;
  const timingStatusValid =
    !timingStatusPresent ||
    (object(timingStatus) &&
      timingStatus.scope === "render-compute-pass-envelope" &&
      timingStatus.availability === "available" &&
      timingStatus.observedFrames === accepted.length &&
      timingStatus.validFrames === accepted.length &&
      timingStatus.unavailableFrames === 0 &&
      timingStatus.pendingFrames === 0 &&
      timingStatus.lastFrame === lastFrame);
  check(
    "gpu.status-consistent",
    timingStatusValid,
    timingStatusPresent
      ? {
          scope: timingStatus.scope ?? null,
          availability: timingStatus.availability ?? null,
          observedFrames: timingStatus.observedFrames ?? null,
          validFrames: timingStatus.validFrames ?? null,
          unavailableFrames: timingStatus.unavailableFrames ?? null,
          pendingFrames: timingStatus.pendingFrames ?? null,
          lastFrame: timingStatus.lastFrame ?? null,
        }
      : null,
    "If recorded, GPU status matches the same complete stopped frame range and render/compute scope",
    requireFullGpu,
  );

  const gpuValues = accepted.map((sample) => sample.fullGpuMs ?? null);
  const malformedGpu = gpuValues.filter(
    (value) => value !== null && !nonnegative(value),
  ).length;
  const gpu = malformedGpu ? null : distribution(gpuValues);
  const gpuComplete =
    accepted.length > 0 && !malformedGpu && gpu?.count === accepted.length;
  const malformedHdr = accepted.filter(
    (sample) =>
      sample.gpuMs !== undefined &&
      sample.gpuMs !== null &&
      !nonnegative(sample.gpuMs),
  ).length;
  check(
    "gpu.main-hdr-values-valid",
    malformedHdr === 0,
    malformedHdr,
    "Provided main-HDR intervals are null or finite nonnegative values",
    requireFullGpu,
  );
  const gpuScopeViolations = accepted.filter(
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
      accepted: accepted.length,
      malformed: malformedGpu,
    },
    "One valid render/compute pass envelope for every accepted frame",
    requireFullGpu,
  );
  check(
    "gpu.budget-p95",
    gpuComplete && gpu.p95 <= budget,
    gpu?.p95 ?? null,
    budget,
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
    gpuComplete &&
    scopeValid &&
    timingStatusValid &&
    malformedHdr === 0 &&
    gpuScopeViolations === 0 &&
    gpu.p95 <= budget;
  const cpu = {};
  let malformedCpu = 0;
  for (const key of CPU_STAGES) {
    const values = samples.map((sample) => sample?.[key] ?? null);
    const malformed = values.filter(
      (value) => value !== null && !nonnegative(value),
    ).length;
    malformedCpu += malformed;
    const stats = malformed ? null : distribution(values);
    const acceptedStats = malformed
      ? null
      : distribution(accepted.map((sample) => sample[key] ?? null));
    cpu[key] = {
      distribution: stats,
      acceptedDistribution: acceptedStats,
      malformed,
      p95WithinTargetPeriod: acceptedStats ? acceptedStats.p95 <= budget : null,
      scope:
        "Diagnostic only; all-attempt and accepted-only distributions are separate. Nested stages overlap and must not be summed",
    };
  }
  check(
    "samples.cpu-values-valid",
    malformedCpu === 0,
    malformedCpu,
    "Optional CPU samples are null or finite nonnegative intervals",
  );
  check(
    "qualification.duration",
    validObservation && durationMs >= 30000,
    durationMs,
    30000,
    mode === "qualification",
  );
  check(
    "qualification.warmup",
    warmupMs >= 5000,
    warmupMs,
    5000,
    mode === "qualification",
  );
  const ok = checks.every((entry) => !entry.required || entry.ok);
  const windowQualified = mode === "qualification" && ok;
  return {
    schema: entryClock
      ? "aethyrforge.application-pacing.v2"
      : "aethyrforge.application-pacing.v1",
    ok,
    windowQualified,
    qualified: false,
    gpuBudgetWindowQualified: windowQualified && Boolean(gpuPassed),
    browserMode,
    compositorPresentation: null,
    physicalPresentation: null,
    checks,
    summary: {
      mode,
      minHz,
      durationMs,
      warmupMs,
      targetHz: targetHz ?? null,
      requireFullGpu,
      sampleScope: result?.sampleScope ?? null,
      callbackCounts: validCounts ? { ...callbackCounts } : null,
      acceptedFrames: accepted.length,
      completedFrames: completions.length,
      rendererRejectedAttempts: validSamples
        ? samples.length - accepted.length
        : null,
      acceptedRateHz: validObservation
        ? (accepted.length * 1000) / durationMs
        : null,
      completedRateHz: validObservation
        ? (completions.length * 1000) / durationMs
        : null,
      acceptedOpportunityIntervalMs: opportunityCadence,
      acceptedEqualTimestamps: equalOpportunities,
      acceptedSlowIntervals: slowIntervals,
      acceptedSlowFraction: slowFraction,
      acceptedStalls: stalls,
      queueLatencyMs: latency,
      postSubmitReturnEvidenceAvailable: postSubmitValid,
      ...(entryClock
        ? {
            callbackClock: "entry",
            callbackEntryEvidenceAvailable: validEntries,
            callbackEntryToEnqueueDelayMs: callbackEntryDelay,
            callbackEnteredIntervalMs: validEntries
              ? distribution(
                  samples
                    .slice(1)
                    .map(
                      (sample, index) =>
                        sample.callbackEnteredAt -
                        samples[index].callbackEnteredAt,
                    ),
                )
              : null,
          }
        : {}),
      opportunityToEnqueueDelayMs: enqueueDelay,
      submitReturnedIntervalMs: distribution(submitReturnIntervals),
      finishAndSubmitCpuMs: finishAndSubmitCpu,
      legacySubmissionStartIntervalMs: distribution(enqueueIntervals),
      fenceNotificationIntervalMs: distribution(fenceIntervals),
      equalFenceNotificationTimes: fenceIntervals.filter((value) => value === 0)
        .length,
      fullGpuScope: typeof fullGpuScope === "string" ? fullGpuScope : null,
      gpuTimingStatusConsistent: timingStatusValid,
      fullGpuMs: gpu,
      missingFullGpuSamples: accepted.length - (gpu?.count ?? 0),
      gpuScopeViolations,
      malformedMainHdrSamples: malformedHdr,
      cpu,
      dropped,
      wholeWindowDropped,
      authoritativeCountersAvailable: countersValid,
      productCounters: countersValid
        ? {
            before: Object.fromEntries(
              counterFields.map((key) => [key, productCounters.before[key]]),
            ),
            after: Object.fromEntries(
              counterFields.map((key) => [key, productCounters.after[key]]),
            ),
          }
        : null,
      presentationMs: null,
    },
    assumptions: [
      ...(entryClock
        ? [
            "Version 2 distinguishes scheduled rAF opportunity timestamps from explicit performance.now callback entry. CPU ordering uses actual entries; scheduled opportunity cadence and enqueue deadlines remain independently required.",
          ]
        : []),
      "Render-attempt timestamps identify rAF opportunities in the same worker performance clock as recorder and post-submit return observations; they do not timestamp display presentation.",
      "The host reports submitted=true only after actual queue submission. Recorded callbacks and queue observations are trusted instrumentation, not remote attestation.",
      "A single first callback may carry a vsync timestamp just before recorder start; actual enqueue time must still fall inside the recorder window.",
    ],
    limitations: [
      ...(entryClock
        ? [
            "Version 2 changes only the callback-clock interpretation with new explicit entry evidence. Legacy observations without entry fields or marker retain their original version 1 evaluations; historical reports are not relabeled.",
          ]
        : []),
      "This independent application verdict never rewrites or replaces the strict v1 fence-cadence verdict or historical reports.",
      "Qualified remains false for one window. A caller must require at least two independent 30-second windows with five-second warmup and the complete source, quality, resolution, route, feature, and surface matrix.",
      "Intentional pacing callbacks are separate from genuine renderer rejections. Accepted count and cadence determine the requested application target; a rejected display-rate attempt is not automatically a missed 60 Hz accepted-frame deadline.",
      "Post-submit return intervals, legacy pre-finish submission intervals, and fence-notification intervals remain diagnostics. Opportunity-to-post-submit delay has its own scheduling deadline. None establishes display cadence. The pre-finish-to-return CPU duration includes encoder.finish and queue.submit.",
      "Queue latency includes host notification delay and conservatively bounds backlog. Its cadence is not used as a presentation proxy.",
      "Render/compute GPU envelopes exclude copies and uploads outside passes, query readback, browser composition, and physical presentation. CPU stages overlap; do not add them.",
      "Sampled simulation-loss deltas are diagnostic. Whole-window qualification additionally requires authoritative start/stop counters, including any loss in the first timed frame.",
      "Headless and headed modes can both supply application evidence. Neither supplies compositor or physical presentation evidence through this evaluator.",
      ...(!gpuComplete
        ? [
            "GPU envelopes are incomplete or unavailable; no whole-envelope GPU-budget claim is supported.",
          ]
        : []),
      ...(mode === "diagnostic"
        ? [
            "A diagnostic observation cannot qualify the sustained window requirement.",
          ]
        : []),
    ],
  };
}

/** Interpret retained CDP observations, never launcher preferences. */
export function applicationBrowserMode(evidence) {
  if (
    evidence?.schema !== "aethyrforge.browser-mode.v1" ||
    typeof evidence.userAgent !== "string" ||
    typeof evidence.commandLineAvailable !== "boolean" ||
    !Array.isArray(evidence.headlessArguments) ||
    !evidence.headlessArguments.every(
      (value) => typeof value === "string" && /^--headless(?:=|$)/.test(value),
    )
  )
    return "unknown";
  if (
    /HeadlessChrome/.test(evidence.userAgent) ||
    evidence.headlessArguments.length
  )
    return "headless";
  return evidence.commandLineAvailable ? "headed" : "unknown";
}
export async function observeApplicationBrowserMode(browser) {
  let session,
    version = null,
    command = null;
  const errors = [];
  try {
    session = await browser.newBrowserCDPSession();
    version = await session.send("Browser.getVersion").catch((error) => {
      errors.push(String(error).slice(0, 512));
      return null;
    });
    command = await session
      .send("Browser.getBrowserCommandLine")
      .catch((error) => {
        errors.push(String(error).slice(0, 512));
        return null;
      });
  } catch (error) {
    errors.push(String(error).slice(0, 512));
  } finally {
    if (session)
      await session.detach().catch((error) => {
        errors.push(String(error).slice(0, 512));
      });
  }
  const evidence = {
    schema: "aethyrforge.browser-mode.v1",
    product: typeof version?.product === "string" ? version.product : "",
    userAgent: typeof version?.userAgent === "string" ? version.userAgent : "",
    commandLineAvailable: Boolean(command && Array.isArray(command.arguments)),
    headlessArguments: (Array.isArray(command?.arguments)
      ? command.arguments
      : []
    ).filter(
      (value) => typeof value === "string" && /^--headless(?:=|$)/.test(value),
    ),
  };
  return {
    ...evidence,
    mode: applicationBrowserMode(evidence),
    scope:
      "Actual CDP browser command-line and user-agent observations; no display-presentation claim",
    ...(errors.length ? { errors } : {}),
  };
}
