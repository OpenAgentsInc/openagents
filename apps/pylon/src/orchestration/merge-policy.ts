import { createHash } from "node:crypto"
import { Schema as S } from "effect"

import type { CreateWorkClaimInput, OrchestrationTaskSpec, WorkClaim } from "./store.js"

export const CLOSEOUT_REVIEW_GATE_SCHEMA = "openagents.khala_code.closeout_review_gate.v1" as const
export const MERGE_POLICY_SCHEMA = "openagents.khala_code.merge_policy.v1" as const
export const MERGE_WAVE_RESOLVER_SCHEMA = "openagents.khala_code.merge_wave_resolver.v1" as const

export const CloseoutReviewStatusSchema = S.Literals(["ready_for_review", "blocked"])
export type CloseoutReviewStatus = typeof CloseoutReviewStatusSchema.Type

export const VerifyCommandStatusSchema = S.Literals(["green", "red", "missing"])
export type VerifyCommandStatus = typeof VerifyCommandStatusSchema.Type

export const VerifyWorkspaceKindSchema = S.Literals(["worker_workspace", "other"])
export type VerifyWorkspaceKind = typeof VerifyWorkspaceKindSchema.Type

export type VerifyCommandEvidence = {
  commandRef: string
  status: VerifyCommandStatus
  exitCode: number | null
  workspaceKind: VerifyWorkspaceKind
  evidenceRef: string
}

export type CloseoutReviewGateInput = {
  pinnedVerifyCommandRef: string
  claim: WorkClaim | null
  verify: VerifyCommandEvidence | null
  now?: Date
}

export type CloseoutReviewGate = {
  schema: typeof CLOSEOUT_REVIEW_GATE_SCHEMA
  status: CloseoutReviewStatus
  readyForReview: boolean
  claimRef: string | null
  verifyRef: string | null
  refs: string[]
  blockerRefs: string[]
}

export const MergeModeSchema = S.Literals(["manual_review", "auto_merge_clean"])
export type MergeMode = typeof MergeModeSchema.Type

export type MergePolicyInput = {
  mode?: MergeMode
  closeout: CloseoutReviewGate
  mergeable: boolean
  verifyGreen: boolean
  hasConflicts: boolean
  diffWithinScope: boolean
  siblingConflictRefs?: readonly string[]
}

export type MergePolicyDecisionAction =
  | "manual_review"
  | "auto_merge"
  | "merge_wave_resolver"
  | "hold_for_verification"

export type MergePolicyDecision = {
  schema: typeof MERGE_POLICY_SCHEMA
  mode: MergeMode
  action: MergePolicyDecisionAction
  ownerToggleRequired: boolean
  refs: string[]
  blockerRefs: string[]
}

export type MergeWaveResolverJob = {
  schema: typeof MERGE_WAVE_RESOLVER_SCHEMA
  waveRef: string
  workUnitRef: string
  sequential: true
  execution: "owner_toggle_required"
  siblingRefs: readonly string[]
  claim: CreateWorkClaimInput
  taskSpec: OrchestrationTaskSpec
  refs: string[]
}

const publicRef = (prefix: string, value: string): string =>
  value.startsWith(`${prefix}.`)
    ? value
    : `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

const isLiveClaim = (claim: WorkClaim, now: Date): boolean =>
  (claim.state === "claimed" || claim.state === "in_progress" || claim.state === "closeout") &&
  Date.parse(claim.expiresAt) > now.getTime()

export function evaluateCloseoutReviewGate(input: CloseoutReviewGateInput): CloseoutReviewGate {
  const now = input.now ?? new Date()
  const refs: string[] = []
  const blockerRefs: string[] = []

  if (input.claim === null) {
    blockerRefs.push("blocker.public.pylon.closeout.claim_missing")
  } else {
    refs.push(publicRef("claim.public.pylon", input.claim.claimRef))
    if (!isLiveClaim(input.claim, now)) {
      blockerRefs.push("blocker.public.pylon.closeout.claim_not_live")
    }
  }

  if (input.verify === null) {
    blockerRefs.push("blocker.public.pylon.closeout.verify_missing")
  } else {
    refs.push(publicRef("verify.public.pylon", input.verify.evidenceRef))
    if (input.verify.commandRef !== input.pinnedVerifyCommandRef) {
      blockerRefs.push("blocker.public.pylon.closeout.verify_command_mismatch")
    }
    if (input.verify.workspaceKind !== "worker_workspace") {
      blockerRefs.push("blocker.public.pylon.closeout.verify_not_worker_workspace")
    }
    if (input.verify.status !== "green" || input.verify.exitCode !== 0) {
      blockerRefs.push("blocker.public.pylon.closeout.verify_not_green")
    }
  }

  const readyForReview = blockerRefs.length === 0
  return {
    schema: CLOSEOUT_REVIEW_GATE_SCHEMA,
    status: readyForReview ? "ready_for_review" : "blocked",
    readyForReview,
    claimRef: input.claim === null ? null : publicRef("claim.public.pylon", input.claim.claimRef),
    verifyRef: input.verify === null ? null : publicRef("verify.public.pylon", input.verify.evidenceRef),
    refs,
    blockerRefs,
  }
}

export function decideMergePolicy(input: MergePolicyInput): MergePolicyDecision {
  const mode = input.mode ?? "manual_review"
  const refs = [
    `merge-policy.public.pylon.${mode}`,
    `closeout-status.public.pylon.${input.closeout.status}`,
    ...input.closeout.refs,
  ]
  const blockerRefs = [...input.closeout.blockerRefs]

  if (!input.closeout.readyForReview || !input.verifyGreen) {
    return {
      schema: MERGE_POLICY_SCHEMA,
      mode,
      action: "hold_for_verification",
      ownerToggleRequired: false,
      refs,
      blockerRefs: [...blockerRefs, "blocker.public.pylon.merge.verify_required"],
    }
  }

  if (input.hasConflicts || input.siblingConflictRefs?.length) {
    return {
      schema: MERGE_POLICY_SCHEMA,
      mode,
      action: "merge_wave_resolver",
      ownerToggleRequired: true,
      refs: [...refs, "merge-wave.public.pylon.required"],
      blockerRefs,
    }
  }

  if (mode === "auto_merge_clean" && input.mergeable && input.diffWithinScope) {
    return {
      schema: MERGE_POLICY_SCHEMA,
      mode,
      action: "auto_merge",
      ownerToggleRequired: true,
      refs: [...refs, "merge-action.public.pylon.auto_merge_clean"],
      blockerRefs,
    }
  }

  return {
    schema: MERGE_POLICY_SCHEMA,
    mode,
    action: "manual_review",
    ownerToggleRequired: false,
    refs,
    blockerRefs,
  }
}

export function createMergeWaveResolverJob(input: {
  waveRef: string
  runRef: string
  siblingRefs: readonly string[]
  workerAccountRef: string
  claimRef?: string
  ttl: number
  now?: Date
}): MergeWaveResolverJob {
  const waveRef = publicRef("merge_wave.public.pylon", input.waveRef)
  const workUnitRef = publicRef("work_unit.public.pylon.merge_wave", `${input.runRef}:${waveRef}`)
  const claimRef = input.claimRef ?? publicRef("claim.public.pylon.merge_wave", `${workUnitRef}:${input.workerAccountRef}`)
  const siblingRefs = input.siblingRefs.map((ref) => publicRef("sibling.public.pylon", ref))
  const taskSpec: OrchestrationTaskSpec = {
    title: `Merge-wave resolver ${waveRef}`,
    prompt: [
      `Resolve merge wave ${waveRef}.`,
      "Sequential semantics are data: rebase, verify, and order sibling PRs one at a time.",
      "Do not execute an actual merge unless the owner has toggled merge execution on.",
      `Sibling refs: ${siblingRefs.join(", ")}.`,
    ].join("\n"),
    runnerKind: "codex",
    fleetRunRef: input.runRef,
    issueRef: waveRef,
  }

  return {
    schema: MERGE_WAVE_RESOLVER_SCHEMA,
    waveRef,
    workUnitRef,
    sequential: true,
    execution: "owner_toggle_required",
    siblingRefs,
    claim: {
      claimRef,
      workUnitRef,
      runRef: input.runRef,
      assignmentRef: null,
      workerAccountRef: input.workerAccountRef,
      ttl: input.ttl,
      now: input.now,
    },
    taskSpec,
    refs: [waveRef, workUnitRef, claimRef, "merge-wave-semantics.public.pylon.sequential"],
  }
}
