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
//   - VERIFIED REPO WORK ONLY. Artanis dispatch must carry a bounded public
//     verification command and a resolved commit SHA. If either is absent, this
//     seam rejects instead of falling back to the fixture path.
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
  authorizeCommandProposal,
  evaluateCommandSourceVerified,
} from '@openagentsinc/blueprint-contracts'
import {
  ArtanisApprovalGateRecord,
  type ArtanisRiskyActionKind,
  artanisApprovalGateEffective,
} from './artanis-approval-gates'
import {
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  artanisAuthorityScopeAllowsOwnerLinkedCapacity,
  artanisAuthorityScopeEvidenceRef,
  type ArtanisAuthorityScope,
} from './artanis-authority-scope'
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

const verifyCommandAuthorized = (verify: string): boolean => {
  const isApiTestCommand =
    /^bun\s+run\s+--cwd\s+apps\/openagents\.com\/workers\/api\s+test(?:\s|$)/.test(
      verify,
    )
  const decision = authorizeCommandProposal(
    evaluateCommandSourceVerified({
      commandString: verify,
      declaredFlags: isApiTestCommand ? ['--cwd'] : [],
      dryRunExitCode: isApiTestCommand ? 0 : null,
      expectedFlags: ['--cwd'],
      scriptPath: isApiTestCommand
        ? 'apps/openagents.com/workers/api/package.json'
        : 'unknown',
      sourceReadHash: isApiTestCommand
        ? 'source.public.apps.openagents.com.workers.api.package_json.test'
        : null,
    }),
  )
  return decision.ok
}

// Decode the wire shape `delegateCodingWorkflow` expects from a public-safe plan.
// No target Pylon ref is set: the server route auto-selects the owner's most
// recent eligible linked Pylon (still strictly own-capacity). A workspace
// (pinned git checkout + verification) is included only when a verify command
// and a resolved commit SHA are both present. The caller must enforce that
// before invoking this helper; no Artanis live dispatch is allowed to fall back
// to the fixture path.
const buildDelegationBody = (
  plan: ArtanisDispatchPlanInput & Readonly<{ verify: string }>,
  commitSha: string,
): Record<string, unknown> => {
  const coding: Record<string, unknown> = {
    authorityScope: plan.authorityScope,
    fleetRunIntent: {
      controlRefs: plan.fleetRunPlan.controlRefs,
      evidenceRefs: plan.fleetRunPlan.evidenceRefs,
      runRef: plan.fleetRunPlan.runRef,
      targetConcurrency: plan.fleetRunPlan.targetConcurrency,
      workerKind: plan.fleetRunPlan.workerKind,
      workSourceRef: plan.fleetRunPlan.workSourceRef,
    },
    objectiveSummary: plan.objective,
    spawnRunRef: plan.fleetRunPlan.runRef,
    spawnWorkerRef: plan.fleetRunPlan.workerRef,
  }
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
  return {
    messages: [{ content: plan.prompt, role: 'user' }],
    model: 'openagents/khala',
    openagents: {
      authorityScope: plan.authorityScope,
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
  readEffectivePylonDispatchApproval: (
    authorityScope: ArtanisAuthorityScope,
  ) => Promise<boolean>
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
    if (plan.verify === undefined) {
      return { kind: 'rejected', reason: 'verification_required' }
    }
    if (!verifyCommandAuthorized(plan.verify)) {
      return { kind: 'rejected', reason: 'command_source_not_verified' }
    }
    const verifiedPlan = { ...plan, verify: plan.verify }
    if (!artanisAuthorityScopeAllowsOwnerLinkedCapacity(plan.authorityScope)) {
      return { kind: 'rejected', reason: 'authority_scope_capacity_unavailable' }
    }
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
      deps.resolveCommitSha === undefined
        ? undefined
        : await deps.resolveCommitSha(verifiedPlan.branch).catch(() => undefined)
    if (commitSha === undefined) {
      return { kind: 'rejected', reason: 'verification_workspace_unavailable' }
    }

    const result = await delegateCodingWorkflow({
      classification: {
        confidence: 1,
        evidenceRefs: [
          ARTANIS_DISPATCH_EVIDENCE_REF,
          artanisAuthorityScopeEvidenceRef(verifiedPlan.authorityScope),
          ...verifiedPlan.fleetRunPlan.evidenceRefs,
        ],
        workflowClass: 'codex_agent_task',
      },
      authorityScope: verifiedPlan.authorityScope,
      linkedAgents,
      makeId: deps.makeId,
      nowIso,
      pylonStore: deps.pylonStore,
      rawBody: buildDelegationBody(verifiedPlan, commitSha),
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
  isOwnerApproved: (authorityScope: ArtanisAuthorityScope) =>
    Effect.tryPromise({
      catch: () => false,
      try: () => deps.readEffectivePylonDispatchApproval(authorityScope),
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
  authorityScope: ArtanisAuthorityScope = ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
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
        record.authorityScope === authorityScope &&
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

export const readEffectiveArtanisApproval = async (
  db: D1Database,
  nowIso: string,
  kind: ArtanisRiskyActionKind,
  authorityScope?: ArtanisAuthorityScope | undefined,
): Promise<boolean> => {
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
        record.kind === kind &&
        (authorityScope === undefined || record.authorityScope === authorityScope) &&
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


// Owner-aware effective-approval read (owner-directed 2026-06-27, epic #6359;
// generalized for #6382). This is the seam the LIVE route now uses. It flips
// the gated `dispatch_codex_task` tool from "deferred" to "live" when EITHER:
//
//   (a) the authenticated owner has a non-empty owner scope. Every tenant has a
//       STANDING approval for their own `pylon_job_dispatch` actions, so their
//       own-capacity, no-spend Codex dispatch EXECUTES without a separately
//       armed `artanis_approval_gates` row; OR
//   (b) an effective `pylon_job_dispatch` gate exists in `artanis_approval_gates`
//       (legacy armed-gate compatibility).
//
// NEVER-WAIVABLE BOUNDS: the standing tenant approval approves
// `pylon_job_dispatch` ONLY. It does not touch `wallet_spend`, `settlement`,
// `l402_redemption`, or any money-movement/payout-bearing kind — those stay
// gated and still require an explicit effective approval. The dispatch path
// itself remains own-capacity + `unpaid_smoke` no-spend by construction
// (`createAssignmentPromise` above), so the tenant approval grants no payout
// authority and invents no custody path.
export const readEffectiveArtanisPylonDispatchApprovalForOwner = async (
  db: D1Database,
  nowIso: string,
  ownerOpenAuthUserId: string,
  authorityScope: ArtanisAuthorityScope = ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
): Promise<boolean> => {
  if (!artanisAuthorityScopeAllowsOwnerLinkedCapacity(authorityScope)) {
    return false
  }
  // (a) Standing per-tenant approval — scoped to pylon_job_dispatch only. The
  // owner-promoted Artanis identity is included by this tenant-wide rule.
  if (ownerOpenAuthUserId.trim() !== '') {
    return true
  }
  // (b) Otherwise fall back to the armed D1 approval-gate path.
  return readEffectiveArtanisPylonDispatchApproval(db, nowIso, authorityScope)
}
