// The benchmark RUNNER (book P1-5 / #6088).
//
// Executes a matrix config cell-by-cell against a pluggable `BenchmarkLaneSeam`,
// and records the P0-1 telemetry RECORD per sample by feeding the seam's measured
// sample straight into the canonical `buildKhalaTelemetryRecord`. The runner
// REUSES the telemetry schema — it never forks a parallel metric vocabulary — so
// a benchmark sample and a production request are described by the exact same
// `openagents.khala.telemetry.v1` record. That is the whole point: the benchmark
// measures the SAME lifecycle fields the gateway records in production.
//
// PURE/DETERMINISTIC with the fixture seam: same config + same seam → same runs.
// No clock, no randomness, no network. The runner itself issues no IO; it only
// drives the seam and assembles telemetry records. (A real, billable sweep is
// achieved by passing the owner-armed real seam — gated in `lane-seam.ts`.)
import {
  type KhalaTelemetryRecord,
  buildKhalaTelemetryRecord,
} from '../khala-telemetry'
import type {
  BenchmarkClientSurfaceSample,
  BenchmarkLaneSeam,
} from './lane-seam'
import type { BenchmarkCell, BenchmarkMatrixConfig } from './matrix'
import { expandMatrix } from './matrix'
import { modelIdForBenchmarkCell } from './opencode-client-runner'

// One executed sample of one cell: the cell context + the canonical telemetry
// record. A not-yet-available lane produces NO telemetry record (it was never
// executed) — `record` is null and `skippedReason` says why.
export type BenchmarkRun = Readonly<{
  cellId: string
  cell: BenchmarkCell
  sampleIndex: number
  // The canonical lifecycle record, or null when the cell was skipped (a
  // not-yet-available lane is never executed — honest, not a fabricated zero).
  record: KhalaTelemetryRecord | null
  // Public-safe client-surface metrics that are not part of provider telemetry,
  // such as OpenCode tool-call success counts. Null for direct API workloads or
  // skipped cells.
  clientSurface: BenchmarkClientSurfaceSample | null
  // Why a run was skipped; null when it executed.
  skippedReason: string | null
}>

// The full result of running a matrix config against a seam.
export type BenchmarkRunSet = Readonly<{
  configId: string
  // The seam that produced these runs ("fixture" | "real").
  seamId: string
  // Whether the seam that ran these was capable of real spend. The report
  // surfaces this so a reader knows whether numbers are illustrative or real.
  seamCanSpend: boolean
  // Every (cell × sample) run, in deterministic matrix order.
  runs: ReadonlyArray<BenchmarkRun>
  // Cell-level coverage: how many cells expanded, executed, and were skipped.
  cellsExpanded: number
  cellsExecuted: number
  cellsSkipped: number
}>

// Build a stable request id for a benchmark run. PURE (no UUID/clock): it encodes
// the config, cell, and sample index so the same run always gets the same id —
// reproducible and auditable, never a random handle.
const buildRunRequestId = (
  configId: string,
  cellId: string,
  sampleIndex: number,
): string => `bench:${configId}:${cellId}:s${sampleIndex}`

const skippedReasonForCell = (
  cell: BenchmarkCell,
  seam: BenchmarkLaneSeam,
): string | null => {
  if (cell.laneAvailability === 'not_yet_available') {
    return `lane_not_yet_available:${cell.lane}`
  }
  if (
    cell.laneAvailability === 'fixture_only' &&
    seam.canSpend &&
    seam.canExecuteFixtureOnlyLane?.(cell.lane) !== true
  ) {
    return `lane_fixture_only:${cell.lane}`
  }
  return null
}

// Run a single sample of a cell against the seam and assemble its telemetry
// record. A not-yet-available lane is NEVER executed against any seam — it yields
// a skipped run (honest absence), because there is no real path to measure and we
// refuse to fabricate one.
const runSample = (
  config: BenchmarkMatrixConfig,
  cell: BenchmarkCell,
  sampleIndex: number,
  seam: BenchmarkLaneSeam,
): BenchmarkRun => {
  const skippedReason = skippedReasonForCell(cell, seam)
  if (skippedReason !== null) {
    return {
      cellId: cell.cellId,
      cell,
      sampleIndex,
      record: null,
      clientSurface: null,
      skippedReason,
    }
  }

  const sample = seam.sample(cell, sampleIndex)
  const modelId = modelIdForBenchmarkCell(cell)
  const requestClass =
    cell.transport === 'batch'
      ? 'batch'
      : cell.workload === 'verifier-run'
        ? 'verifier_run'
        : 'interactive_stream'

  const record = buildKhalaTelemetryRecord({
    requestId: buildRunRequestId(config.id, cell.cellId, sampleIndex),
    requestedModel: modelId,
    servedModel: modelId,
    route: cell.workload,
    provider: cell.lane,
    requestClass,
    promptTokens: sample.promptTokens,
    completionTokens: sample.completionTokens,
    totalTokens: sample.totalTokens,
    cachedInputTokens: sample.cachedInputTokens,
    ttftMs: cell.transport === 'streaming' ? sample.ttftMs : undefined,
    totalWallClockMs: sample.totalWallClockMs,
    generationWallClockMs: sample.generationWallClockMs,
    providerTimeMs: sample.providerTimeMs,
    gatewayOverheadMs: sample.gatewayOverheadMs,
    queueWaitMs: sample.queueWaitMs,
    batchWaitMs: sample.batchWaitMs,
    verifierTimeMs:
      sample.verifierTimeMs > 0 ? sample.verifierTimeMs : undefined,
    region: sample.region,
    fallbackReason: sample.fallbackReason,
    // Speculation disclosure (book P1-8 / #6091): the seam's per-sample
    // speculation outcome (mode + draft counts) flows into the canonical record
    // so a benchmark sample discloses speculation the same way a production
    // request will. Absent => the builder records the honest-unknown shape.
    ...(sample.speculation === undefined
      ? {}
      : { speculation: sample.speculation }),
    verificationClass: sample.verificationClass,
    executedVerdict: sample.executedVerdict,
    scalarReward: sample.scalarReward,
    costBasisMsat: sample.costBasisMsat,
    priceMsat: sample.priceMsat,
    economicsState: sample.economicsState,
    // The benchmark records price only when the seam has a measured/simulated
    // metering value. The public report still omits raw price and margin.
    settlementState: 'not_applicable',
  })

  return {
    cellId: cell.cellId,
    cell,
    sampleIndex,
    record,
    clientSurface: sample.clientSurface ?? null,
    skippedReason: null,
  }
}

// Run an entire matrix config against a seam. Expands the matrix deterministically,
// then runs `samplesPerCell` samples of each executable cell (one skipped run for
// each unavailable cell). PURE with the fixture seam.
export const runBenchmark = (
  config: BenchmarkMatrixConfig,
  seam: BenchmarkLaneSeam,
): BenchmarkRunSet => {
  const cells = expandMatrix(config)
  const runs: Array<BenchmarkRun> = []
  let cellsExecuted = 0
  let cellsSkipped = 0

  for (const cell of cells) {
    if (skippedReasonForCell(cell, seam) !== null) {
      cellsSkipped += 1
      runs.push(runSample(config, cell, 0, seam))
      continue
    }
    cellsExecuted += 1
    const samples = Math.max(1, Math.floor(cell.samplesPerCell))
    for (let sampleIndex = 0; sampleIndex < samples; sampleIndex += 1) {
      runs.push(runSample(config, cell, sampleIndex, seam))
    }
  }

  return {
    configId: config.id,
    seamId: seam.id,
    seamCanSpend: seam.canSpend,
    runs,
    cellsExpanded: cells.length,
    cellsExecuted,
    cellsSkipped,
  }
}
