import assert from "node:assert/strict";
import { createHash } from "node:crypto";

export const MEASUREMENT_SCHEMA = "aethyrforge.measurement.v1";
export const QUANTILE_METHOD = "linear-n-minus-one";
const finite = (value) => typeof value === "number" && Number.isFinite(value);
const nonnegative = (value) => finite(value) && value >= 0;
const record = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const digest = (value) =>
  value === null || (typeof value === "string" && /^[a-f0-9]{64}$/.test(value));

/** Exact legacy interpolation. Missing observations remain distinct from zero. */
export function distribution(values) {
  assert(Array.isArray(values), "Use an array of measurements.");
  assert(
    values.every((value) => value == null || nonnegative(value)),
    "Measurements must be finite, nonnegative, or explicitly unavailable.",
  );
  const sorted = values.filter((value) => value != null).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const quantile = (p) => {
    const index = p * (sorted.length - 1),
      low = Math.floor(index),
      fraction = index - low;
    return (
      sorted[low] +
      fraction * (sorted[Math.min(low + 1, sorted.length - 1)] - sorted[low])
    );
  };
  return {
    count: sorted.length,
    min: sorted[0],
    p50: quantile(0.5),
    p95: quantile(0.95),
    p99: quantile(0.99),
    max: sorted.at(-1),
    mean: sorted.reduce((a, b) => a + b, 0) / sorted.length,
    missing: values.length - sorted.length,
  };
}
/** Existing C-town callers keep their original median field and result shape. */
export function statistics(values) {
  assert(
    Array.isArray(values) && values.length > 0 && values.every(nonnegative),
    "Benchmark samples must be finite, nonnegative, and nonempty.",
  );
  const { p50, missing: _missing, ...rest } = distribution(values);
  return {
    count: rest.count,
    min: rest.min,
    median: p50,
    p95: rest.p95,
    p99: rest.p99,
    max: rest.max,
    mean: rest.mean,
  };
}
/** Keep invalid browser byte observations distinct from a measured zero. */
export function summarizeResponseBodies(entries) {
  assert(Array.isArray(entries), "Use observed browser responses.");
  let knownResponseBodyBytes = 0,
    unavailableResponses = 0,
    invalidResponses = 0;
  for (const entry of entries) {
    const value = entry.responseBodySize;
    if (Number.isSafeInteger(value) && value >= 0) {
      knownResponseBodyBytes += value;
      assert(
        Number.isSafeInteger(knownResponseBodyBytes),
        "Response byte totals exceed the supported range.",
      );
    } else {
      unavailableResponses++;
      if (value != null) invalidResponses++;
    }
  }
  return {
    responseBodyBytes: unavailableResponses ? null : knownResponseBodyBytes,
    knownResponseBodyBytes,
    responses: entries.length,
    unavailableResponses,
    invalidResponses,
  };
}
/** Paired reversed orders avoid always measuring the same candidate first. */
export function balancedOrder(candidates, rounds) {
  assert(
    Array.isArray(candidates) &&
      candidates.length > 0 &&
      candidates.length <= 32 &&
      new Set(candidates).size === candidates.length &&
      candidates.every(
        (value) =>
          typeof value === "string" ||
          (typeof value === "number" && Number.isFinite(value)),
      ),
    "Choose distinct bounded benchmark candidates.",
  );
  assert(
    Number.isSafeInteger(rounds) && rounds > 0 && rounds <= 128,
    "Choose 1 to 128 benchmark rounds.",
  );
  return Array.from({ length: rounds }, (_, round) => {
    const shift = Math.floor(round / 2) % candidates.length;
    const order = candidates.map(
      (_, index) => candidates[(index + shift) % candidates.length],
    );
    return round % 2 ? order.reverse() : order;
  });
}
const frameMetrics = [
  "stepMs",
  "renderCpuMs",
  "cullMs",
  "clusterMs",
  "gpuMs",
  "bridgeEncodeMs",
  "wasmRequestMs",
  "bridgeDecodeMs",
  "bridgeOverheadMs",
  "commandEncodeMs",
];
export const FRAME_TIMING_SCOPES = Object.freeze({
  stepMs:
    "Whole kernel bridge, gameplay, and camera update; contains bridge and Wasm request intervals.",
  wasmRequestMs:
    "Wasm request wall interval including Rust request decoding, work, and response encoding; not isolated simulation math.",
  renderCpuMs:
    "Whole render call including culling, lighting preparation, command encoding, and submission; overlaps those nested stages.",
  commandEncodeMs:
    "createCommandEncoder through finish/submit; contains nested encoding callbacks, culling, and lighting work where applicable.",
  gpuMs:
    "Timestamp duration of the main HDR pass only when available; excludes other passes and presentation.",
  cadence:
    "Worker callbacks and accepted submissions are separate from displayed presentation; CPU span totals are not additive across overlapping scopes.",
  dropped:
    "Last minus first cumulative counters in the observed callback window; never sum cumulative totals.",
  memory:
    "Explicitly accounted allocations are separate from process, browser, driver, and total GPU memory.",
});
export function summarizeFrames(samples) {
  assert(Array.isArray(samples), "Use bounded frame observations.");
  for (let index = 0; index < samples.length; index++) {
    const sample = samples[index];
    assert(
      record(sample) &&
        nonnegative(sample.at) &&
        typeof sample.submitted === "boolean" &&
        (index === 0 || sample.at >= samples[index - 1].at),
      "Frame observations need ordered timestamps and explicit submission outcomes.",
    );
    assert(
      [
        ...frameMetrics,
        "elapsedMs",
        "kernelDroppedSeconds",
        "hostClampedSeconds",
      ].every((key) => sample[key] == null || nonnegative(sample[key])),
      "Frame measurements must be finite, nonnegative, or unavailable.",
    );
  }
  const accepted = samples.filter((sample) => sample.submitted);
  const measurementMs =
    samples.length > 1 ? samples.at(-1).at - samples[0].at : 0;
  const intervals = accepted
    .slice(1)
    .map((sample, index) => sample.at - accepted[index].at);
  const metrics = Object.fromEntries(
    frameMetrics.map((key) => [
      key,
      distribution(
        (key === "gpuMs" ? accepted : samples).map((sample) => sample[key]),
      ),
    ]),
  );
  const acceptedMetrics = Object.fromEntries(
    frameMetrics.map((key) => [
      key,
      distribution(accepted.map((sample) => sample[key])),
    ]),
  );
  const cumulativeDelta = (key) => {
    if (!samples.length || samples.some((sample) => sample[key] == null))
      return null;
    const values = samples.map((sample) => sample[key]);
    assert(
      values.every(
        (value, index) =>
          nonnegative(value) && (!index || value >= values[index - 1]),
      ),
      "Dropped-time counters must be finite and cumulative.",
    );
    return values.at(-1) - values[0];
  };
  return {
    callbacks: samples.length,
    acceptedSubmissions: accepted.length,
    skippedCallbacks: samples.length - accepted.length,
    measurementMs,
    acceptedSubmissionsPerSecond:
      measurementMs > 0
        ? ((accepted.length - (samples[0].submitted ? 1 : 0)) * 1000) /
          measurementMs
        : null,
    callbackIntervalMs: distribution(samples.map((sample) => sample.elapsedMs)),
    submissionIntervalMs: distribution(intervals),
    metrics,
    acceptedMetrics,
    dropped: {
      kernelSeconds: cumulativeDelta("kernelDroppedSeconds"),
      hostClampedSeconds: cumulativeDelta("hostClampedSeconds"),
    },
    presentationMs: null,
    scopes: FRAME_TIMING_SCOPES,
  };
}
export function summarizeSamples(samples) {
  assert(Array.isArray(samples) && samples.length > 0);
  return Object.fromEntries(
    [
      "frameMs",
      "cpuMs",
      "simulationMs",
      "cullingMs",
      "clusterMs",
      "renderSubmitMs",
      "gpuMs",
    ].map((key) => {
      const measured = distribution(samples.map((sample) => sample[key]));
      if (!measured) return [key, null];
      const { p50, ...rest } = measured;
      return [key, { ...rest, median: p50 }];
    }),
  );
}
export function frameSummaries(rawSamples) {
  assert(Array.isArray(rawSamples) && rawSamples.length > 0);
  const samples = rawSamples.map((sample) => ({
    ...sample,
    frameMs: sample.elapsedMs,
    cpuMs:
      nonnegative(sample.stepMs) && nonnegative(sample.renderCpuMs)
        ? sample.stepMs + sample.renderCpuMs
        : null,
    simulationMs: sample.stepMs,
    cullingMs: sample.cullMs,
    clusterMs: sample.clusterMs,
    renderSubmitMs: sample.renderCpuMs,
    gpuMs: sample.gpuMs,
  }));
  let previousSubmission;
  const submitted = samples
    .filter((sample) => sample.submitted)
    .map((sample) => {
      const frameMs =
        previousSubmission === undefined
          ? null
          : sample.at - previousSubmission;
      previousSubmission = sample.at;
      return { ...sample, frameMs };
    });
  const intervals = submitted
    .map((sample) => sample.frameMs)
    .filter((value) => value !== null);
  const windowMs = samples.at(-1).at - samples[0].at;
  return {
    summary: summarizeSamples(samples),
    submittedSummary: submitted.length ? summarizeSamples(submitted) : null,
    submissionIntervalsMs: intervals.length ? statistics(intervals) : null,
    submissionMeasurementMs: windowMs,
    submittedFramesPerSecond:
      windowMs > 0
        ? ((submitted.length - (samples[0].submitted ? 1 : 0)) * 1000) /
          windowMs
        : null,
  };
}

/** Retain native evidence identity and limitations without rewriting old measurements. */
export function archiveEvidence(inputs) {
  assert(
    Array.isArray(inputs) && inputs.length <= 128,
    "Choose bounded evidence references.",
  );
  return inputs.map(({ path, bytes, issue, limitations }) => {
    assert(
      typeof path === "string" &&
        path.length > 0 &&
        path.length <= 1024 &&
        bytes instanceof Uint8Array &&
        bytes.byteLength <= 128 * 1024 * 1024 &&
        (issue === undefined || (Number.isSafeInteger(issue) && issue > 0)) &&
        Array.isArray(limitations) &&
        limitations.length > 0 &&
        limitations.every(
          (value) =>
            typeof value === "string" &&
            value.length > 0 &&
            value.length <= 4096,
        ),
      "Archive evidence needs exact bytes, its path, and explicit limitations.",
    );
    const native = JSON.parse(new TextDecoder().decode(bytes));
    assert(
      record(native) && typeof native.schema === "string",
      "Archive a native versioned JSON report.",
    );
    return {
      path,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      bytes: bytes.byteLength,
      schema: native.schema,
      ...(issue === undefined ? {} : { issue }),
      limitations: [...limitations],
    };
  });
}
function finiteJSON(value, depth = 0) {
  assert(depth <= 64, "Keep measurement report nesting bounded.");
  if (typeof value === "number")
    assert(
      Number.isFinite(value),
      "Unavailable measurements must use null, not NaN or Infinity.",
    );
  else if (Array.isArray(value))
    value.forEach((child) => finiteJSON(child, depth + 1));
  else if (record(value))
    Object.values(value).forEach((child) => finiteJSON(child, depth + 1));
  else
    assert(
      value === null || ["boolean", "string"].includes(typeof value),
      "Use JSON values in measurement reports.",
    );
}
export function validateMeasurementReport(value) {
  assert(
    record(value) &&
      value.schema === MEASUREMENT_SCHEMA &&
      value.quantile === QUANTILE_METHOD &&
      typeof value.id === "string" &&
      value.id.length > 0 &&
      typeof value.ok === "boolean",
    "Use a versioned measurement report.",
  );
  assert(
    Number.isFinite(Date.parse(value.startedAt)) &&
      Number.isFinite(Date.parse(value.finishedAt)) &&
      Date.parse(value.finishedAt) >= Date.parse(value.startedAt),
    "Record ordered measurement times.",
  );
  const { environment, pins, cache, warmup } = value;
  assert(
    record(environment) &&
      [environment.browser, environment.os].every(
        (item) => item === null || typeof item === "string",
      ) &&
      (environment.gpu === null || record(environment.gpu)),
    "Pin browser, OS, and GPU, using null when unavailable.",
  );
  assert(
    record(pins) &&
      ["source", "assets", "kernel", "wgsl", "quality", "build"].every((key) =>
        record(pins[key]),
      ),
    "Pin source, assets, kernel, WGSL, quality, and build identities.",
  );
  assert(
    (pins.source.revision === null ||
      (typeof pins.source.revision === "string" &&
        /^[a-f0-9]{7,64}$/.test(pins.source.revision))) &&
      typeof pins.source.dirty === "boolean" &&
      digest(pins.source.patchSha256),
    "Pin source revision and working-tree changes explicitly.",
  );
  for (const name of ["assets", "kernel", "wgsl", "quality", "build"])
    assert(
      digest(pins[name].sha256),
      `Use an exact or unavailable ${name} identity.`,
    );
  assert(
    [pins.assets.scope, pins.wgsl.scope, pins.build.mode].every(
      (item) => typeof item === "string" && item.length > 0,
    ) &&
      (pins.kernel.abi === null ||
        typeof pins.kernel.abi === "string" ||
        Number.isSafeInteger(pins.kernel.abi)) &&
      record(pins.quality.settings) &&
      record(pins.build.toolchain),
    "Describe identity, quality, and build scopes.",
  );
  assert(
    record(cache) &&
      ["cold", "warm", "mixed", "not-applicable", "unknown"].includes(
        cache.state,
      ) &&
      typeof cache.scope === "string" &&
      cache.scope.length > 0,
    "Describe browser and OS cache conditions.",
  );
  assert(
    record(warmup) &&
      [warmup.durationMs, warmup.iterations].every(
        (item) => item === null || nonnegative(item),
      ),
    "State warmup duration and iterations, or null.",
  );
  assert(
    Array.isArray(value.trialOrder) &&
      value.trialOrder.length <= 128 &&
      value.trialOrder.every(
        (order) =>
          Array.isArray(order) &&
          order.length > 0 &&
          order.length <= 32 &&
          order.every((candidate) => typeof candidate === "string"),
      ),
    "Record actual bounded trial order.",
  );
  assert(
    Array.isArray(value.phases) &&
      value.phases.length > 0 &&
      value.phases.length <= 1024 &&
      value.phases.every(
        (phase) =>
          record(phase) &&
          typeof phase.id === "string" &&
          [
            "simulation-cpu",
            "integrated",
            "startup",
            "edit",
            "archive",
          ].includes(phase.kind) &&
          typeof phase.scope === "string" &&
          phase.scope.length > 0 &&
          record(phase.metrics),
      ),
    "Name phase timing boundaries and preserve native metrics.",
  );
  assert(
    Array.isArray(value.evidence) &&
      value.evidence.length <= 128 &&
      value.evidence.every(
        (item) =>
          record(item) &&
          typeof item.path === "string" &&
          digest(item.sha256) &&
          item.sha256 !== null &&
          Number.isSafeInteger(item.bytes) &&
          item.bytes >= 0 &&
          typeof item.schema === "string" &&
          Array.isArray(item.limitations) &&
          item.limitations.length > 0,
      ),
    "Keep exact native evidence references and limitations.",
  );
  assert(
    Array.isArray(value.limitations) &&
      value.limitations.length > 0 &&
      value.limitations.every(
        (item) => typeof item === "string" && item.length > 0,
      ),
    "State measurement limitations.",
  );
  finiteJSON(value);
  assert(
    new TextEncoder().encode(JSON.stringify(value)).length <= 64 * 1024 * 1024,
    "Keep measurement reports bounded.",
  );
  return structuredClone(value);
}
export function createMeasurementReport(input) {
  return validateMeasurementReport({
    ...input,
    schema: MEASUREMENT_SCHEMA,
    quantile: QUANTILE_METHOD,
  });
}

/** Check complete delivered artifacts against profile-specific build ceilings. */
export function checkByteBudgets(audit, budget) {
  assert(
    record(audit) &&
      typeof audit.ok === "boolean" &&
      record(audit.totals) &&
      record(budget),
    "Choose an emitted build audit and explicit profile budget.",
  );
  const checks = [],
    violations = [];
  const fields = {
    jsRawBytes: audit.totals.js?.rawBytes,
    jsGzipBytes: audit.totals.js?.gzipBytes,
    wasmRawBytes: audit.totals.wasm?.rawBytes,
    assetRawBytes: audit.totals.assets?.rawBytes,
    deliveredFiles: audit.requests,
  };
  assert(
    Object.keys(budget).length === Object.keys(fields).length,
    "Choose complete profile byte budgets.",
  );
  for (const [metric, max] of Object.entries(budget)) {
    assert(
      Object.hasOwn(fields, metric),
      "Choose supported byte-budget fields.",
    );
    const actual = fields[metric];
    assert(
      Number.isSafeInteger(max) &&
        max >= 0 &&
        Number.isSafeInteger(actual) &&
        actual >= 0,
      "Budget limits and observed sizes must be finite nonnegative integers.",
    );
    const ok = actual <= max;
    checks.push({ metric, actual, max, ok });
    if (!ok) violations.push(`${metric}: ${actual} exceeds ${max}.`);
  }
  if (!audit.ok)
    violations.push("The delivered artifact dependency audit failed.");
  return { ok: violations.length === 0, checks, violations };
}
