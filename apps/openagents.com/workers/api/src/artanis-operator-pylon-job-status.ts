// Artanis owner-scoped Pylon job-STATUS reader (iteration-3 capability).
//
// This is the production seam behind the `get_pylon_job_status` read tool
// (`artanis-operator-tools.ts`). Given an assignment ref it resolves the REAL
// public-safe closeout/proof status of ONE owner-scoped Pylon/Codex assignment
// from the Pylon API store, so Artanis can verify whether a delegated burndown
// task actually passed, read the failing check, and iterate the next dispatch.
//
// It stays conservative by construction:
//
//   - OWNER-SCOPED. It reads the owner's own linked-agent set first
//     (`listLinkedAgentUserIds`, the same seam the gated dispatch uses) and only
//     returns a status when the assignment is owned by one of those linked agent
//     credentials. An assignment owned by a different owner reads as `null`
//     (honest absence) — it never leaks another owner's work.
//   - PUBLIC-SAFE. Every ref is taken from the store's public assignment
//     projection (`publicPylonApiAssignmentProjection`, which already runs the
//     scanner-safe ref filter); the failure summary is built from those
//     public-safe rejection/blocker refs only. No raw prompts, shell output,
//     credentials, wallet material, or local paths ever leave this reader.
//   - READ-ONLY + FAIL-SOFT. It never writes, dispatches, spends, or mutates. A
//     missing assignment returns `null`; the tool turns a thrown rejection into
//     an honest "(could not read status …)" string rather than a fabricated
//     status.

import type {
  ArtanisPylonJobStatus,
  ArtanisPylonJobStatusReader,
  ArtanisPylonJobVerifyResult,
} from './artanis-operator-tools'
import {
  type PylonApiEventRecord,
  type PylonApiStore,
  publicPylonApiAssignmentProjection,
} from './pylon-api'
import { currentIsoTimestamp } from './runtime-primitives'

// Max events pulled per status read. Bounded so a chatty assignment stays cheap.
export const ARTANIS_JOB_STATUS_EVENT_LIMIT = 50

export type ArtanisPylonJobStatusReaderDeps = Readonly<{
  // The Pylon API store used to read the assignment + its events (read-only).
  pylonStore: PylonApiStore
  // The OpenAuth user id of the authenticated owner (the chat session user id).
  ownerOpenAuthUserId: string
  // Resolve the owner's linked agent user ids (their Pylon-owning credentials),
  // the same owner-scope resolver the gated dispatch seam uses.
  listLinkedAgentUserIds: (
    ownerOpenAuthUserId: string,
  ) => Promise<ReadonlyArray<string>>
  // Clock seam (testable); defaults to the current ISO timestamp.
  nowIso?: (() => string) | undefined
  // Max events pulled per read; defaults to ARTANIS_JOB_STATUS_EVENT_LIMIT.
  eventLimit?: number | undefined
}>

// Public-safe blocker refs gathered from the assignment's events. Progress and
// worker-closeout event bodies may carry a `blockerRefs` array (the public-safe
// PylonApiAssignmentProgressRequest/WorkerCloseoutRequest field); a `blocked`
// status event is itself signal. All refs here originate from PublicSafeRefs at
// write time, so they are public-safe by construction.
const blockerRefsFromEvents = (
  events: ReadonlyArray<PylonApiEventRecord>,
): ReadonlyArray<string> => {
  const refs: Array<string> = []
  for (const event of events) {
    const value = event.eventBody.blockerRefs
    if (Array.isArray(value)) {
      for (const ref of value) {
        if (typeof ref === 'string' && ref.trim() !== '') {
          refs.push(ref.trim())
        }
      }
    }
  }
  return [...new Set(refs)]
}

const hasProofEvent = (
  events: ReadonlyArray<PylonApiEventRecord>,
): boolean =>
  events.some(event => event.eventKind === 'artifact_proof_metadata')

// Derive the verify/proof verdict from the REAL closeout state. The closeout
// path records acceptedWorkRefs for an accepted closeout and rejectionRefs for a
// rejected one (`closeoutPylonApiAssignmentRecord`). Honest, conservative:
//   - 'fail'    : the assignment was rejected/blocked, OR a closeout carried
//                 rejection refs, OR a blocker was observed.
//   - 'pass'    : a submitted closeout with retained proof/artifacts and no
//                 rejection/blocker refs.
//   - 'unknown' : still in progress (offered/accepted/running/proof_submitted)
//                 or otherwise unresolved.
const deriveVerifyResult = (input: {
  state: string
  closeoutSubmitted: boolean
  proofObserved: boolean
  rejectionRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}): ArtanisPylonJobVerifyResult => {
  if (
    input.state === 'rejected' ||
    input.state === 'blocked' ||
    input.rejectionRefs.length > 0 ||
    input.blockerRefs.length > 0
  ) {
    return 'fail'
  }
  if (input.closeoutSubmitted && input.proofObserved) {
    return 'pass'
  }
  return 'unknown'
}

// Build a short, public-safe, redacted failure summary from rejection/blocker
// refs only. Returns null when nothing failed. The refs themselves are
// public-safe; the tool defensively re-gates the string before rendering it.
const buildFailureSummary = (input: {
  verifyResult: ArtanisPylonJobVerifyResult
  state: string
  rejectionRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}): string | null => {
  if (input.verifyResult !== 'fail') {
    return null
  }
  const parts: Array<string> = []
  if (input.rejectionRefs.length > 0) {
    parts.push(`rejected (${input.rejectionRefs.slice(0, 5).join(', ')})`)
  }
  if (input.blockerRefs.length > 0) {
    parts.push(`blocked (${input.blockerRefs.slice(0, 5).join(', ')})`)
  }
  if (parts.length === 0) {
    parts.push(`state ${input.state}`)
  }
  return parts.join('; ')
}

// Build the owner-scoped Pylon job-status reader for the `get_pylon_job_status`
// tool. Resolves an assignment ref to its public-safe status, owner-scoped, or
// `null` when the assignment is missing or owned by a different owner.
export const makeArtanisPylonJobStatusReader = (
  deps: ArtanisPylonJobStatusReaderDeps,
): ArtanisPylonJobStatusReader => {
  const eventLimit = deps.eventLimit ?? ARTANIS_JOB_STATUS_EVENT_LIMIT
  const nowIso = deps.nowIso ?? currentIsoTimestamp

  return async (assignmentRef: string): Promise<ArtanisPylonJobStatus | null> => {
    const assignment = await deps.pylonStore.readAssignment(assignmentRef)
    if (assignment === undefined) {
      return null
    }

    // Owner scoping: the assignment must be owned by one of THIS owner's linked
    // agent credentials. Anything else is honest absence, never a cross-owner
    // leak. A resolver failure fails closed (no linked agents -> null).
    const ownerAgentUserIds = await deps
      .listLinkedAgentUserIds(deps.ownerOpenAuthUserId)
      .catch(() => [] as ReadonlyArray<string>)
    if (!ownerAgentUserIds.includes(assignment.ownerAgentUserId)) {
      return null
    }

    const now = nowIso()
    // The projection gives us scanner-safe refs + the lease state. We report the
    // RAW lifecycle state (`assignment.state`) for the `state` field, because the
    // projection conflates an expired lease into a `stale` state — which would
    // hide a real `closeout_submitted`/`rejected` lifecycle behind lease timing.
    const projection = publicPylonApiAssignmentProjection(assignment, now)
    const state = assignment.state
    const events = await deps.pylonStore
      .listEventsForAssignment(assignmentRef, eventLimit)
      .catch(() => [] as ReadonlyArray<PylonApiEventRecord>)

    const blockerRefs = blockerRefsFromEvents(events)
    const proofObserved =
      hasProofEvent(events) || projection.proofRefs.length > 0
    const closeoutSubmitted =
      state === 'closeout_submitted' ||
      events.some(event => event.eventKind === 'worker_closeout')
    const rejectionRefs = projection.rejectionRefs

    const verifyResult = deriveVerifyResult({
      blockerRefs,
      closeoutSubmitted,
      proofObserved,
      rejectionRefs,
      state,
    })
    const failureSummary = buildFailureSummary({
      blockerRefs,
      rejectionRefs,
      state,
      verifyResult,
    })

    return {
      artifactRefs: projection.artifactRefs,
      assignmentRef: projection.assignmentRef,
      blockerRefs,
      closeoutRefs: projection.closeoutRefs,
      closeoutSubmitted,
      failureSummary,
      jobKind: projection.jobKind,
      leaseState: projection.leaseState,
      proofObserved,
      proofRefs: projection.proofRefs,
      rejectionRefs,
      state,
      updatedAt: projection.updatedAtDisplay,
      verifyResult,
    }
  }
}
