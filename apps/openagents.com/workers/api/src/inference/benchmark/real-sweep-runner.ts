// The owner-armed REAL SWEEP runner (Open Question #5 suite / #6307).
//
// Ties the owner-armed transports (`real-lane-executor.ts`) to the existing,
// already-merged benchmark harness (matrix expansion, telemetry-record assembly,
// public-safe report) to produce the FIRST `decisionGrade: true`
// Khala-vs-Fireworks/Vertex report.
//
// The sync `runBenchmark` path drives a SYNC seam (the fixture lane). A real sweep
// must AWAIT live provider IO per cell, so this runner reimplements the same
// deterministic matrix walk as `runBenchmark` but `await`s each lane transport,
// then hands the assembled run set to the SAME `buildBenchmarkReport`. The report
// math, public-safety tripwire, and decision-grade rule are unchanged — only the
// sample SOURCE differs (real provider responses instead of fixture arithmetic).
//
// SPEND / HONESTY DISCIPLINE:
//   - This runner refuses to start unless `preflightRealBenchmarkSweep` says the
//     sweep can be armed (owner confirmation, approval ref, budget cap, billable
//     sample cap, realistic-traffic evidence). It NEVER arms itself.
//   - A lane with no owner-armed transport is recorded as a SKIPPED run (honest
//     absence), exactly like a not-yet-available lane — never a fabricated number.
//   - The Khala lane (and any no-cost/local lane) can run with only the public
//     Khala transport; the Fireworks/Vertex lanes stay skipped until the owner
//     supplies their credentialed transports. So the Khala-side report is
//     producible NOW; the full cross-provider decision-grade report needs the
//     owner-armed spendful transports.
//
// WORKER-SAFE: no Worker bindings; the only IO is through the injected transports.
import {
  type KhalaTelemetryRecord,
  buildKhalaTelemetryRecord,
} from '../khala-telemetry'
import type { ServedTokensRequestAttribution } from '../served-tokens-recorder'
import type { BenchmarkLaneSample } from './lane-seam'
import type { BenchmarkCell, BenchmarkLane, BenchmarkMatrixConfig } from './matrix'
import { expandMatrix } from './matrix'
import { modelIdForBenchmarkCell } from './opencode-client-runner'
import {
  type RealLaneTransport,
  type RealLaneVerdictResolver,
  benchmarkRealSweepAttribution,
  realLaneSampleFromHttpResult,
  resolveRealLaneVerdict,
} from './real-lane-executor'
import {
  type RealSweepPreflight,
  type RealSweepPreflightOptions,
  preflightRealBenchmarkSweep,
} from './real-sweep-plan'
import { type BenchmarkRun, type BenchmarkRunSet } from './runner'

// Raised when a real sweep is started without a green preflight. The runner NEVER
// spends without the preflight clearing every owner gate first.
export class RealSweepNotArmedError extends Error {
  readonly _tag = 'RealSweepNotArmedError'
  constructor(readonly preflight: RealSweepPreflight) {
    super(
      'Real benchmark sweep cannot start: preflight did not clear the owner ' +
        'arming gate. Blockers: ' +
        preflight.blockers.map(b => b.code).join(', '),
    )
    this.name = 'RealSweepNotArmedError'
  }
}

export type RunRealSweepOptions = Readonly<{
  config: BenchmarkMatrixConfig
  // The owner-arming inputs the preflight validates (confirmation, approval ref,
  // budget cap, billable sample cap, realistic-traffic evidence).
  preflight: RealSweepPreflightOptions
  // The owner-armed transports. The Khala (and any no-cost) transport may be
  // present while the spendful Fireworks/Vertex transports are absent; absent
  // lanes are skipped, not fabricated.
  transports: ReadonlyArray<RealLaneTransport>
  // Optional resolver of real executed verdicts for verified workloads.
  verdictResolver?: RealLaneVerdictResolver | undefined
}>

const buildRunRequestId = (
  configId: string,
  cellId: string,
  sampleIndex: number,
): string => `real-sweep:${configId}:${cellId}:s${sampleIndex}`

const skippedRun = (
  cell: BenchmarkCell,
  sampleIndex: number,
  reason: string,
): BenchmarkRun => ({
  cellId: cell.cellId,
  cell,
  sampleIndex,
  record: null,
  clientSurface: null,
  skippedReason: reason,
})

const recordForSample = (
  config: BenchmarkMatrixConfig,
  cell: BenchmarkCell,
  sampleIndex: number,
  sample: BenchmarkLaneSample,
): KhalaTelemetryRecord => {
  const modelId = modelIdForBenchmarkCell(cell)
  const requestClass =
    cell.transport === 'batch'
      ? 'batch'
      : cell.workload === 'verifier-run'
        ? 'verifier_run'
        : 'interactive_stream'
  return buildKhalaTelemetryRecord({
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
    ...(sample.speculation === undefined
      ? {}
      : { speculation: sample.speculation }),
    verificationClass: sample.verificationClass,
    executedVerdict: sample.executedVerdict,
    scalarReward: sample.scalarReward,
    costBasisMsat: sample.costBasisMsat,
    priceMsat: sample.priceMsat,
    economicsState: sample.economicsState,
    settlementState: 'not_applicable',
  })
}

// The Khala lane carries the internal benchmark-sweep attribution so its own
// inference is segmented (#6298); third-party lanes carry no Khala attribution.
const attributionForLane = (
  lane: BenchmarkLane,
): ServedTokensRequestAttribution | null =>
  lane === 'khala' ? benchmarkRealSweepAttribution() : null

// Run the owner-armed real sweep. AWAITs each lane transport per cell, assembles
// the canonical telemetry run set, and returns it. The caller builds the report
// with the existing `buildBenchmarkReport`. Refuses to start without a green
// preflight (so it can never spend unarmed).
export const runRealSweep = async (
  options: RunRealSweepOptions,
): Promise<BenchmarkRunSet> => {
  const armedBillableLanes = options.transports
    .filter(transport => transport.billable)
    .map(transport => transport.lane)
  const preflight = preflightRealBenchmarkSweep(
    options.config,
    options.preflight.billableLanes === undefined
      ? { ...options.preflight, billableLanes: armedBillableLanes }
      : options.preflight,
  )
  if (!preflight.canArmRealSeam) {
    throw new RealSweepNotArmedError(preflight)
  }

  const transportByLane = new Map<BenchmarkLane, RealLaneTransport>()
  for (const transport of options.transports) {
    transportByLane.set(transport.lane, transport)
  }

  const cells = expandMatrix(options.config)
  const runs: Array<BenchmarkRun> = []
  let cellsExecuted = 0
  let cellsSkipped = 0
  let anyBillable = false

  for (const cell of cells) {
    // Never-available future lanes are always skipped.
    if (cell.laneAvailability === 'not_yet_available') {
      cellsSkipped += 1
      runs.push(skippedRun(cell, 0, `lane_not_yet_available:${cell.lane}`))
      continue
    }
    const transport = transportByLane.get(cell.lane)
    if (transport === undefined) {
      // No owner-armed transport for this lane: honest skip, never fabricated.
      cellsSkipped += 1
      runs.push(
        skippedRun(cell, 0, `real_transport_not_armed:${cell.lane}`),
      )
      continue
    }
    if (transport.billable) {
      anyBillable = true
    }
    cellsExecuted += 1
    const samples = Math.max(1, Math.floor(cell.samplesPerCell))
    const attribution = attributionForLane(cell.lane)
    for (let sampleIndex = 0; sampleIndex < samples; sampleIndex += 1) {
      const httpResult = await transport.execute(cell, sampleIndex, attribution)
      const verdict = resolveRealLaneVerdict(
        cell,
        sampleIndex,
        options.verdictResolver,
      )
      const sample = realLaneSampleFromHttpResult(
        cell,
        httpResult,
        verdict,
        transport.billable,
      )
      runs.push({
        cellId: cell.cellId,
        cell,
        sampleIndex,
        record: recordForSample(options.config, cell, sampleIndex, sample),
        clientSurface: null,
        skippedReason: null,
      })
    }
  }

  return {
    configId: options.config.id,
    // The seam is the owner-armed real lane. `seamCanSpend` must reflect whether
    // any executed lane was actually billable, so a Khala-only run (no spendful
    // provider armed) is NOT mislabeled as a spend sweep. The report's
    // decision-grade rule keys on `seamCanSpend`, so a Khala-only run stays
    // decision-grade ONLY when a billable comparator also ran (see the runner
    // contract below).
    seamId: 'real',
    seamCanSpend: anyBillable,
    runs,
    cellsExpanded: cells.length,
    cellsExecuted,
    cellsSkipped,
  }
}
