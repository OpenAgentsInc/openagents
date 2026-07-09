/**
 * TypeScript Effect Schema mirrors for OpenAgents Cloud contracts.
 * Hand-maintained alongside crates/openagents-cloud-contract (MH/cloud #8591).
 * Do not embed secrets, host topology, or live tokens in fixtures decoded here.
 */
import { Schema as S } from "effect"

export const CODEX_PLACEMENT_ASSIGNMENT_VERSION =
  "openagents.codex_placement_assignment.v1" as const
export const CLOUD_VM_PROVISIONER_VERSION =
  "openagents.cloud_vm_provisioner.v1" as const
export const GCE_CAPACITY_CLASS_VERSION =
  "openagents.gce_capacity_class.v1" as const
export const RESOURCE_USAGE_RECEIPT_VERSION =
  "openagents.resource_usage_receipt.v1" as const
export const AGENT_COMPUTER_ISOLATION_POLICY_VERSION =
  "openagents.agent_computer_isolation_policy.v1" as const

/** Public-safe ref string — never a secret or host path. */
export const PublicSafeRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(512),
)

export const ComputeLane = S.Literals([
  "auto",
  "cloud_gcp",
  "shc",
  "local",
  "cloud_vm",
])
export type ComputeLane = typeof ComputeLane.Type

export const CodexPlacementAssignmentSchema = S.Struct({
  schema: S.Literal(CODEX_PLACEMENT_ASSIGNMENT_VERSION),
  assignmentRef: PublicSafeRef,
  workContextRef: S.optionalKey(PublicSafeRef),
  requestedLane: S.optionalKey(ComputeLane),
  pinnedLane: S.optionalKey(ComputeLane),
  objective: S.optionalKey(S.String),
  repositoryRef: S.optionalKey(PublicSafeRef),
  /** Refs only — never raw auth JSON. */
  grantRefs: S.optionalKey(S.Array(PublicSafeRef)),
})
export type CodexPlacementAssignment = typeof CodexPlacementAssignmentSchema.Type

export const CloudVmSessionHandleSchema = S.Struct({
  schema: S.Literal(CLOUD_VM_PROVISIONER_VERSION),
  sessionRef: PublicSafeRef,
  osTier: S.optionalKey(S.String),
  /** Limits and policy labels only — no KVM socket / guest IP / rootfs path. */
  limits: S.optionalKey(
    S.Struct({
      vcpus: S.optionalKey(S.Number),
      memoryMb: S.optionalKey(S.Number),
      timeoutMs: S.optionalKey(S.Number),
    }),
  ),
})
export type CloudVmSessionHandle = typeof CloudVmSessionHandleSchema.Type

export const GceCapacityClassSchema = S.Struct({
  schema: S.Literal(GCE_CAPACITY_CLASS_VERSION),
  capacityClassId: PublicSafeRef,
  machineFamily: S.optionalKey(S.String),
  /** Catalog cost basis labels only — not live billing credentials. */
  costBasis: S.optionalKey(S.String),
})
export type GceCapacityClass = typeof GceCapacityClassSchema.Type

export const ResourceUsageReceiptSchema = S.Struct({
  schema: S.Literal(RESOURCE_USAGE_RECEIPT_VERSION),
  receiptRef: PublicSafeRef,
  workContextRef: S.optionalKey(PublicSafeRef),
  compute: S.optionalKey(
    S.Struct({
      lane: S.optionalKey(ComputeLane),
      wallClockMs: S.optionalKey(S.Number),
      vcpuSeconds: S.optionalKey(S.Number),
      costMicros: S.optionalKey(S.Number),
      costUnavailableReason: S.optionalKey(S.String),
    }),
  ),
  tokens: S.optionalKey(
    S.Struct({
      inputTokens: S.optionalKey(S.Number),
      outputTokens: S.optionalKey(S.Number),
      totalTokens: S.optionalKey(S.Number),
      unavailableReason: S.optionalKey(S.String),
    }),
  ),
  cleanup: S.optionalKey(
    S.Struct({
      scratchWipeReceiptRef: S.optionalKey(PublicSafeRef),
      microvmDestroyReceiptRef: S.optionalKey(PublicSafeRef),
    }),
  ),
})
export type ResourceUsageReceipt = typeof ResourceUsageReceiptSchema.Type

export const AgentComputerIsolationPolicySchema = S.Struct({
  schema: S.Literal(AGENT_COMPUTER_ISOLATION_POLICY_VERSION),
  policyRef: PublicSafeRef,
  oneWorkContextPerComputer: S.Boolean,
  noCrossContextReuse: S.Boolean,
  scmBrokerOnlyCredentials: S.Boolean,
  requireScratchWipeReceipt: S.Boolean,
  requireMicrovmDestroyReceipt: S.Boolean,
  walletAuthority: S.Literal("none"),
})
export type AgentComputerIsolationPolicy =
  typeof AgentComputerIsolationPolicySchema.Type

export const DEFAULT_AGENT_COMPUTER_ISOLATION_POLICY: AgentComputerIsolationPolicy =
  {
    schema: AGENT_COMPUTER_ISOLATION_POLICY_VERSION,
    policyRef: "policy.agent_computer.isolation.default",
    oneWorkContextPerComputer: true,
    noCrossContextReuse: true,
    scmBrokerOnlyCredentials: true,
    requireScratchWipeReceipt: true,
    requireMicrovmDestroyReceipt: true,
    walletAuthority: "none",
  }
