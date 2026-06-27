// Artanis situational-awareness context builder (issue #6363, epic #6359).
//
// This is the other half of #6363: the "Talk to Artanis" operator channel must
// let Artanis answer "what are you doing?" GROUNDED IN LIVE STATE, not
// training-data roleplay. This module assembles a bounded, owner-only context
// object from REAL state already in the system:
//
//   buildArtanisSituationalAwareness(ownerId) -> { recentActions, goals, ongoingOps }
//
//   - recentActions: recent commits, Pylon-Codex assignments / token rows,
//       issues opened+closed, and the Artanis tick log.
//   - goals: the master roadmap + the open epics that define the current
//       sequence (#6359 / #6316 / #6303).
//   - ongoingOps: active assignments, recent deploys (Worker versions), GLM
//       fleet readiness, and the public counter state.
//
// Design notes (load-bearing):
//   - READ-ONLY. This builder never writes, dispatches, spends, or mutates. It
//     only reads state to describe it.
//   - DECOUPLED. The GLM serving/scheduler/stress lane is owned by another
//     agent. Rather than import that code, this builder takes injected READER
//     functions for every source. The Artanis operator core wires real readers
//     (the GLM readiness projection, the public counter route, the tick store,
//     etc.); tests wire fakes. This keeps awareness logic out of GLM
//     serving/router/admission code entirely.
//   - OWNER-ONLY + PUBLIC-SAFE-INTERNALLY. The result is bounded structured
//     summaries for the owner. It must never carry raw prompts, raw shell
//     output, credentials, wallet material, or another owner's private data.
//     Readers are expected to return already-redacted/public-safe summaries; the
//     builder additionally bounds list sizes.

import type { ArtanisTokenPaceBlock } from './artanis-token-pace'
import { currentIsoTimestamp } from './runtime-primitives'

// Typed validation error for the awareness builder so callers can branch on the
// tag, and so the module stays free of generic thrown Errors.
export class ArtanisAwarenessValidationError extends Error {
  readonly _tag = 'ArtanisAwarenessValidationError'
  constructor(reason: string) {
    super(reason)
    this.name = 'ArtanisAwarenessValidationError'
  }
}

// ---------------------------------------------------------------------------
// recentActions
// ---------------------------------------------------------------------------

export type ArtanisRecentCommit = Readonly<{
  sha: string
  summary: string
  committedAt: string
}>

export type ArtanisRecentAssignment = Readonly<{
  assignmentRef: string
  // e.g. 'accepted' | 'closeout_submitted' | 'dispatched'
  state: string
  // Public-safe one-line objective summary; never the raw prompt.
  objective: string | null
  updatedAt: string
}>

export type ArtanisRecentIssueChange = Readonly<{
  number: number
  title: string
  // 'opened' | 'closed'
  change: 'opened' | 'closed'
  at: string
}>

export type ArtanisRecentTick = Readonly<{
  decisionRef: string
  state: 'dispatched' | 'no_action' | 'blocked' | 'dispatch_failed'
  assignmentRef: string | null
  at: string
}>

export type ArtanisRecentActions = Readonly<{
  commits: ReadonlyArray<ArtanisRecentCommit>
  assignments: ReadonlyArray<ArtanisRecentAssignment>
  issueChanges: ReadonlyArray<ArtanisRecentIssueChange>
  ticks: ReadonlyArray<ArtanisRecentTick>
}>

// ---------------------------------------------------------------------------
// goals
// ---------------------------------------------------------------------------

export type ArtanisGoalEpic = Readonly<{
  number: number
  title: string
  // Short public-safe statement of what the epic is driving.
  mandate: string
}>

export type ArtanisGoals = Readonly<{
  // The master roadmap that defines the current sequence.
  roadmapRef: string
  roadmapSummary: string
  // Open epics in priority order. #6359 is the Artanis autonomy epic, #6316 the
  // serving track, #6303 the Khala product track.
  epics: ReadonlyArray<ArtanisGoalEpic>
}>

// ---------------------------------------------------------------------------
// ongoingOps
// ---------------------------------------------------------------------------

export type ArtanisActiveAssignment = Readonly<{
  assignmentRef: string
  state: string
  // Where in the run it is, if known (e.g. 'proof-ready').
  phase: string | null
  startedAt: string
}>

export type ArtanisRecentDeploy = Readonly<{
  workerVersion: string
  deployedAt: string
}>

export type ArtanisFleetReadiness = Readonly<{
  // 'ready' | 'degraded' | 'unavailable', mirrored from the GLM readiness
  // projection. Read-only summary; the GLM lane owns the projection itself.
  status: string
  readyReplicas: number
  totalReplicas: number
}>

export type ArtanisPublicCounter = Readonly<{
  tokensServed: number
  asOf: string
}>

// Re-export the token-pace block type so awareness consumers import it from
// one place. The pace block tells Artanis whether today is on track for the
// daily token target (at least 4x the prior day, goal 10x).
export type { ArtanisTokenPaceBlock } from './artanis-token-pace'

export type ArtanisOngoingOps = Readonly<{
  activeAssignments: ReadonlyArray<ArtanisActiveAssignment>
  recentDeploys: ReadonlyArray<ArtanisRecentDeploy>
  fleetReadiness: ArtanisFleetReadiness | null
  publicCounter: ArtanisPublicCounter | null
  // Live daily token-pace block: today's tokens, the midnight projection,
  // yesterday's baseline, the 4x/10x targets, and whether we are behind pace.
  // Injected into EVERY Artanis turn so he sees the gap without calling a tool.
  tokenPace: ArtanisTokenPaceBlock | null
}>

// ---------------------------------------------------------------------------
// the assembled context
// ---------------------------------------------------------------------------

export type ArtanisSituationalAwareness = Readonly<{
  kind: 'artanis_situational_awareness'
  ownerOnly: true
  ownerId: string
  generatedAt: string
  recentActions: ArtanisRecentActions
  goals: ArtanisGoals
  ongoingOps: ArtanisOngoingOps
}>

// ---------------------------------------------------------------------------
// injected readers (read-only; supplied by the operator core, faked in tests)
// ---------------------------------------------------------------------------

// Every reader is best-effort: a failure or absence of a source degrades that
// one bucket to empty/null rather than failing the whole awareness build. A
// missing source is honest absence, never a fabricated value.
export type ArtanisAwarenessReaders = Readonly<{
  readRecentCommits?:
    | ((limit: number) => Promise<ReadonlyArray<ArtanisRecentCommit>>)
    | undefined
  // Owner-scoped: the operator core passes ownerId so only this owner's
  // assignments are read.
  readRecentAssignments?:
    | ((
        ownerId: string,
        limit: number,
      ) => Promise<ReadonlyArray<ArtanisRecentAssignment>>)
    | undefined
  readRecentIssueChanges?:
    | ((limit: number) => Promise<ReadonlyArray<ArtanisRecentIssueChange>>)
    | undefined
  readRecentTicks?:
    | ((limit: number) => Promise<ReadonlyArray<ArtanisRecentTick>>)
    | undefined
  readGoals?: (() => Promise<ArtanisGoals>) | undefined
  readActiveAssignments?:
    | ((
        ownerId: string,
        limit: number,
      ) => Promise<ReadonlyArray<ArtanisActiveAssignment>>)
    | undefined
  readRecentDeploys?:
    | ((limit: number) => Promise<ReadonlyArray<ArtanisRecentDeploy>>)
    | undefined
  readFleetReadiness?:
    | (() => Promise<ArtanisFleetReadiness | null>)
    | undefined
  readPublicCounter?:
    | (() => Promise<ArtanisPublicCounter | null>)
    | undefined
  // Live token-pace reader (read-only, fail-soft). Returns the computed pace
  // block or null when the public stats cannot ground a projection.
  readTokenPace?:
    | (() => Promise<ArtanisTokenPaceBlock | null>)
    | undefined
}>

export type ArtanisAwarenessBounds = Readonly<{
  commits: number
  assignments: number
  issueChanges: number
  ticks: number
  activeAssignments: number
  recentDeploys: number
}>

export const ARTANIS_AWARENESS_DEFAULT_BOUNDS: ArtanisAwarenessBounds = {
  activeAssignments: 10,
  assignments: 10,
  commits: 10,
  issueChanges: 10,
  recentDeploys: 5,
  ticks: 10,
}

// The static goal context. The current open epics and roadmap reference are
// code-anchored so awareness is grounded even when no live goal reader is
// wired; a live reader can override this.
export const ARTANIS_DEFAULT_GOALS: ArtanisGoals = {
  epics: [
    {
      mandate:
        'Artanis autonomously owns the whole Khala improvement loop: unblock users, keep inference solid, drive the burndown loop, act on feedback.',
      number: 6359,
      title: 'Artanis: autonomous owner of the loop',
    },
    {
      mandate:
        'The GLM serving track: throughput, readiness, durability, and capacity for Khala inference.',
      number: 6316,
      title: 'Khala serving track',
    },
    {
      mandate: 'Khala product track: ship the user-facing Khala surface.',
      number: 6303,
      title: 'Khala product',
    },
  ],
  roadmapRef:
    'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
  roadmapSummary:
    'Master roadmap for the open Khala issue set and the Artanis autonomy mandate; the current sequence Artanis is driving.',
}

const boundList = <A>(
  items: ReadonlyArray<A> | undefined,
  limit: number,
): ReadonlyArray<A> => (items === undefined ? [] : items.slice(0, limit))

// Run a reader best-effort: any rejection or absence degrades to the fallback.
const safeRead = async <A>(
  reader: (() => Promise<A>) | undefined,
  fallback: A,
): Promise<A> => {
  if (reader === undefined) {
    return fallback
  }
  try {
    return await reader()
  } catch {
    return fallback
  }
}

export type BuildArtanisSituationalAwarenessOptions = Readonly<{
  bounds?: Partial<ArtanisAwarenessBounds> | undefined
  nowIso?: (() => string) | undefined
}>

// buildArtanisSituationalAwareness(ownerId) -> { recentActions, goals, ongoingOps }
//
// Assembles the three buckets from the injected read-only sources, bounded and
// owner-scoped. Consumed by the Artanis operator core (#6359) via exactly this
// signature; independently testable with faked readers.
export const buildArtanisSituationalAwareness = async (
  ownerId: string,
  readers: ArtanisAwarenessReaders = {},
  options: BuildArtanisSituationalAwarenessOptions = {},
): Promise<ArtanisSituationalAwareness> => {
  const owner = ownerId?.trim?.() ?? ''
  if (owner.length === 0) {
    throw new ArtanisAwarenessValidationError(
      'buildArtanisSituationalAwareness: ownerId must be non-empty',
    )
  }

  const bounds: ArtanisAwarenessBounds = {
    ...ARTANIS_AWARENESS_DEFAULT_BOUNDS,
    ...(options.bounds ?? {}),
  }
  const nowIso = options.nowIso ?? currentIsoTimestamp

  const [
    commits,
    assignments,
    issueChanges,
    ticks,
    goals,
    activeAssignments,
    recentDeploys,
    fleetReadiness,
    publicCounter,
    tokenPace,
  ] = await Promise.all([
    safeRead(
      readers.readRecentCommits
        ? () => readers.readRecentCommits!(bounds.commits)
        : undefined,
      [] as ReadonlyArray<ArtanisRecentCommit>,
    ),
    safeRead(
      readers.readRecentAssignments
        ? () => readers.readRecentAssignments!(owner, bounds.assignments)
        : undefined,
      [] as ReadonlyArray<ArtanisRecentAssignment>,
    ),
    safeRead(
      readers.readRecentIssueChanges
        ? () => readers.readRecentIssueChanges!(bounds.issueChanges)
        : undefined,
      [] as ReadonlyArray<ArtanisRecentIssueChange>,
    ),
    safeRead(
      readers.readRecentTicks
        ? () => readers.readRecentTicks!(bounds.ticks)
        : undefined,
      [] as ReadonlyArray<ArtanisRecentTick>,
    ),
    safeRead(readers.readGoals, ARTANIS_DEFAULT_GOALS),
    safeRead(
      readers.readActiveAssignments
        ? () =>
            readers.readActiveAssignments!(owner, bounds.activeAssignments)
        : undefined,
      [] as ReadonlyArray<ArtanisActiveAssignment>,
    ),
    safeRead(
      readers.readRecentDeploys
        ? () => readers.readRecentDeploys!(bounds.recentDeploys)
        : undefined,
      [] as ReadonlyArray<ArtanisRecentDeploy>,
    ),
    safeRead(readers.readFleetReadiness, null as ArtanisFleetReadiness | null),
    safeRead(readers.readPublicCounter, null as ArtanisPublicCounter | null),
    safeRead(readers.readTokenPace, null as ArtanisTokenPaceBlock | null),
  ])

  return {
    generatedAt: nowIso(),
    goals,
    kind: 'artanis_situational_awareness',
    ongoingOps: {
      activeAssignments: boundList(
        activeAssignments,
        bounds.activeAssignments,
      ),
      fleetReadiness,
      publicCounter,
      recentDeploys: boundList(recentDeploys, bounds.recentDeploys),
      tokenPace,
    },
    ownerId: owner,
    ownerOnly: true,
    recentActions: {
      assignments: boundList(assignments, bounds.assignments),
      commits: boundList(commits, bounds.commits),
      issueChanges: boundList(issueChanges, bounds.issueChanges),
      ticks: boundList(ticks, bounds.ticks),
    },
  }
}
