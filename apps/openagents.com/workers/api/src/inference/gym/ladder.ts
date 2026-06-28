// Gym benchmark LADDER as a recurring, published leaderboard (#6309).
//
// GTM doc §4 "Run it through the gym — the benchmark ladder": the gym that
// trains/uses Khala also BENCHMARKS it on a recurring basis over a ladder of
// opponents on identical prompts and our axes (cost-per-accepted-outcome,
// verified-rate, tool-call completion, wall-clock), publishing the result as a
// recurring dereferenceable leaderboard. The progression is deliberate:
//
//   Rung 1 — Big Pickle (OpenCode's default free model): the baseline.
//   Rung 2 — other free/open models: the field users reach for when not paying.
//   Rung 3 — paid frontier (Claude / GPT / Gemini class): the upper bound.
//
// This module owns ONLY the PUBLISHING layer on top of the already-shipped
// benchmark harness (matrix, runner, report) and the flat
// `buildGymLeaderboardProjection`. It does NOT re-implement the harness, the
// matrix, the cost math, or the public-safety boundary — it consumes them. It is
// PURE and framework-agnostic: no Worker, no network, no clock. The route + store
// (`ladder-routes.ts`, `ladder-store.ts`) publish what this builds.
//
// HONESTY: every rung is labeled with how it was produced. A rung backed by a
// `decisionGrade: true` report from the owner-armed real seam is `published`. A
// rung whose opponent lanes are `fixture_only` (no real executor wired) or that
// requires owner-held API keys / spend approval is `awaiting_owner` — the ladder
// shows the rung shape and the gate, never a fabricated measurement. Fixture /
// synthetic numbers are NEVER published as a rung measurement.
import { Schema as S } from 'effect'

import {
  type BenchmarkLane,
  laneAvailability,
} from '../benchmark'
import { publicRefSegment, uniqueRefs } from '../../public-ref-format'
import {
  buildGymLeaderboardProjection,
  type GymLeaderboardExcludedReport,
  type GymLeaderboardProjection,
  type GymLeaderboardReportInput,
  type GymLeaderboardRow,
} from './leaderboard'

export const GymLadderLeaderboardSchemaVersion =
  'openagents.gym.ladder_leaderboard.v1'

// The three rungs of the ladder, in deliberate climbing order.
export const GymLadderRungId = S.Literals(['rung1', 'rung2', 'rung3'])
export type GymLadderRungId = typeof GymLadderRungId.Type

// How a rung was produced. Only `published` rungs carry decision-grade numbers.
//   published      — at least one opponent row is a decision-grade real measurement
//   awaiting_owner — the rung's opponent lanes need the owner-armed real seam
//                    (fixture-only lanes, or paid lanes needing owner API keys +
//                    spend approval). Honest gate, never a fabricated number.
export const GymLadderRungState = S.Literals(['published', 'awaiting_owner'])
export type GymLadderRungState = typeof GymLadderRungState.Type

// The recurring cadence at which the ladder is re-run + re-published.
export const GymLadderCadence = S.Literals([
  'weekly',
  'per_model_release',
  'on_demand',
])
export type GymLadderCadence = typeof GymLadderCadence.Type

// The lane membership + intent of one rung. This is the deliberate-progression
// contract: which opponents belong to which rung, and what Khala must clear.
export type GymLadderRungDefinition = Readonly<{
  rung: GymLadderRungId
  title: string
  // The opponent lanes for this rung. `khala` is the protagonist and is never an
  // opponent.
  opponentLanes: ReadonlyArray<BenchmarkLane>
  // The honest bar Khala must clear on this rung (public-safe English).
  barToClear: string
  // The owner-gate that must resolve before this rung can publish decision-grade
  // numbers, when it is not yet self-publishing. Empty when no extra gate beyond
  // the standard real-sweep arming.
  ownerGateRef: string
}>

// The canonical three-rung ladder definition. Lane membership mirrors the matrix
// availability table + the planning doc `opencode-gym-benchmark-ladder.md`.
export const GYM_LADDER_RUNGS: ReadonlyArray<GymLadderRungDefinition> = [
  {
    rung: 'rung1',
    title: 'Rung 1 — Khala vs Big Pickle',
    opponentLanes: ['bigpickle'],
    barToClear:
      'Beat Big Pickle (OpenCode default free model) on cost-per-accepted-outcome AND verified-rate on the same coding task.',
    ownerGateRef:
      'gate.owner.gym.ladder.rung1.real_seam_with_bigpickle_model_id',
  },
  {
    rung: 'rung2',
    title: 'Rung 2 — Khala vs free / open models',
    opponentLanes: ['gemini-free'],
    barToClear:
      'Match or beat the free field on verified-rate at equal-or-lower cost; win on tool-call completion.',
    ownerGateRef: 'gate.owner.gym.ladder.rung2.free_tier_real_seam',
  },
  {
    rung: 'rung3',
    title: 'Rung 3 — Khala vs paid frontier',
    opponentLanes: ['openai-gpt', 'claude'],
    barToClear:
      'Honestly measure the gap to paid frontier models and track it shrinking over successive runs. No requirement to beat today.',
    ownerGateRef: 'gate.owner.gym.ladder.rung3.paid_api_keys_and_spend_approval',
  },
]

// The recurring config: WHICH ladder, at WHAT cadence, and the canonical
// dereference path the published artifact lives at. This is the "recurring run
// mechanism" surface — a scheduler / operator reads this to know when to re-run
// and where to publish. PURE data, no clock.
export type GymLadderRecurringConfig = Readonly<{
  ladderRef: string
  cadence: GymLadderCadence
  // The well-known public dereference path for the latest published ladder.
  publishPath: string
  // The experiment config id the ladder re-runs each cadence (the OpenCode
  // head-to-head matrix). The owner-armed real sweep arms this config.
  experimentConfigId: string
  // The demand attribution tags every Khala request the ladder drives MUST carry
  // (#6298) so gym traffic stays segmented from external traffic.
  demandKind: 'internal'
  demandSource: 'gym_ladder'
  rungs: ReadonlyArray<GymLadderRungDefinition>
}>

export const GYM_LADDER_RECURRING_CONFIG: GymLadderRecurringConfig = {
  ladderRef: 'ladder.public.gym.opencode_khala_vs_field.v1',
  cadence: 'per_model_release',
  publishPath: '/api/public/gym/leaderboard',
  experimentConfigId: 'gym-opencode-khala-vs-bigpickle-fixture-v1',
  demandKind: 'internal',
  demandSource: 'gym_ladder',
  rungs: GYM_LADDER_RUNGS,
}

// One opponent entry inside a rung: a candidate that was actually measured
// decision-grade, alongside the Khala protagonist row for the same rung. Carries
// only the public-safe fields the flat leaderboard already exposes.
export type GymLadderOpponentEntry = Readonly<{
  rank: number
  lane: BenchmarkLane | null
  reportRef: string
  candidateRef: string
  acceptedOutcomes: number
  verificationRateBps: number | null
  costPerAcceptedOutcomeMsat: number
}>

// One rung of the published ladder.
export type GymLadderRung = Readonly<{
  rung: GymLadderRungId
  title: string
  state: GymLadderRungState
  barToClear: string
  opponentLanes: ReadonlyArray<BenchmarkLane>
  // The decision-grade rows that landed on this rung (Khala + measured
  // opponents). Empty when the rung is `awaiting_owner`.
  entries: ReadonlyArray<GymLadderOpponentEntry>
  // When `awaiting_owner`, the honest reasons the rung cannot publish yet.
  blockerRefs: ReadonlyArray<string>
}>

export type GymLadderLeaderboard = Readonly<{
  schemaVersion: typeof GymLadderLeaderboardSchemaVersion
  ladderRef: string
  cadence: GymLadderCadence
  publishPath: string
  demandKind: 'internal'
  demandSource: 'gym_ladder'
  // The flat decision-grade projection the rungs are derived from. Reusing the
  // shipped projection keeps the ranking math + public-safety boundary in one
  // place; the ladder only re-groups its rows into rungs.
  projectionRef: string
  decisionGradeRowCount: number
  rungs: ReadonlyArray<GymLadderRung>
  excludedReports: ReadonlyArray<GymLeaderboardExcludedReport>
  caveatRefs: ReadonlyArray<string>
}>

const LADDER_CAVEATS = [
  'caveat.public.gym.ladder.decision_grade_rungs_only',
  'caveat.public.gym.ladder.fixture_or_synthetic_never_published',
  'caveat.public.gym.ladder.awaiting_owner_rungs_show_gate_not_numbers',
  'caveat.public.gym.ladder.no_beats_frontier_claim_from_single_run',
  'caveat.public.gym.ladder.gym_traffic_tagged_internal_gym_ladder',
] as const

// Recover the lane from a candidateRef prefix for opponent classification. The
// flat projection rows do not carry a lane field, so we read the leading segment
// and accept it only when it is a real benchmark lane; otherwise null (the row is
// still ranked honestly, it just does not classify into a rung's opponent set).
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

const toOpponentEntry = (row: GymLeaderboardRow): GymLadderOpponentEntry => ({
  rank: row.rank,
  lane: laneFromCandidateRef(row.candidateRef),
  reportRef: row.reportRef,
  candidateRef: row.candidateRef,
  acceptedOutcomes: row.acceptedOutcomes,
  verificationRateBps: row.verificationRateBps,
  costPerAcceptedOutcomeMsat: row.costPerAcceptedOutcomeMsat,
})

// The honest blockers for a rung that has no published opponent measurement yet.
// A `fixture_only` opponent lane needs a real executor wired + owner arming; a
// rung with no decision-grade Khala protagonist row cannot publish either.
const rungBlockers = (
  definition: GymLadderRungDefinition,
  hasKhalaRow: boolean,
  measuredOpponentLanes: ReadonlySet<BenchmarkLane>,
): ReadonlyArray<string> => {
  const blockers: Array<string> = []
  if (!hasKhalaRow) {
    blockers.push('blocker.gym.ladder.no_decision_grade_khala_row')
  }
  for (const lane of definition.opponentLanes) {
    if (measuredOpponentLanes.has(lane)) {
      continue
    }
    const availability = laneAvailability(lane)
    if (availability === 'available') {
      blockers.push(`blocker.gym.ladder.opponent_not_yet_measured.${lane}`)
    } else {
      blockers.push(`blocker.gym.ladder.opponent_lane_${availability}.${lane}`)
    }
  }
  if (definition.ownerGateRef !== '') {
    blockers.push(definition.ownerGateRef)
  }
  return uniqueRefs(blockers)
}

// Build the published ladder leaderboard. It runs the shipped flat projection
// (which filters out everything that is not a public-safe decision-grade report),
// then re-groups the surviving rows into the three rungs. The best Khala
// protagonist row (lowest cost-per-accepted-outcome decision-grade Khala report)
// is shown on every rung that has at least one measured opponent; a rung with no
// measured opponent is `awaiting_owner` and shows the gate, never a number.
export const buildGymLadderLeaderboard = (
  inputs: ReadonlyArray<GymLeaderboardReportInput>,
  config: GymLadderRecurringConfig = GYM_LADDER_RECURRING_CONFIG,
): GymLadderLeaderboard => {
  const projection: GymLeaderboardProjection =
    buildGymLeaderboardProjection(inputs)

  const khalaRows = projection.rows.filter(isKhalaRow)
  const bestKhalaRow = khalaRows[0]

  const rungs: Array<GymLadderRung> = config.rungs.map(definition => {
    const opponentLaneSet = new Set(definition.opponentLanes)
    const measuredOpponentRows = projection.rows.filter(row => {
      const lane = laneFromCandidateRef(row.candidateRef)
      return lane !== null && opponentLaneSet.has(lane)
    })
    const measuredOpponentLanes = new Set(
      measuredOpponentRows
        .map(row => laneFromCandidateRef(row.candidateRef))
        .filter((lane): lane is BenchmarkLane => lane !== null),
    )

    const published =
      bestKhalaRow !== undefined && measuredOpponentRows.length > 0
    const entries = published
      ? [
          toOpponentEntry(bestKhalaRow),
          ...measuredOpponentRows.map(toOpponentEntry),
        ]
          // Re-rank within the rung by cost-per-accepted-outcome so the rung is
          // self-consistent even though the flat projection ranked globally.
          .sort(
            (left, right) =>
              left.costPerAcceptedOutcomeMsat -
              right.costPerAcceptedOutcomeMsat,
          )
          .map((entry, index) => ({ ...entry, rank: index + 1 }))
      : []

    return {
      rung: definition.rung,
      title: definition.title,
      state: published ? 'published' : 'awaiting_owner',
      barToClear: definition.barToClear,
      opponentLanes: definition.opponentLanes,
      entries,
      blockerRefs: published
        ? []
        : rungBlockers(
            definition,
            bestKhalaRow !== undefined,
            measuredOpponentLanes,
          ),
    }
  })

  return {
    schemaVersion: GymLadderLeaderboardSchemaVersion,
    ladderRef: config.ladderRef,
    cadence: config.cadence,
    publishPath: config.publishPath,
    demandKind: config.demandKind,
    demandSource: config.demandSource,
    projectionRef: projection.projectionRef,
    decisionGradeRowCount: projection.rowCount,
    rungs,
    excludedReports: projection.excludedReports,
    caveatRefs: uniqueRefs([...projection.caveatRefs, ...LADDER_CAVEATS]),
  }
}

// A stable public dereference ref for one published ladder snapshot, derived from
// the ladder ref + the run timestamp. PURE string assembly over public-safe
// values; no clock.
export const gymLadderSnapshotRef = (
  ladderRef: string,
  runIso: string,
): string =>
  `snapshot.${publicRefSegment(`${ladderRef}.${runIso}`, 'gym_ladder')}`
