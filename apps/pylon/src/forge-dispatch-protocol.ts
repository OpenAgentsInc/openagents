import type {
  ForgeDispatchCloseout,
  ForgeDispatchDecision,
  ForgeDispatchVerificationCommand,
  ForgeDispatchWorkItem,
} from "@openagentsinc/forge-protocol"
import type {
  AssignmentCloseout,
  PylonAssignmentLease,
} from "./assignment.js"

export const FORGE_PYLON_DISPATCH_BACKEND_REF = "forge.openagents.com"

export type ForgeDispatchPylonCodingAssignment = Readonly<{
  schema: "openagents.forge.pylon_dispatch.coding_assignment.v0.1"
  tenantRef: string
  dispatchRef: string
  issueRef: string | null
  objectiveRef: string
  objectiveSummary: string
  workClass: ForgeDispatchWorkItem["work_class"]
  git: Readonly<{
    repositoryRef: string
    remoteUrl: string
    baseRef: string
    baseHead: string
    branchRef: string
    receivePackRef: string
    gitAccess: Readonly<{
      tokenRef: string
      tokenPrefix: string
      scopes: string[]
      expiresAt: string
      delivery: string
    }>
  }>
  verificationCommand: Readonly<{
    commandRef: string
    runnerRef: string
    workingDirectory: string
    args: string[]
    timeoutSeconds: number
  }> | null
  sourceRefs: string[]
}>

export type ForgeDispatchDecisionForPylonInput = Readonly<{
  item: ForgeDispatchWorkItem
  pylonRef: string
  state: ForgeDispatchDecision["state"]
  observedAt: string
  blockerRefs?: readonly string[]
  sourceRefs?: readonly string[]
}>

export type PylonCloseoutToForgeDispatchInput = Readonly<{
  item: ForgeDispatchWorkItem
  pylonRef: string
  closeout: AssignmentCloseout
  changeRef?: string | null
  packfileRef?: string | null
  verificationRef?: string | null
  sourceRefs?: readonly string[]
}>

const mergedRefs = (
  baseRefs: readonly string[],
  extraRefs: readonly string[] = [],
): string[] => [...new Set([...baseRefs, ...extraRefs])]

const forgeDispatchVerificationCommandForPylon = (
  command: ForgeDispatchVerificationCommand | null,
): ForgeDispatchPylonCodingAssignment["verificationCommand"] =>
  command === null
    ? null
    : {
        commandRef: command.command_ref,
        runnerRef: command.runner_ref,
        workingDirectory: command.working_directory,
        args: [...command.args],
        timeoutSeconds: command.timeout_seconds,
      }

export const forgeDispatchCodingAssignmentFromWorkItem = (
  item: ForgeDispatchWorkItem,
): ForgeDispatchPylonCodingAssignment => ({
  schema: "openagents.forge.pylon_dispatch.coding_assignment.v0.1",
  tenantRef: item.tenant_ref,
  dispatchRef: item.dispatch_ref,
  issueRef: item.issue_ref,
  objectiveRef: item.objective_ref,
  objectiveSummary: item.objective_summary,
  workClass: item.work_class,
  git: {
    repositoryRef: item.git.repository_ref,
    remoteUrl: item.git.remote_url,
    baseRef: item.git.base_ref,
    baseHead: item.git.base_head,
    branchRef: item.git.branch_ref,
    receivePackRef: item.git.receive_pack_ref,
    gitAccess: {
      tokenRef: item.git.git_access.token_ref,
      tokenPrefix: item.git.git_access.token_prefix,
      scopes: [...item.git.git_access.scopes],
      expiresAt: item.git.git_access.expires_at,
      delivery: item.git.git_access.delivery,
    },
  },
  verificationCommand: forgeDispatchVerificationCommandForPylon(
    item.verification_command,
  ),
  sourceRefs: [...item.source_refs],
})

export const forgeDispatchWorkItemToPylonLease = (
  item: ForgeDispatchWorkItem,
): PylonAssignmentLease => ({
  schema: "openagents.pylon.assignment_lease.v0.3",
  assignmentRef: item.work_ref,
  leaseRef: item.lease_ref,
  goal: item.objective_summary,
  paymentMode: item.payment_mode,
  capabilityRefs: [...item.capability_refs],
  codingAssignment: forgeDispatchCodingAssignmentFromWorkItem(
    item,
  ) as Readonly<Record<string, unknown>>,
  backendRef: FORGE_PYLON_DISPATCH_BACKEND_REF,
  expiresAt: item.expires_at,
  createdAt: item.created_at,
})

export const forgeDispatchDecisionForPylon = ({
  item,
  pylonRef,
  state,
  observedAt,
  blockerRefs = [],
  sourceRefs = [],
}: ForgeDispatchDecisionForPylonInput): ForgeDispatchDecision => ({
  schema: "openagents.forge.dispatch.decision.v0.1",
  tenant_ref: item.tenant_ref,
  dispatch_ref: item.dispatch_ref,
  work_ref: item.work_ref,
  lease_ref: item.lease_ref,
  pylon_ref: pylonRef,
  state,
  accepted_at: state === "accepted" ? observedAt : null,
  rejected_at: state === "rejected" ? observedAt : null,
  blocker_refs: [...blockerRefs],
  source_refs: mergedRefs(item.source_refs, sourceRefs),
})

const assertCloseoutMatchesWorkItem = (
  closeout: AssignmentCloseout,
  item: ForgeDispatchWorkItem,
): void => {
  if (closeout.assignmentRef !== item.work_ref) {
    throw new Error(
      `Forge dispatch closeout assignment mismatch: ${closeout.assignmentRef} !== ${item.work_ref}`,
    )
  }
  if (closeout.leaseRef !== item.lease_ref) {
    throw new Error(
      `Forge dispatch closeout lease mismatch: ${closeout.leaseRef} !== ${item.lease_ref}`,
    )
  }
}

export const pylonCloseoutToForgeDispatchCloseout = ({
  item,
  pylonRef,
  closeout,
  changeRef = null,
  packfileRef = null,
  verificationRef = null,
  sourceRefs = [],
}: PylonCloseoutToForgeDispatchInput): ForgeDispatchCloseout => {
  assertCloseoutMatchesWorkItem(closeout, item)

  return {
    schema: "openagents.forge.dispatch.closeout.v0.1",
    tenant_ref: item.tenant_ref,
    dispatch_ref: item.dispatch_ref,
    work_ref: item.work_ref,
    lease_ref: item.lease_ref,
    pylon_ref: pylonRef,
    status: closeout.status,
    payment_mode: closeout.paymentMode,
    settlement_state: closeout.settlementState,
    payout_claim_allowed: closeout.payoutClaimAllowed,
    change_ref: changeRef,
    packfile_ref: packfileRef,
    verification_ref: verificationRef,
    artifact_refs: [...closeout.artifactRefs],
    blocker_refs: [...closeout.blockerRefs],
    build_refs: [...closeout.buildRefs],
    closeout_refs: [...closeout.closeoutRefs],
    preview_refs: [...closeout.previewRefs],
    proof_refs: [...closeout.proofRefs],
    receipt_refs: [...closeout.receiptRefs],
    result_refs: [...closeout.resultRefs],
    summary_refs: [...closeout.summaryRefs],
    test_refs: [...closeout.testRefs],
    source_refs: mergedRefs(item.source_refs, sourceRefs),
    redacted: true,
    completed_at: closeout.completedAt,
  }
}
