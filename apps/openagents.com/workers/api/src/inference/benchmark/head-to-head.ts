// External head-to-head: Khala vs the tools/models a developer would otherwise
// reach for, as a recurring published quality bar (#6308, epic #6303 Khala GTM).
//
// GTM doc §4 "External head-to-head comparisons": generalize the supply-lane
// decision sweep (#6307) and the gym ladder (#6309) into the comparison a
// DEVELOPER actually cares about — "is Khala better than the thing I'd otherwise
// use?" — on identical prompts, scored on BOTH headline axes:
//   - SOLVE RATE (verified-rate: executed-passed / executed-attempted), and
//   - COST-PER-ACCEPTED-OUTCOME (the only cost metric that respects verification:
//     a cheap lane that fails is not cheap).
// published as a RECURRING, dereferenceable quality bar (re-run as Khala changes),
// NOT a one-off.
//
// This module owns ONLY the PUBLICATION layer on top of the already-shipped
// benchmark harness (matrix, runner, report) and the gym leaderboard projection.
// It does NOT re-implement the matrix, the cost math, the verification math, the
// real-sweep owner-arm gate, or the public-safety boundary — it consumes them. It
// is PURE and framework-agnostic: no Worker, no network, no clock. The route +
// store (`head-to-head-routes.ts`, `head-to-head-store.ts`) publish what this
// builds; the owner-armed real sweep (`real-sweep-runner.ts`) produces the
// decision-grade reports it consumes.
//
// HONESTY: a comparator entry is only ever produced from a `decisionGrade: true`,
// public-safety-checked report over REALISTIC traffic — the same bar the gym
// leaderboard enforces. A comparator the owner has not yet armed (needs paid API
// keys + spend approval) is shown as `awaiting_owner` with its owner-gate ref and
// the NEEDS-OWNER arming requirement — never a fabricated number. Khala always
// runs at no third-party cost, so the Khala side is producible now; the verdict
// against each comparator stays `awaiting_owner` until that comparator's spendful
// run is armed.
import { Schema as S } from 'effect'

import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import {
  buildGymLeaderboardProjection,
  type GymLeaderboardExcludedReport,
  type GymLeaderboardReportInput,
  type GymLeaderboardRow,
} from '../gym/leaderboard'
import { type BenchmarkLane, laneAvailability } from './matrix'

export const KhalaHeadToHeadSchemaVersion = 'openagents.khala.head_to_head.v1'

// How a comparator entry was produced. Only `published` carries decision-grade
// numbers from an owner-armed real run; `awaiting_owner` shows the gate.
export const KhalaHeadToHeadComparatorState = S.Literals([
  'published',
  'awaiting_owner',
])
export type KhalaHeadToHeadComparatorState =
  typeof KhalaHeadToHeadComparatorState.Type

// The recurring cadence at which the quality bar is re-run + re-published. It is a
// RECURRING bar (the issue's "not a one-off"): re-scored whenever Khala changes.
export const KhalaHeadToHeadCadence = S.Literals([
  'weekly',
  'per_khala_release',
  'on_demand',
])
export type KhalaHeadToHeadCadence = typeof KhalaHeadToHeadCadence.Type

// The category a comparator falls into — "the tools/models a developer would
// otherwise reach for". This is the framing the issue calls for (free/open vs
// paid frontier vs the default coding-agent model), used only to GROUP the
// published bar, never to route or to fabricate.
export const KhalaHeadToHeadComparatorCategory = S.Literals([
  'default_coding_agent_model',
  'free_or_open',
  'paid_frontier',
])
export type KhalaHeadToHeadComparatorCategory =
  typeof KhalaHeadToHeadComparatorCategory.Type

// One comparator the developer would otherwise reach for. The lane reuses the
// existing benchmark matrix lane vocabulary so there is never a parallel
// comparator taxonomy; the `category` only groups the published bar.
export type KhalaHeadToHeadComparatorDefinition = Readonly<{
  // The comparator lane (a matrix lane). `khala` is the protagonist, never a
  // comparator.
  lane: BenchmarkLane
  // The developer-facing label (public-safe English) of what this stands in for.
  label: string
  category: KhalaHeadToHeadComparatorCategory
  // The honest bar Khala aims to clear vs this comparator (public-safe English).
  barToClear: string
  // The owner-gate ref that must resolve before this comparator can publish a
  // decision-grade verdict (paid API keys + spend approval for billable lanes).
  ownerGateRef: string
}>

// The canonical developer-default comparator set. Lane membership mirrors the
// matrix availability table + the gym ladder's deliberate progression
// (`bigpickle` -> free/open -> paid frontier). These are the things a developer
// would otherwise point their tool at; Khala is measured against each one.
export const KHALA_HEAD_TO_HEAD_COMPARATORS: ReadonlyArray<KhalaHeadToHeadComparatorDefinition> =
  [
    {
      lane: 'bigpickle',
      label: "Big Pickle (OpenCode's default free model)",
      category: 'default_coding_agent_model',
      barToClear:
        'Beat the default coding-agent free model on cost-per-accepted-outcome AND solve-rate on the same task.',
      ownerGateRef:
        'gate.owner.khala.head_to_head.bigpickle.real_seam_with_model_id',
    },
    {
      lane: 'gemini-free',
      label: 'Free / open models (Gemini free tier class)',
      category: 'free_or_open',
      barToClear:
        'Match or beat the free field on solve-rate at equal-or-lower cost-per-accepted-outcome.',
      ownerGateRef: 'gate.owner.khala.head_to_head.free_tier_real_seam',
    },
    {
      lane: 'openai-gpt',
      label: 'Paid frontier — GPT class',
      category: 'paid_frontier',
      barToClear:
        'Honestly measure the gap to paid GPT-class frontier on both axes and track it shrinking over runs.',
      ownerGateRef:
        'gate.owner.khala.head_to_head.openai_gpt.paid_api_keys_and_spend_approval',
    },
    {
      lane: 'claude',
      label: 'Paid frontier — Claude class',
      category: 'paid_frontier',
      barToClear:
        'Honestly measure the gap to paid Claude-class frontier on both axes and track it shrinking over runs.',
      ownerGateRef:
        'gate.owner.khala.head_to_head.claude.paid_api_keys_and_spend_approval',
    },
    {
      lane: 'fireworks',
      label: 'Paid frontier — Fireworks DeepSeek class',
      category: 'paid_frontier',
      barToClear:
        'Match or beat the paid Fireworks open-weight serving lane on cost-per-accepted-outcome at equal solve-rate.',
      ownerGateRef:
        'gate.owner.khala.head_to_head.fireworks.paid_api_keys_and_spend_approval',
    },
  ]

// The recurring config: WHICH head-to-head, at WHAT cadence, where the published
// artifact dereferences, and which experiment config the owner-armed real sweep
// arms each cadence. PURE data, no clock.
export type KhalaHeadToHeadRecurringConfig = Readonly<{
  headToHeadRef: string
  cadence: KhalaHeadToHeadCadence
  // The well-known public dereference path for the latest published bar.
  publishPath: string
  // The experiment config id the bar re-runs each cadence (the external
  // head-to-head decision suite). The owner-armed real sweep arms this config.
  experimentConfigId: string
  // The demand attribution every Khala request the bar drives MUST carry (#6298)
  // so head-to-head traffic stays segmented from external/user traffic.
  demandKind: 'internal'
  demandSource: 'head_to_head'
  comparators: ReadonlyArray<KhalaHeadToHeadComparatorDefinition>
}>

export const KHALA_HEAD_TO_HEAD_RECURRING_CONFIG: KhalaHeadToHeadRecurringConfig =
  {
    headToHeadRef: 'head_to_head.public.khala_vs_developer_defaults.v1',
    cadence: 'per_khala_release',
    publishPath: '/api/public/khala/head-to-head',
    experimentConfigId: 'khala-vs-fireworks-vertex-decision-suite-oq5-v1',
    demandKind: 'internal',
    demandSource: 'head_to_head',
    comparators: KHALA_HEAD_TO_HEAD_COMPARATORS,
  }

// The measured axes for one side (Khala protagonist OR a comparator), carried
// straight from the public-safe gym leaderboard row. Only aggregate counts,
// durations, rates, and cost.
export type KhalaHeadToHeadSide = Readonly<{
  lane: BenchmarkLane | null
  reportRef: string
  candidateRef: string
  acceptedOutcomes: number
  attemptedVerifications: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  meanWallClockMs: number | null
  // Solve rate in basis points (verified-rate * 10000). Null when the side ran no
  // verified workload (honest absence, never a fabricated 0).
  solveRateBps: number | null
  costPerAcceptedOutcomeMsat: number
}>

// The honest verdict of one Khala-vs-comparator matchup on the two headline axes.
//   khala_wins_both        — Khala wins solve-rate AND cost-per-accepted-outcome
//   khala_wins_cost        — Khala cheaper per accepted outcome, comparable/lower solve
//   khala_wins_quality     — Khala higher solve-rate, comparable/higher cost
//   comparator_ahead       — the comparator is ahead on the axis(es) that matter
//   even                   — within the tolerance band on both axes
export const KhalaHeadToHeadVerdict = S.Literals([
  'khala_wins_both',
  'khala_wins_cost',
  'khala_wins_quality',
  'comparator_ahead',
  'even',
])
export type KhalaHeadToHeadVerdict = typeof KhalaHeadToHeadVerdict.Type

// One published (or awaiting-owner) matchup: Khala vs one comparator.
export type KhalaHeadToHeadMatchup = Readonly<{
  lane: BenchmarkLane
  label: string
  category: KhalaHeadToHeadComparatorCategory
  state: KhalaHeadToHeadComparatorState
  barToClear: string
  // The two measured sides, present only when `published`.
  khala: KhalaHeadToHeadSide | null
  comparator: KhalaHeadToHeadSide | null
  // The two-axis verdict, present only when `published`.
  verdict: KhalaHeadToHeadVerdict | null
  // Solve-rate delta in bps (khala - comparator); null unless both sides measured
  // a solve rate.
  solveRateDeltaBps: number | null
  // Cost-per-accepted-outcome delta in msat (comparator - khala): positive means
  // Khala is cheaper per accepted outcome. Null unless both sides published.
  costPerAcceptedOutcomeDeltaMsat: number | null
  // When `awaiting_owner`, the honest reasons + the owner-arm gate.
  blockerRefs: ReadonlyArray<string>
}>

export type KhalaHeadToHead = Readonly<{
  schemaVersion: typeof KhalaHeadToHeadSchemaVersion
  headToHeadRef: string
  cadence: KhalaHeadToHeadCadence
  publishPath: string
  demandKind: 'internal'
  demandSource: 'head_to_head'
  // The flat decision-grade projection the matchups are derived from. Reusing the
  // shipped projection keeps the ranking + public-safety boundary in one place;
  // the head-to-head only re-pairs Khala against each comparator.
  projectionRef: string
  decisionGradeRowCount: number
  // The Khala protagonist side (best decision-grade Khala row), null until armed.
  khala: KhalaHeadToHeadSide | null
  matchups: ReadonlyArray<KhalaHeadToHeadMatchup>
  excludedReports: ReadonlyArray<GymLeaderboardExcludedReport>
  caveatRefs: ReadonlyArray<string>
}>

const HEAD_TO_HEAD_CAVEATS = [
  'caveat.public.khala.head_to_head.decision_grade_matchups_only',
  'caveat.public.khala.head_to_head.fixture_or_synthetic_never_published',
  'caveat.public.khala.head_to_head.awaiting_owner_shows_gate_not_numbers',
  'caveat.public.khala.head_to_head.scored_on_solve_rate_and_cost_per_accepted_outcome',
  'caveat.public.khala.head_to_head.no_beats_frontier_claim_from_single_run',
  'caveat.public.khala.head_to_head.traffic_tagged_internal_head_to_head',
] as const

// Tolerance bands for the verdict. A solve-rate difference inside ±200 bps (2pp)
// or a cost difference inside ±5% is treated as "comparable" so a matchup is not
// declared a win on noise.
const SOLVE_RATE_TOLERANCE_BPS = 200
const COST_TOLERANCE_FRACTION = 0.05

// Recover the lane from a candidateRef prefix (the flat projection rows do not
// carry a lane field). Accept it only when it is a real benchmark lane.
const laneFromCandidateRef = (candidateRef: string): BenchmarkLane | null => {
  const head = candidateRef.split(/[./|]/)[0]
  if (head === undefined) {
    return null
  }
  try {
    const lane = head as BenchmarkLane
    laneAvailability(lane)
    return lane
  } catch {
    return null
  }
}

const isKhalaRow = (row: GymLeaderboardRow): boolean =>
  laneFromCandidateRef(row.candidateRef) === 'khala' ||
  row.candidateRef.includes('khala')

const toSide = (row: GymLeaderboardRow): KhalaHeadToHeadSide => ({
  lane: laneFromCandidateRef(row.candidateRef),
  reportRef: row.reportRef,
  candidateRef: row.candidateRef,
  acceptedOutcomes: row.acceptedOutcomes,
  attemptedVerifications: row.attemptedVerifications,
  inputTokens: row.inputTokens,
  outputTokens: row.outputTokens,
  totalTokens: row.totalTokens,
  meanWallClockMs: row.meanWallClockMs,
  solveRateBps: row.verificationRateBps,
  costPerAcceptedOutcomeMsat: row.costPerAcceptedOutcomeMsat,
})

// Decide the two-axis verdict for a Khala-vs-comparator matchup. PURE. Cost is
// always measured (the projection guarantees a cost-per-accepted-outcome); solve
// rate may be absent on either side (a chat-only run), in which case the verdict
// keys on cost alone.
const decideVerdict = (
  khala: KhalaHeadToHeadSide,
  comparator: KhalaHeadToHeadSide,
): KhalaHeadToHeadVerdict => {
  const costTolerance =
    comparator.costPerAcceptedOutcomeMsat * COST_TOLERANCE_FRACTION
  const khalaCheaper =
    khala.costPerAcceptedOutcomeMsat <
    comparator.costPerAcceptedOutcomeMsat - costTolerance
  const comparatorCheaper =
    comparator.costPerAcceptedOutcomeMsat <
    khala.costPerAcceptedOutcomeMsat - costTolerance

  // When either side did not measure a solve rate, the matchup is decided on cost.
  if (khala.solveRateBps === null || comparator.solveRateBps === null) {
    if (khalaCheaper) {
      return 'khala_wins_cost'
    }
    if (comparatorCheaper) {
      return 'comparator_ahead'
    }
    return 'even'
  }

  const solveDelta = khala.solveRateBps - comparator.solveRateBps
  const khalaSolvesMore = solveDelta > SOLVE_RATE_TOLERANCE_BPS
  const comparatorSolvesMore = solveDelta < -SOLVE_RATE_TOLERANCE_BPS

  if (khalaSolvesMore && khalaCheaper) {
    return 'khala_wins_both'
  }
  if (comparatorSolvesMore && comparatorCheaper) {
    return 'comparator_ahead'
  }
  if (khalaCheaper && !comparatorSolvesMore) {
    return 'khala_wins_cost'
  }
  if (khalaSolvesMore && !comparatorCheaper) {
    return 'khala_wins_quality'
  }
  if (comparatorSolvesMore || comparatorCheaper) {
    return 'comparator_ahead'
  }
  return 'even'
}

// The honest blockers for a matchup that has no published comparator measurement.
// A comparator lane that is `available` but unmeasured needs the owner-armed real
// seam; a `fixture_only` / `not_yet_available` lane needs a real executor wired;
// a matchup with no decision-grade Khala row cannot publish either.
const matchupBlockers = (
  definition: KhalaHeadToHeadComparatorDefinition,
  hasKhalaRow: boolean,
  comparatorMeasured: boolean,
): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (!hasKhalaRow) {
    blockers.push('blocker.khala.head_to_head.no_decision_grade_khala_row')
  }
  if (!comparatorMeasured) {
    const availability = laneAvailability(definition.lane)
    if (availability === 'available') {
      blockers.push(
        `blocker.khala.head_to_head.comparator_not_yet_measured.${definition.lane}`,
      )
    } else {
      blockers.push(
        `blocker.khala.head_to_head.comparator_lane_${availability}.${definition.lane}`,
      )
    }
  }
  if (definition.ownerGateRef !== '') {
    blockers.push(definition.ownerGateRef)
  }
  return uniqueRefs(blockers)
}

// Build the published external head-to-head. Runs the shipped flat projection
// (which filters out everything not a public-safe decision-grade report), then
// pairs the best Khala protagonist row against each comparator lane that was
// measured decision-grade. A comparator with no measured row is `awaiting_owner`
// and shows the owner-arm gate, never a fabricated number. PURE.
export const buildKhalaHeadToHead = (
  inputs: ReadonlyArray<GymLeaderboardReportInput>,
  config: KhalaHeadToHeadRecurringConfig = KHALA_HEAD_TO_HEAD_RECURRING_CONFIG,
): KhalaHeadToHead => {
  const projection = buildGymLeaderboardProjection(inputs)

  const khalaRows = projection.rows.filter(isKhalaRow)
  // The flat projection ranks by cost-per-accepted-outcome ascending, so the first
  // Khala row is the best (cheapest-per-accepted) decision-grade Khala protagonist.
  const bestKhalaRow = khalaRows[0]
  const khalaSide = bestKhalaRow === undefined ? null : toSide(bestKhalaRow)

  const matchups: Array<KhalaHeadToHeadMatchup> = config.comparators.map(
    definition => {
      const comparatorRows = projection.rows.filter(
        row => laneFromCandidateRef(row.candidateRef) === definition.lane,
      )
      // Best (cheapest-per-accepted) decision-grade row for this comparator lane.
      const comparatorRow = comparatorRows[0]
      const published = khalaSide !== null && comparatorRow !== undefined
      const comparatorSide =
        comparatorRow === undefined ? null : toSide(comparatorRow)

      const verdict =
        published && khalaSide !== null && comparatorSide !== null
          ? decideVerdict(khalaSide, comparatorSide)
          : null
      const solveRateDeltaBps =
        khalaSide?.solveRateBps !== null &&
        khalaSide?.solveRateBps !== undefined &&
        comparatorSide?.solveRateBps !== null &&
        comparatorSide?.solveRateBps !== undefined
          ? khalaSide.solveRateBps - comparatorSide.solveRateBps
          : null
      const costPerAcceptedOutcomeDeltaMsat =
        khalaSide !== null && comparatorSide !== null
          ? comparatorSide.costPerAcceptedOutcomeMsat -
            khalaSide.costPerAcceptedOutcomeMsat
          : null

      return {
        lane: definition.lane,
        label: definition.label,
        category: definition.category,
        state: published ? 'published' : 'awaiting_owner',
        barToClear: definition.barToClear,
        khala: published ? khalaSide : null,
        comparator: comparatorSide,
        verdict,
        solveRateDeltaBps: published ? solveRateDeltaBps : null,
        costPerAcceptedOutcomeDeltaMsat: published
          ? costPerAcceptedOutcomeDeltaMsat
          : null,
        blockerRefs: published
          ? []
          : matchupBlockers(
              definition,
              khalaSide !== null,
              comparatorRow !== undefined,
            ),
      }
    },
  )

  return {
    schemaVersion: KhalaHeadToHeadSchemaVersion,
    headToHeadRef: config.headToHeadRef,
    cadence: config.cadence,
    publishPath: config.publishPath,
    demandKind: config.demandKind,
    demandSource: config.demandSource,
    projectionRef: projection.projectionRef,
    decisionGradeRowCount: projection.rowCount,
    khala: khalaSide,
    matchups,
    excludedReports: projection.excludedReports,
    caveatRefs: uniqueRefs([
      ...projection.caveatRefs,
      ...HEAD_TO_HEAD_CAVEATS,
    ]),
  }
}

// A stable public dereference ref for one published head-to-head snapshot, derived
// from the ref + run timestamp. PURE string assembly over public-safe values.
export const khalaHeadToHeadSnapshotRef = (
  headToHeadRef: string,
  runIso: string,
): string =>
  `snapshot.${publicRefSegment(`${headToHeadRef}.${runIso}`, 'head_to_head')}`
