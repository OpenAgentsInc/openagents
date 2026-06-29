// Gym — GPT-OSS live latency playground runner (#6167).
//
// PURE, IO-FREE measurement + aggregation core for the `/gym/oss` surface. The
// page fires the SAME prompt N times at the GPT-OSS lane (`openagents/khala-oss-20b`)
// over the OpenAI-compatible streaming gateway, measures each sample, and reads
// the server-measured `openagents.telemetry` block off the terminal SSE frame.
// This module owns:
//
//   - the honest measured/not-measured number model (mirrors
//     `workers/api/src/inference/khala-telemetry.ts`: `not_measured` is the
//     ONLY honest absence — NEVER a fabricated `0`),
//   - the perceived-TPS / inter-token-latency derivations (MIRRORED EXACTLY from
//     `derivePerceivedTps` / `deriveInterTokenLatencyMs` in khala-telemetry.ts —
//     those are not exported and the web app cannot import workers/api, so the
//     formula is copied with this provenance note; keep the two in lockstep),
//   - reconciliation of client-stamped timing against the server telemetry block
//     (prefer server numbers when measured, fall back to client timing, mark
//     truly-unavailable fields `not_measured`),
//   - the concurrency runner (fire C in flight at a time, hard in-flight cap),
//   - the ramp sweep (1->2->4->8) shape,
//   - aggregation into P50/P90/P99/mean using the SAME percentile/mean math as
//     the benchmark report (`workers/api/src/inference/benchmark/report.ts`:
//     nearest-rank percentile, null for empty — never a fabricated 0; mirrored
//     here with the same provenance reason).
//
// The streaming call is injected as a seam (`SampleStream`) so the runner and
// aggregation are unit-testable offline with a deterministic fake stream and
// NEVER hit the network in tests.

import { recordFromUnknown } from '../../../json-boundary'

// ---------------------------------------------------------------------------
// Honest measured-number model (mirror of khala-telemetry.ts).
// ---------------------------------------------------------------------------

// `not_measured` means "no measurement exists" — distinct from a real measured
// 0. A field is NEVER fabricated or defaulted to a fake 0.
export const NOT_MEASURED = 'not_measured' as const
export type NotMeasured = typeof NOT_MEASURED
export type MeasuredNumber = number | NotMeasured

export const isMeasured = (value: MeasuredNumber): value is number =>
  value !== NOT_MEASURED

// Coerce a possibly-undefined/non-finite number into the honest model. Mirrors
// khala-telemetry `measured`: a non-finite or absent value is `not_measured`.
export const measured = (value: number | undefined | null): MeasuredNumber => {
  if (value === undefined || value === null) {
    return NOT_MEASURED
  }
  if (!Number.isFinite(value)) {
    return NOT_MEASURED
  }
  return value
}

// ---------------------------------------------------------------------------
// Percentile / mean (MIRROR of inference/benchmark/report.ts).
// ---------------------------------------------------------------------------

// Nearest-rank percentile over a sample array. Returns null for empty input
// (honest absence, never a fabricated 0). `p` in [0,100]. PURE: sorts a copy.
export const percentile = (
  values: ReadonlyArray<number>,
  p: number,
): number | null => {
  if (values.length === 0) {
    return null
  }
  const sorted = [...values].sort((a, b) => a - b)
  if (sorted.length === 1) {
    return sorted[0] ?? null
  }
  const rank = Math.min(
    sorted.length,
    Math.max(1, Math.ceil((p / 100) * sorted.length)),
  )
  return sorted[rank - 1] ?? null
}

// Arithmetic mean, null for empty. PURE.
export const mean = (values: ReadonlyArray<number>): number | null => {
  if (values.length === 0) {
    return null
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length
}

// ---------------------------------------------------------------------------
// Perceived-TPS / inter-token-latency derivations (MIRROR of khala-telemetry.ts).
// ---------------------------------------------------------------------------

// Derive mean inter-token latency from completion tokens + generation
// wall-clock. Sentinel unless BOTH are measured and there is >1 token (ITL is
// undefined for a single token). Never fabricated.
export const deriveInterTokenLatencyMs = (
  completionTokens: number | undefined,
  generationWallClockMs: number | undefined,
): MeasuredNumber => {
  if (
    completionTokens === undefined ||
    generationWallClockMs === undefined ||
    completionTokens <= 1 ||
    !Number.isFinite(generationWallClockMs) ||
    generationWallClockMs < 0
  ) {
    return NOT_MEASURED
  }
  return generationWallClockMs / (completionTokens - 1)
}

// Derive perceived tokens/second from completion tokens + generation wall-clock.
// Sentinel unless both are measured and wall-clock > 0.
export const derivePerceivedTps = (
  completionTokens: number | undefined,
  generationWallClockMs: number | undefined,
): MeasuredNumber => {
  if (
    completionTokens === undefined ||
    generationWallClockMs === undefined ||
    !Number.isFinite(generationWallClockMs) ||
    generationWallClockMs <= 0 ||
    completionTokens < 0
  ) {
    return NOT_MEASURED
  }
  return completionTokens / (generationWallClockMs / 1000)
}

// ---------------------------------------------------------------------------
// The GPT-OSS lane identity.
// ---------------------------------------------------------------------------

// The neutral lane alias is the identity surface (#6156): NEVER the raw upstream
// id. Driven from the `/v1/models` catalog where practical; this is the target.
export const GPT_OSS_MODEL_ID = 'openagents/khala-oss-20b'

// A hard cap on total in-flight requests so a ramp can't wedge the box. This is
// the lane safety bound (the lane is hourly, no per-call balance gate).
export const MAX_IN_FLIGHT = 8

// The concurrency dial values and the ramp sweep steps.
export const CONCURRENCY_OPTIONS: ReadonlyArray<number> = [1, 2, 4, 8]
export const RAMP_STEPS: ReadonlyArray<number> = [1, 2, 4, 8]

// Clamp a requested concurrency into [1, MAX_IN_FLIGHT].
export const clampConcurrency = (requested: number): number =>
  Math.min(MAX_IN_FLIGHT, Math.max(1, Math.trunc(requested)))

// ---------------------------------------------------------------------------
// The server telemetry block we read off the terminal SSE frame.
// ---------------------------------------------------------------------------

// The subset of `openagents.telemetry` (KhalaTelemetryBlock) the playground
// reads. Each numeric is a real number OR the literal `'not_measured'`.
export type ServerTelemetry = Readonly<{
  ttftMs: MeasuredNumber
  totalWallClockMs: MeasuredNumber
  promptTokens: MeasuredNumber
  completionTokens: MeasuredNumber
  totalTokens: MeasuredNumber
  cachedInputTokens: MeasuredNumber
}>

// Parse a raw `openagents.telemetry` value (possibly absent / malformed) into
// the honest model. Anything missing/non-finite is `not_measured`, never 0.
export const parseServerTelemetry = (raw: unknown): ServerTelemetry | null => {
  const record = recordFromUnknown(raw)
  if (record === undefined) {
    return null
  }
  const pick = (key: string): MeasuredNumber => {
    const value = record[key]
    if (value === NOT_MEASURED) {
      return NOT_MEASURED
    }
    if (typeof value === 'number') {
      return measured(value)
    }
    return NOT_MEASURED
  }
  return {
    ttftMs: pick('ttftMs'),
    totalWallClockMs: pick('totalWallClockMs'),
    promptTokens: pick('promptTokens'),
    completionTokens: pick('completionTokens'),
    totalTokens: pick('totalTokens'),
    cachedInputTokens: pick('cachedInputTokens'),
  }
}

// ---------------------------------------------------------------------------
// The client-stamped timing for one sample (perf marks).
// ---------------------------------------------------------------------------

export type ClientTiming = Readonly<{
  // ms from request start to the first content byte (client TTFT).
  firstContentByteMs: number | undefined
  // ms from request start to stream end (client total wall-clock).
  endMs: number | undefined
  // ms from the first content byte to stream end (client generation wall-clock,
  // the window perceived TPS / ITL are derived over).
  generationMs: number | undefined
  // content tokens observed client-side (delta count; coarse but honest).
  observedContentDeltas: number
}>

// ---------------------------------------------------------------------------
// One measured sample (reconciled).
// ---------------------------------------------------------------------------

export type SampleStatus = 'ok' | 'failed'

export type SampleResult = Readonly<{
  index: number
  status: SampleStatus
  // Failure reason when status === 'failed'. A failed run shows the failure,
  // NEVER a fabricated latency.
  error: string | null
  // Reconciled metrics (server-preferred, client fallback, else not_measured).
  ttftMs: MeasuredNumber
  totalWallClockMs: MeasuredNumber
  perceivedTps: MeasuredNumber
  interTokenLatencyMs: MeasuredNumber
  completionTokens: MeasuredNumber
  // Provenance per metric so the UI can show whether a number came from the
  // server telemetry block or client perf marks (or is unavailable).
  source: Readonly<{
    ttft: MetricSource
    totalWallClock: MetricSource
    tps: MetricSource
  }>
}>

export type MetricSource = 'server' | 'client' | 'not_measured'

// Reconcile one sample's client timing + server telemetry into honest metrics.
// PURE. Prefers the server-measured number when present, falls back to client
// timing, and marks anything truly unavailable `not_measured` (never 0). A
// failed sample carries the error and all-`not_measured` metrics.
export const reconcileSample = (input: {
  index: number
  status: SampleStatus
  error?: string | null
  client: ClientTiming
  server: ServerTelemetry | null
}): SampleResult => {
  if (input.status === 'failed') {
    return {
      index: input.index,
      status: 'failed',
      error: input.error ?? 'request failed',
      ttftMs: NOT_MEASURED,
      totalWallClockMs: NOT_MEASURED,
      perceivedTps: NOT_MEASURED,
      interTokenLatencyMs: NOT_MEASURED,
      completionTokens: NOT_MEASURED,
      source: {
        ttft: 'not_measured',
        totalWallClock: 'not_measured',
        tps: 'not_measured',
      },
    }
  }

  const server = input.server
  const client = input.client

  // TTFT: server-preferred, then client first-content-byte.
  const serverTtft = server === null ? NOT_MEASURED : server.ttftMs
  const clientTtft = measured(client.firstContentByteMs)
  const ttftSource: MetricSource = isMeasured(serverTtft)
    ? 'server'
    : isMeasured(clientTtft)
      ? 'client'
      : 'not_measured'
  const ttftMs = isMeasured(serverTtft)
    ? serverTtft
    : isMeasured(clientTtft)
      ? clientTtft
      : NOT_MEASURED

  // Total wall-clock: server-preferred, then client end mark.
  const serverWall = server === null ? NOT_MEASURED : server.totalWallClockMs
  const clientWall = measured(client.endMs)
  const wallSource: MetricSource = isMeasured(serverWall)
    ? 'server'
    : isMeasured(clientWall)
      ? 'client'
      : 'not_measured'
  const totalWallClockMs = isMeasured(serverWall)
    ? serverWall
    : isMeasured(clientWall)
      ? clientWall
      : NOT_MEASURED

  // Completion tokens: server count preferred; client delta count is a coarse
  // honest fallback (one delta ~ one content chunk, not strictly one token).
  const serverCompletion =
    server === null ? NOT_MEASURED : server.completionTokens
  const completionTokens = isMeasured(serverCompletion)
    ? serverCompletion
    : client.observedContentDeltas > 0
      ? client.observedContentDeltas
      : NOT_MEASURED

  // Perceived TPS + ITL: derived over the generation window. Prefer the server
  // numbers (completion tokens + server generation wall-clock = total - ttft);
  // fall back to client generation window. Honest sentinel when neither yields.
  const serverGenerationMs =
    server !== null &&
    isMeasured(server.totalWallClockMs) &&
    isMeasured(server.ttftMs)
      ? Math.max(0, server.totalWallClockMs - server.ttftMs)
      : undefined
  const serverCompletionN = isMeasured(serverCompletion)
    ? serverCompletion
    : undefined

  const serverTps = derivePerceivedTps(serverCompletionN, serverGenerationMs)
  const tps = isMeasured(serverTps)
    ? serverTps
    : derivePerceivedTps(
        client.observedContentDeltas > 0
          ? client.observedContentDeltas
          : undefined,
        client.generationMs,
      )
  const tpsSource: MetricSource = isMeasured(serverTps)
    ? 'server'
    : isMeasured(tps)
      ? 'client'
      : 'not_measured'

  const serverItl = deriveInterTokenLatencyMs(
    serverCompletionN,
    serverGenerationMs,
  )
  const interTokenLatencyMs = isMeasured(serverItl)
    ? serverItl
    : deriveInterTokenLatencyMs(
        client.observedContentDeltas > 0
          ? client.observedContentDeltas
          : undefined,
        client.generationMs,
      )

  return {
    index: input.index,
    status: 'ok',
    error: null,
    ttftMs,
    totalWallClockMs,
    perceivedTps: tps,
    interTokenLatencyMs,
    completionTokens,
    source: {
      ttft: ttftSource,
      totalWallClock: wallSource,
      tps: tpsSource,
    },
  }
}

// ---------------------------------------------------------------------------
// Aggregation (P50/P90/P99/mean) over a set of samples.
// ---------------------------------------------------------------------------

export type MetricSummary = Readonly<{
  p50: number | null
  p90: number | null
  p99: number | null
  mean: number | null
  // MEASURED samples behind this summary (read confidence).
  sampleCount: number
}>

// Collect only the MEASURED values of a field across OK samples (drop the honest
// `not_measured` sentinel so percentiles are never polluted by a coerced 0).
const measuredField = (
  samples: ReadonlyArray<SampleResult>,
  pick: (sample: SampleResult) => MeasuredNumber,
): ReadonlyArray<number> => {
  const out: Array<number> = []
  for (const sample of samples) {
    if (sample.status !== 'ok') {
      continue
    }
    const value = pick(sample)
    if (isMeasured(value)) {
      out.push(value)
    }
  }
  return out
}

const summarize = (values: ReadonlyArray<number>): MetricSummary => ({
  p50: percentile(values, 50),
  p90: percentile(values, 90),
  p99: percentile(values, 99),
  mean: mean(values),
  sampleCount: values.length,
})

export type SampleAggregate = Readonly<{
  totalSamples: number
  okSamples: number
  failedSamples: number
  ttftMs: MetricSummary
  totalWallClockMs: MetricSummary
  perceivedTps: MetricSummary
  interTokenLatencyMs: MetricSummary
  completionTokens: MetricSummary
  // Sum of perceived TPS across the MEASURED samples — the aggregate throughput
  // figure (book framing: total tokens/sec across in-flight requests). null when
  // no sample measured TPS (honest absence, never a fabricated 0).
  aggregateTps: number | null
}>

// Aggregate a set of reconciled samples. PURE. Same samples -> same aggregate.
export const aggregateSamples = (
  samples: ReadonlyArray<SampleResult>,
): SampleAggregate => {
  const ok = samples.filter(sample => sample.status === 'ok')
  const tpsValues = measuredField(samples, sample => sample.perceivedTps)
  return {
    totalSamples: samples.length,
    okSamples: ok.length,
    failedSamples: samples.length - ok.length,
    ttftMs: summarize(measuredField(samples, sample => sample.ttftMs)),
    totalWallClockMs: summarize(
      measuredField(samples, sample => sample.totalWallClockMs),
    ),
    perceivedTps: summarize(tpsValues),
    interTokenLatencyMs: summarize(
      measuredField(samples, sample => sample.interTokenLatencyMs),
    ),
    completionTokens: summarize(
      measuredField(samples, sample => sample.completionTokens),
    ),
    aggregateTps: tpsValues.length === 0 ? null : tpsValues.reduce((s, v) => s + v, 0),
  }
}

// ---------------------------------------------------------------------------
// The streaming-call seam (so the runner is testable offline).
// ---------------------------------------------------------------------------

// One streaming sample call. Implementations measure their own client timing and
// read the server telemetry block; tests inject a deterministic fake. Resolves
// with a SampleResult (failed runs resolve, they do not reject, so a failure in
// one sample never wedges the whole run).
export type SampleStream = (input: {
  index: number
  signal?: AbortSignal
}) => Promise<SampleResult>

// Run N samples at a bounded concurrency. Fires at most `concurrency` in flight
// at a time (clamped to MAX_IN_FLIGHT), preserving sample index in the output.
// PURE w.r.t. scheduling: every sample resolves (failures included). Order of
// the returned array is by sample index, not completion order.
export const runConcurrent = async (input: {
  samples: number
  concurrency: number
  stream: SampleStream
  signal?: AbortSignal
}): Promise<ReadonlyArray<SampleResult>> => {
  const total = Math.max(0, Math.trunc(input.samples))
  const concurrency = clampConcurrency(input.concurrency)
  const results: Array<SampleResult> = new Array(total)
  let next = 0

  const worker = async (): Promise<void> => {
    for (;;) {
      const index = next
      if (index >= total) {
        return
      }
      next += 1
      results[index] = await input.stream({
        index,
        ...(input.signal === undefined ? {} : { signal: input.signal }),
      })
    }
  }

  const lanes = Math.min(concurrency, total)
  const workers: Array<Promise<void>> = []
  for (let i = 0; i < lanes; i += 1) {
    workers.push(worker())
  }
  await Promise.all(workers)
  return results
}

// ---------------------------------------------------------------------------
// Ramp sweep (1 -> 2 -> 4 -> 8).
// ---------------------------------------------------------------------------

export type RampStepResult = Readonly<{
  concurrency: number
  samples: ReadonlyArray<SampleResult>
  aggregate: SampleAggregate
}>

// The concurrency steps a ramp will sweep, given the configured top concurrency.
// Always the prefix of RAMP_STEPS up to and including the clamped top. PURE.
export const rampSweepSteps = (topConcurrency: number): ReadonlyArray<number> => {
  const top = clampConcurrency(topConcurrency)
  return RAMP_STEPS.filter(step => step <= top)
}

// Run a ramp sweep: for each concurrency step, run `samples` samples at that
// concurrency and aggregate. Steps run sequentially (so the box sees one
// concurrency level at a time and the chart reads cleanly). PURE w.r.t.
// scheduling. Returns one RampStepResult per step in ascending concurrency.
export const runRampSweep = async (input: {
  samples: number
  topConcurrency: number
  stream: SampleStream
  signal?: AbortSignal
}): Promise<ReadonlyArray<RampStepResult>> => {
  const steps = rampSweepSteps(input.topConcurrency)
  const out: Array<RampStepResult> = []
  for (const concurrency of steps) {
    const samples = await runConcurrent({
      samples: input.samples,
      concurrency,
      stream: input.stream,
      ...(input.signal === undefined ? {} : { signal: input.signal }),
    })
    out.push({
      concurrency,
      samples,
      aggregate: aggregateSamples(samples),
    })
  }
  return out
}

// ---------------------------------------------------------------------------
// Presentation formatting (honest `not_measured`, never a fake 0).
// ---------------------------------------------------------------------------

// Format a measured number for display. `not_measured` renders as the explicit
// label — NEVER as `0`. A real measured 0 renders as `0`.
export const formatMeasured = (
  value: MeasuredNumber,
  options: { unit?: string; digits?: number } = {},
): string => {
  if (!isMeasured(value)) {
    return 'not measured'
  }
  const digits = options.digits ?? 0
  const text = value.toFixed(digits)
  return options.unit === undefined ? text : `${text} ${options.unit}`
}

// Format a nullable summary number (percentile/mean) — null renders honestly as
// the not-measured label, never 0.
export const formatSummaryNumber = (
  value: number | null,
  options: { unit?: string; digits?: number } = {},
): string =>
  value === null
    ? 'not measured'
    : formatMeasured(value, options)
