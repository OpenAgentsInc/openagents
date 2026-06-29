// Pure, IO-free comparison model for the `/trace/compare/{ids}` view (#6211).
//
// Given an ordered list of trace uuids, this resolves each to its ATIF
// trajectory (via an injected lookup so tests + the eventual read-API wiring can
// supply their own source) and derives the per-trace metrics the comparison
// table shows: verdict, latency (duration), step count, cost — plus the deltas
// of every variant relative to the BASELINE (the first id), mirroring the
// qa-runner chill-eval comparison math (`apps/qa-runner/src/evals.ts`).
//
// HONEST: a metric a trajectory does not carry is the `not_measured` sentinel,
// never a fabricated 0 — exactly the contract the eval runner uses. A delta is
// `not_measured` whenever either side is unmeasured. An unknown uuid resolves to
// an explicit `found: false` cell so the view renders it as "unknown id" rather
// than inventing numbers.

import type { Trajectory } from '../trace/atif'
import {
  type TraceVerdict,
  type VerdictTone,
  agentSteps,
  traceDurationMs,
  traceTarget,
  traceVerdict,
  verdictLabel,
  verdictTone,
} from '../trace/atif'

// A measured number OR the honest "we did not / could not measure this" marker
// (mirrors the qa-runner eval honesty contract: a value we never measured is the
// sentinel, never 0).
export type MeasuredNumber = number | 'not_measured'

export const isMeasured = (value: MeasuredNumber): value is number =>
  value !== 'not_measured'

const measuredOr = (value: number | undefined): MeasuredNumber =>
  value === undefined ? 'not_measured' : value

const subtractMeasured = (
  a: MeasuredNumber,
  b: MeasuredNumber,
): MeasuredNumber =>
  isMeasured(a) && isMeasured(b) ? a - b : 'not_measured'

// One resolved trace in the comparison. A `found: false` entry is an honest
// unknown-id cell — the uuid did not resolve to any stored trajectory.
export type CompareTrace =
  | {
      readonly found: true
      readonly uuid: string
      readonly isBaseline: boolean
      readonly verdict: TraceVerdict
      readonly verdictLabel: string
      readonly verdictTone: VerdictTone
      readonly pass: boolean
      /** Target name (e.g. `openagents.com`), if the trajectory carries one. */
      readonly target: string | undefined
      readonly model: string
      readonly agentName: string
      readonly durationMs: MeasuredNumber
      readonly stepCount: number
      readonly costUsd: MeasuredNumber
    }
  | {
      readonly found: false
      readonly uuid: string
      readonly isBaseline: boolean
    }

// A variant's behavior deltas relative to the baseline (the first found trace).
// Positive `passDelta` means this variant passed where the baseline did not (or
// vice-versa); negative `durationDeltaMs` means faster than the baseline.
export interface CompareDelta {
  readonly uuid: string
  /** +1 this passed & baseline did not; -1 the reverse; 0 same; or unmeasured. */
  readonly passDelta: MeasuredNumber
  readonly durationDeltaMs: MeasuredNumber
  readonly stepCountDelta: MeasuredNumber
  readonly costDeltaUsd: MeasuredNumber
}

export interface Comparison {
  readonly traces: ReadonlyArray<CompareTrace>
  /** uuid -> delta vs baseline, for every trace (the baseline's own deltas are 0). */
  readonly deltas: ReadonlyArray<CompareDelta>
  /** The baseline uuid (the first FOUND trace), if any trace resolved. */
  readonly baselineUuid: string | undefined
  readonly foundCount: number
  readonly unknownCount: number
}

const deriveTrace = (
  uuid: string,
  isBaseline: boolean,
  trajectory: Trajectory | undefined,
): CompareTrace => {
  if (trajectory === undefined) return { found: false, uuid, isBaseline }
  const verdict = traceVerdict(trajectory)
  const target = traceTarget(trajectory)
  const model =
    trajectory.agent.model_name ?? trajectory.steps[1]?.model_name ?? 'unknown'
  return {
    found: true,
    uuid,
    isBaseline,
    verdict,
    verdictLabel: verdictLabel(verdict),
    verdictTone: verdictTone(verdict),
    pass: verdict === 'PASS',
    target: target?.name,
    model,
    agentName: trajectory.agent.name,
    durationMs: measuredOr(traceDurationMs(trajectory)),
    // The step count is the number of AGENT steps (the timeline nodes), matching
    // what the single-trace page renders — not the raw `total_steps` (which
    // includes the user goal). Honest + consistent with `/trace/{uuid}`.
    stepCount: agentSteps(trajectory).length,
    costUsd: measuredOr(trajectory.final_metrics?.total_cost_usd),
  }
}

// The baseline is the first FOUND trace. Unknown leading ids do not become the
// baseline (you cannot compute deltas against an unknown).
const findBaseline = (
  traces: ReadonlyArray<CompareTrace>,
): Extract<CompareTrace, { found: true }> | undefined => {
  for (const t of traces) if (t.found) return t
  return undefined
}

const passScore = (pass: boolean): number => (pass ? 1 : 0)

// Build the full comparison from an ordered uuid list + a lookup. The lookup is
// injected so this stays pure and testable; the page supplies the committed
// sample lookup, and the read-API wiring will supply a fetch-backed one.
export const buildComparison = (
  uuids: ReadonlyArray<string>,
  lookup: (uuid: string) => Trajectory | undefined,
): Comparison => {
  const traces = uuids.map((uuid, index) =>
    deriveTrace(uuid, index === 0, lookup(uuid)),
  )
  const baseline = findBaseline(traces)

  const deltas: ReadonlyArray<CompareDelta> = traces.map(t => {
    if (!t.found || baseline === undefined || t.uuid === baseline.uuid) {
      return {
        uuid: t.uuid,
        passDelta: t.found && baseline !== undefined ? 0 : 'not_measured',
        durationDeltaMs:
          t.found && baseline !== undefined ? 0 : 'not_measured',
        stepCountDelta: t.found && baseline !== undefined ? 0 : 'not_measured',
        costDeltaUsd: t.found && baseline !== undefined ? 0 : 'not_measured',
      }
    }
    return {
      uuid: t.uuid,
      passDelta: passScore(t.pass) - passScore(baseline.pass),
      durationDeltaMs: subtractMeasured(t.durationMs, baseline.durationMs),
      stepCountDelta: t.stepCount - baseline.stepCount,
      costDeltaUsd: subtractMeasured(t.costUsd, baseline.costUsd),
    }
  })

  return {
    traces,
    deltas,
    baselineUuid: baseline?.uuid,
    foundCount: traces.filter(t => t.found).length,
    unknownCount: traces.filter(t => !t.found).length,
  }
}

// Parse the `ids` path component (`a,b,c`) into a clean, de-duplicated,
// order-preserving uuid list. Tolerates `+`/whitespace separators and stray
// empties so a hand-typed or copy-pasted set still resolves.
export const parseCompareIds = (raw: string): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const out: string[] = []
  for (const part of raw.split(/[,+\s]+/)) {
    const id = part.trim()
    if (id.length === 0 || seen.has(id)) continue
    seen.add(id)
    out.push(id)
  }
  return out
}
