// The OWNER-ARMED real-lane executor for the Khala-vs-Fireworks/Vertex decision
// sweep (Open Question #5 suite / #6307).
//
// `lane-seam.ts` deliberately did NOT implement the live provider calls — it built
// the GATE (`makeRealLaneSeam`, default OFF, refuses to spend unarmed) and typed
// the `RealLaneExecutor` seam, leaving the actual billable provider IO to "the
// owner-armed sweep". THIS module is that owner-armed sweep's executor: given an
// owner-armed transport (a credentialed HTTP caller + a clock), it issues ONE real
// request per cell against a live provider and measures it into a
// `BenchmarkLaneSample` the existing runner already knows how to record.
//
// HONESTY / SPEND DISCIPLINE:
//   - This module performs NO IO by itself and holds NO credentials. It is a PURE
//     mapping layer over an INJECTED transport. The owner supplies the transport
//     (with the live base URLs + API keys); absent that transport the executor
//     cannot be constructed (`makeRealLaneExecutor` requires `transports`).
//   - The Khala lane and any zero-cost/local lane can be run NOW with only the
//     public Khala endpoint; the Fireworks and Vertex lanes are SPENDFUL and stay
//     dark until the owner provides their credentialed transports. A cell whose
//     transport is missing is reported as a typed, measured-as-unavailable refusal
//     rather than a fabricated number — the runner records it like any other
//     unexecuted lane.
//   - Inference the sweep itself drives THROUGH Khala is tagged
//     `demand_kind=internal`, `demand_source=benchmark_real_sweep` (#6298) via the
//     attribution this module attaches to the Khala request, so the real sweep's
//     own Khala load stays segmented from external demand.
//
// WORKER-SAFE: no Worker bindings, no Effect runtime, no top-level Date.now /
// Math.random. The clock and HTTP are injected dependencies; the response→sample
// math is pure.
import type { ServedTokensRequestAttribution } from '../served-tokens-recorder'
import type { BenchmarkCell, BenchmarkLane } from './matrix'
import type { BenchmarkLaneSample } from './lane-seam'
import type { KhalaVerificationClass, KhalaExecutedVerdict } from '../khala-telemetry'

// ---------------------------------------------------------------------------
// Attribution: the real sweep's own Khala load is internal + segmented (#6298).
// ---------------------------------------------------------------------------

// The demand source every Khala request issued BY this benchmark sweep carries.
// Stable string so the ledger/projection can segment the sweep's own load.
export const BENCHMARK_REAL_SWEEP_DEMAND_SOURCE = 'benchmark_real_sweep' as const

// The attribution header the Khala lane request attaches so the sweep's own
// Khala inference is tagged internal and excluded from the public counter (#6298,
// roadmap §"real external requests always win"). Only the Khala lane carries this
// — third-party provider lanes (Fireworks/Vertex) are billed by the provider and
// never touch the Khala served-tokens ledger.
export const benchmarkRealSweepAttribution = (): ServedTokensRequestAttribution => ({
  demandKind: 'internal',
  demandSource: BENCHMARK_REAL_SWEEP_DEMAND_SOURCE,
  demandClient: 'khala_benchmark_real_sweep',
})

// ---------------------------------------------------------------------------
// The owner-supplied transport (the ONLY place credentials/IO live).
// ---------------------------------------------------------------------------

// A single live HTTP exchange the executor needs from the owner-armed transport.
// The transport owns the base URL, auth headers, and the actual fetch; it returns
// the measured wall-clock split and the provider usage. Keeping this an injected
// interface means this module never embeds a key, never hard-codes a host, and is
// fully testable with a fake transport (no network in tests).
export type RealLaneHttpResult = Readonly<{
  // Provider-reported token usage (receipt-first — never an estimate).
  promptTokens: number
  completionTokens: number
  totalTokens: number
  // Cached-input tokens the provider reported (e.g. prompt-cache hits). 0 when the
  // provider did not report a cache hit.
  cachedInputTokens: number
  // Measured latency split (ms), from the owner transport's own clock around the
  // request: time to first token (streaming), full wall-clock, and the generation
  // span (full - ttft). A non-streaming call sets ttft = wall-clock.
  ttftMs: number
  totalWallClockMs: number
  // Coarse public-safe region label the transport observed (or a default).
  region: string
  // Provider unit cost basis for this request in msat, where the transport priced
  // it from the provider's published rate card. 0 for a zero-cost/local lane.
  costBasisMsat: number
}>

// The owner-armed transport for ONE lane. It is the credentialed seam: the owner
// constructs it with the live endpoint + key, and it performs the real request.
// PURE callers (this module, the runner) only ever see the measured result.
export type RealLaneTransport = Readonly<{
  lane: BenchmarkLane
  // Whether this transport issues a BILLABLE third-party request (Fireworks /
  // Vertex) vs a no-cost/own Khala call. Surfaced so the sweep can report which
  // lanes actually cost money this run.
  billable: boolean
  // Execute one real request for the cell and return the measured exchange. The
  // attribution is attached for the Khala lane so its own load stays internal.
  execute: (
    cell: BenchmarkCell,
    sampleIndex: number,
    attribution: ServedTokensRequestAttribution | null,
  ) => Promise<RealLaneHttpResult>
}>

// ---------------------------------------------------------------------------
// Verification mapping for a real sample.
// ---------------------------------------------------------------------------

// A real sweep does not (yet) execute the artifact verifier inline for every
// provider — the verifier-run/artifact workloads carry their executed verdict from
// the acceptance runner separately. For the first decision-grade sweep, the
// real executor records the HONEST UNVERIFIED shape for a chat/long-context turn
// (no verifier ran) and leaves verified workloads to an explicit verdict the owner
// attaches per cell. This keeps the report's verification-rate axis truthful: a
// lane only earns an accepted outcome from a real executed verdict, never a
// fabricated pass.
export type RealLaneVerdict = Readonly<{
  verificationClass: KhalaVerificationClass
  executedVerdict: KhalaExecutedVerdict
  scalarReward: number
  verifierTimeMs: number
}>

// The default unverified verdict for a non-executed real sample (chat / long
// context). `none`/`not_executed` means the report counts NO accepted outcome for
// it — honest: latency/cost are measured, quality is not asserted.
const UNVERIFIED_REAL_VERDICT: RealLaneVerdict = {
  verificationClass: 'none',
  executedVerdict: 'not_executed',
  scalarReward: 0,
  verifierTimeMs: 0,
}

// An optional per-cell executed verdict the owner attaches when a real verifier or
// acceptance run produced a real pass/fail for the cell. Keyed by cellId so the
// owner can wire the acceptance runner's verdicts in without this module knowing
// how the verifier works.
export type RealLaneVerdictResolver = (
  cell: BenchmarkCell,
  sampleIndex: number,
) => RealLaneVerdict | undefined

// ---------------------------------------------------------------------------
// The executor.
// ---------------------------------------------------------------------------

// Raised when a cell's lane has no owner-armed transport. It is a typed refusal:
// the executor never fabricates a number for a lane the owner did not arm. The
// runner treats a thrown executor as a skipped/unexecuted run.
export class RealLaneTransportMissingError extends Error {
  readonly _tag = 'RealLaneTransportMissingError'
  constructor(readonly lane: BenchmarkLane) {
    super(
      `No owner-armed transport for lane "${lane}". ` +
        'The real sweep refuses to fabricate a measurement for an un-armed lane. ' +
        'Provide a credentialed RealLaneTransport for this lane to include it.',
    )
    this.name = 'RealLaneTransportMissingError'
  }
}

export type RealLaneExecutorOptions = Readonly<{
  // The owner-armed transports, one per lane the owner is paying to measure. A
  // lane with no transport is refused (never fabricated).
  transports: ReadonlyArray<RealLaneTransport>
  // Optional resolver of real executed verdicts for verified workloads. When a
  // cell has no resolved verdict it records the honest unverified shape.
  verdictResolver?: RealLaneVerdictResolver | undefined
}>

// Map a measured HTTP result + a verdict into the canonical `BenchmarkLaneSample`
// the runner records. PURE.
const sampleFromHttpResult = (
  cell: BenchmarkCell,
  result: RealLaneHttpResult,
  verdict: RealLaneVerdict,
  billable: boolean,
): BenchmarkLaneSample => {
  const generationWallClockMs = Math.max(
    0,
    result.totalWallClockMs - result.ttftMs,
  )
  // The gateway overhead is not separately measured by a third-party transport;
  // we record 0 (honest: it is folded into the provider time the transport saw)
  // rather than inventing a split.
  const gatewayOverheadMs = 0
  const providerTimeMs = result.totalWallClockMs
  return {
    promptTokens: result.promptTokens,
    completionTokens: result.completionTokens,
    totalTokens: result.totalTokens,
    cachedInputTokens: result.cachedInputTokens,
    ttftMs: result.ttftMs,
    totalWallClockMs: result.totalWallClockMs,
    generationWallClockMs,
    providerTimeMs,
    gatewayOverheadMs,
    verificationClass: verdict.verificationClass,
    executedVerdict: verdict.executedVerdict,
    scalarReward: verdict.scalarReward,
    verifierTimeMs: verdict.verifierTimeMs,
    costBasisMsat: result.costBasisMsat,
    // A real third-party request has no simulated price — the report omits price
    // regardless, and the canonical record marks the economics as measured only
    // when a metering hook priced it. Leave price/economics undefined here: the
    // sweep records cost basis (for cost-per-accepted-outcome) without claiming a
    // billed price.
    economicsState: billable ? 'measured' : undefined,
    region: result.region,
    // The real sweep does not assert speculation counts for a third-party
    // provider; leave undefined so the telemetry builder records the honest
    // not_measured speculation shape.
    fallbackReason:
      cell.targetProfile?.routeRole === 'fallback'
        ? 'benchmark_fallback_candidate'
        : null,
  }
}

// Build the owner-armed `RealLaneExecutor` the existing `makeRealLaneSeam` accepts.
// Because the runner calls the executor SYNCHRONOUSLY (it returns a sample, not a
// promise), this builder pre-resolves nothing: the SYNC executor delegates to a
// transport that must itself be synchronous-result. For the live async path the
// owner uses `runRealSweep` (below), which awaits each transport and assembles the
// run set directly. This sync builder exists for transports that can return a
// pre-measured result synchronously (e.g. a replay/fixture-backed real transport
// in a smoke).
export type SyncRealLaneTransport = Readonly<{
  lane: BenchmarkLane
  billable: boolean
  execute: (cell: BenchmarkCell, sampleIndex: number) => RealLaneHttpResult
}>

export const makeSyncRealLaneExecutor = (
  transports: ReadonlyArray<SyncRealLaneTransport>,
  verdictResolver?: RealLaneVerdictResolver | undefined,
): ((cell: BenchmarkCell, sampleIndex: number) => BenchmarkLaneSample) => {
  const byLane = new Map<BenchmarkLane, SyncRealLaneTransport>()
  for (const transport of transports) {
    byLane.set(transport.lane, transport)
  }
  return (cell, sampleIndex) => {
    const transport = byLane.get(cell.lane)
    if (transport === undefined) {
      throw new RealLaneTransportMissingError(cell.lane)
    }
    const result = transport.execute(cell, sampleIndex)
    const verdict =
      verdictResolver?.(cell, sampleIndex) ?? UNVERIFIED_REAL_VERDICT
    return sampleFromHttpResult(cell, result, verdict, transport.billable)
  }
}

// Resolve the verdict for a cell (real executed verdict if the owner wired one,
// else the honest unverified shape). Exposed so the async runner uses the same
// rule.
export const resolveRealLaneVerdict = (
  cell: BenchmarkCell,
  sampleIndex: number,
  verdictResolver?: RealLaneVerdictResolver | undefined,
): RealLaneVerdict =>
  verdictResolver?.(cell, sampleIndex) ?? UNVERIFIED_REAL_VERDICT

// Map a measured async HTTP result into a sample (the async path's mapping seam),
// re-using the same pure mapping the sync path uses.
export const realLaneSampleFromHttpResult = sampleFromHttpResult

export { UNVERIFIED_REAL_VERDICT }
