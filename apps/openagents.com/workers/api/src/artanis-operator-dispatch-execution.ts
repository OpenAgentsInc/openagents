// Artanis gated Codex dispatch EXECUTION seam (#6366 follow-up).
//
// This is the owner-scoped seam that turns the gated `dispatch_codex_task` tool
// (`artanis-operator-tools.ts`) from PLAN-ONLY into LIVE execution. It is the
// only place that actually CREATES a Khala -> Pylon -> Codex `codex_agent_task`
// assignment from Artanis, and it stays conservative by construction:
//
//   - OWN-CAPACITY ONLY. The assignment is created through the existing
//     `delegateCodingWorkflow` server route (the same seam `/v1/chat/completions`
//     uses for `pylon khala request --workflow codex_agent_task`). That route
//     only ever selects among the CALLER's own linked, heartbeat-fresh,
//     Codex-capable Pylon registrations — never pooled, third-party, or
//     marketplace capacity.
//   - NO SPEND. The coding-delegation path uses `paymentMode: 'unpaid_smoke'`,
//     so no money moves, no payout is granted, and closeout settlement is
//     `not_applicable`.
//   - OWNER-GATED. `isOwnerApproved` reports whether an effective owner approval
//     for `pylon_job_dispatch` exists right now (an approved, non-expired
//     `artanis_approval_gates` row carrying operator approval + authority
//     receipt). Without it the gated tool defers — it never fires.
//   - NEVER FAKE. A `created` result is returned ONLY when the server route
//     actually created an assignment; every other path (no linked agent, no
//     eligible Pylon, gate rejection, store error) maps to a typed rejection
//     that the gated tool surfaces as a deferral with the plan.
//
// No secrets, tokens, prompts, wallet material, or local paths ever enter this
// seam's inputs or outputs; the plan input is already public-safety-gated by the
// tool, and only public-safe refs (assignmentRef, pylonRef, durableRequestId)
// come back out.

import { Effect, Schema as S } from 'effect'

import {
  ArtanisApprovalGateRecord,
  artanisApprovalGateEffective,
} from './artanis-approval-gates'
import { isOpenAgentsOwnerAgentOpenAuthUserId } from './artanis-owner-authority'
import type {
  ArtanisDispatchCreateResult,
  ArtanisDispatchExecution,
  ArtanisDispatchPlanInput,
} from './artanis-operator-tools'
import {
  ARTANIS_REPO_READ_OWNER,
  ARTANIS_REPO_READ_REPO,
} from './artanis-operator-tools'
import { delegateCodingWorkflow } from './inference/coding-workflow-delegation'
import { parseJsonUnknown } from './json-boundary'
import type { PylonApiStore } from './pylon-api'

// Demand attribution + evidence refs for an Artanis-originated dispatch. Public
// safe, no raw material.
const ARTANIS_DISPATCH_EVIDENCE_REF =
  'evidence.artanis.operator_codex_dispatch.own_capacity'

// Decode the wire shape `delegateCodingWorkflow` expects from a public-safe plan.
// No target Pylon ref is set: the server route auto-selects the owner's most
// recent eligible linked Pylon (still strictly own-capacity). A workspace
// (pinned git checkout + verification) is included only when a verify command
// and a resolved commit SHA are both present; otherwise the route runs the
// bounded public sum-repair fixture, which is a real own-capacity no-spend
// `codex_agent_task` assignment.
const buildDelegationBody = (
  plan: ArtanisDispatchPlanInput,
  commitSha: string | undefined,
): Record<string, unknown> => {
  const coding: Record<string, unknown> = {
    objectiveSummary: plan.objective,
  }
  if (plan.verify !== undefined && commitSha !== undefined) {
    coding.workspace = {
      kind: 'git_checkout',
      repository: {
        branch: plan.branch,
        commitSha,
        fullName: `${ARTANIS_REPO_READ_OWNER}/${ARTANIS_REPO_READ_REPO}`,
        provider: 'github',
        visibility: 'public',
      },
      verificationCommand: {
        args: plan.verify.split(/\s+/).filter(arg => arg !== ''),
        commandRef: 'command.public.artanis_dispatch.verify',
      },
    }
  }
  return {
    messages: [{ content: plan.prompt, role: 'user' }],
    model: 'openagents/khala',
    openagents: {
      coding,
      workflowClass: 'codex_agent_task',
    },
    stream: true,
  }
}

export type ArtanisDispatchExecutionDeps = Readonly<{
  // The OpenAuth user id of the authenticated owner (the chat session user id).
  ownerOpenAuthUserId: string
  // The Pylon API store used to read linked registrations + create assignments.
  pylonStore: PylonApiStore
  // Resolve the owner's linked agent user ids (their Pylon-owning credentials).
  listLinkedAgentUserIds: (
    ownerOpenAuthUserId: string,
  ) => Promise<ReadonlyArray<string>>
  // True iff an effective owner approval for `pylon_job_dispatch` exists now.
  readEffectivePylonDispatchApproval: () => Promise<boolean>
  // Deterministic id + clock seams (testable).
  makeId: () => string
  nowIso: () => string
  // Resolve the current commit SHA for a pinned-branch workspace run. Optional;
  // when absent (or it returns undefined) the dispatch runs the bounded fixture.
  resolveCommitSha?: (branch: string) => Promise<string | undefined>
}>

const createAssignmentPromise = async (
  deps: ArtanisDispatchExecutionDeps,
  plan: ArtanisDispatchPlanInput,
): Promise<ArtanisDispatchCreateResult> => {
  try {
    const ownerAgentUserIds = await deps.listLinkedAgentUserIds(
      deps.ownerOpenAuthUserId,
    )
    if (ownerAgentUserIds.length === 0) {
      return { kind: 'rejected', reason: 'no_linked_agents' }
    }
    const linkedAgents = ownerAgentUserIds.map(agentUserId => ({ agentUserId }))

    const requestId = deps.makeId()
    const nowIso = deps.nowIso()
    const commitSha =
      plan.verify !== undefined && deps.resolveCommitSha !== undefined
        ? await deps.resolveCommitSha(plan.branch).catch(() => undefined)
        : undefined

    const result = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: [ARTANIS_DISPATCH_EVIDENCE_REF],
        workflowClass: 'codex_agent_task',
      },
      linkedAgents,
      makeId: deps.makeId,
      nowIso,
      pylonStore: deps.pylonStore,
      rawBody: buildDelegationBody(plan, commitSha),
      requestId,
    })

    // `null` means no eligible linked Pylon (none active/heartbeat-fresh/
    // Codex-capable/available) — honest absence, mapped to a deferral.
    if (result === null) {
      return { kind: 'rejected', reason: 'no_eligible_linked_pylon' }
    }
    if (result.kind === 'rejected') {
      return { kind: 'rejected', reason: result.error }
    }
    return {
      assignmentRef: result.assignment.assignmentRef,
      durableRequestId: requestId,
      kind: 'created',
      pylonRef: result.pylon.pylonRef,
    }
  } catch {
    return { kind: 'rejected', reason: 'dispatch_execution_error' }
  }
}

// Build the owner-scoped execution seam for the gated dispatch tool.
export const makeArtanisDispatchExecution = (
  deps: ArtanisDispatchExecutionDeps,
): ArtanisDispatchExecution => ({
  createCodexAssignment: (plan: ArtanisDispatchPlanInput) =>
    Effect.tryPromise({
      catch: () =>
        ({ kind: 'rejected', reason: 'dispatch_execution_error' }) as const,
      try: () => createAssignmentPromise(deps, plan),
    }).pipe(
      Effect.orElseSucceed(
        () => ({ kind: 'rejected', reason: 'dispatch_execution_error' }) as const,
      ),
    ),
  isOwnerApproved: () =>
    Effect.tryPromise({
      catch: () => false,
      try: () => deps.readEffectivePylonDispatchApproval(),
    }).pipe(Effect.orElseSucceed(() => false)),
})

// Read whether an effective owner approval for `pylon_job_dispatch` exists in
// the persisted Artanis approval-gate ledger. An effective gate is an approved,
// non-expired, non-superseded `pylon_job_dispatch` gate carrying operator
// approval as the authority source plus an authority receipt ref. This is the
// SINGLE thing that flips the gated dispatch from "deferred" to "live": once
// such a row exists in `artanis_approval_gates`, the next owner dispatch fires.
// Conservative + fail-soft: any read/decode failure reads as "not approved".
export const readEffectiveArtanisPylonDispatchApproval = async (
  db: D1Database,
  nowIso: string,
): Promise<boolean> => {
  // `artanis_approval_gates` is the persisted Artanis approval-gate table
  // (`tableSpecs.approval_gate` in `artanis-persistence.ts`); `record_json`
  // holds the serialized `ArtanisApprovalGateRecord`.
  const result = await db
    .prepare(
      `SELECT record_json
         FROM artanis_approval_gates
        ORDER BY updated_at DESC
        LIMIT 50`,
    )
    .all<{ record_json: string }>()
    .catch(() => ({ results: [] as ReadonlyArray<{ record_json: string }> }))

  for (const row of result.results ?? []) {
    try {
      const parsed = parseJsonUnknown(row.record_json)
      const record = S.decodeUnknownSync(ArtanisApprovalGateRecord)(parsed)
      if (
        record.kind === 'pylon_job_dispatch' &&
        artanisApprovalGateEffective(record, nowIso)
      ) {
        return true
      }
    } catch {
      // Undecodable/legacy row — skip it; never treat it as an approval.
    }
  }
  return false
}


// Owner-promotion-aware effective-approval read (owner-directed 2026-06-27,
// epic #6359). This is the seam the LIVE route now uses. It flips the gated
// `dispatch_codex_task` tool from "deferred" to "live" when EITHER:
//
//   (a) the authenticated owner is an OWNER-PROMOTED operator agent (Artanis)
//       — he carries a STANDING owner approval for his own `pylon_job_dispatch`
//       actions, recorded under
//       `ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF`
//       ("owner promotion by Chris, 2026-06-27"), so his own-capacity, no-spend
//       Codex dispatch EXECUTES without a separately-armed `artanis_approval_gates`
//       row; OR
//   (b) an effective `pylon_job_dispatch` gate exists in `artanis_approval_gates`
//       for any other owner (the original armed-gate path, unchanged).
//
// NEVER-WAIVABLE BOUNDS: the standing promotion approves `pylon_job_dispatch`
// ONLY. It does not touch `wallet_spend`, `settlement`, `l402_redemption`, or any
// money-movement/payout-bearing kind — those stay gated and still require an
// explicit effective approval. The dispatch path itself remains own-capacity +
// `unpaid_smoke` no-spend by construction (`createAssignmentPromise` above), so
// the promotion grants no payout authority and invents no custody path.
export const readEffectiveArtanisPylonDispatchApprovalForOwner = async (
  db: D1Database,
  nowIso: string,
  ownerOpenAuthUserId: string,
): Promise<boolean> => {
  // (a) Standing owner-promotion approval — scoped to pylon_job_dispatch only.
  // Recorded under ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF (see
  // artanis-owner-authority.ts).
  if (isOpenAgentsOwnerAgentOpenAuthUserId(ownerOpenAuthUserId)) {
    return true
  }
  // (b) Otherwise fall back to the armed D1 approval-gate path.
  return readEffectiveArtanisPylonDispatchApproval(db, nowIso)
}
