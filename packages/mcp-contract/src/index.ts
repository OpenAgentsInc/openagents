import { Schema as S } from "effect"

export const OpenAgentsMcpSchemaVersion = S.Literal("openagents.mcp.phase0.v1")
export type OpenAgentsMcpSchemaVersion = typeof OpenAgentsMcpSchemaVersion.Type

export const OPENAGENTS_MCP_SCHEMA_VERSION: OpenAgentsMcpSchemaVersion =
  "openagents.mcp.phase0.v1"

export const OpenAgentsMcpContractStatus = S.Struct({
  schemaVersion: OpenAgentsMcpSchemaVersion,
  packageName: S.Literal("@openagentsinc/mcp-contract"),
  phase: S.Literal("phase_0_contract_groundwork"),
  runtimeTransportExposed: S.Boolean,
})
export type OpenAgentsMcpContractStatus = typeof OpenAgentsMcpContractStatus.Type

export const openAgentsMcpContractStatus: OpenAgentsMcpContractStatus = {
  schemaVersion: OPENAGENTS_MCP_SCHEMA_VERSION,
  packageName: "@openagentsinc/mcp-contract",
  phase: "phase_0_contract_groundwork",
  runtimeTransportExposed: false,
}

export const decodeOpenAgentsMcpContractStatus = S.decodeUnknownSync(
  OpenAgentsMcpContractStatus,
)

export const OpenAgentsMcpAuthorityClass = S.Literals([
  "public_read",
  "operator_read",
  "private_account_read",
  "workspace_read",
  "workspace_write",
  "local_node_control",
  "coding_session_control",
  "approval_resolution",
  "payment_read",
  "payment_receive",
  "payment_spend",
  "deployment",
  "admin",
])
export type OpenAgentsMcpAuthorityClass = typeof OpenAgentsMcpAuthorityClass.Type

export const openAgentsMcpAuthorityClasses: ReadonlyArray<OpenAgentsMcpAuthorityClass> = [
  "public_read",
  "operator_read",
  "private_account_read",
  "workspace_read",
  "workspace_write",
  "local_node_control",
  "coding_session_control",
  "approval_resolution",
  "payment_read",
  "payment_receive",
  "payment_spend",
  "deployment",
  "admin",
]

export const openAgentsMcpHighRiskAuthorityClasses: ReadonlyArray<OpenAgentsMcpAuthorityClass> = [
  "workspace_write",
  "payment_spend",
  "deployment",
  "admin",
]

export const isOpenAgentsMcpHighRiskAuthority = (
  authorityClass: OpenAgentsMcpAuthorityClass,
): boolean => openAgentsMcpHighRiskAuthorityClasses.includes(authorityClass)

export const OpenAgentsMcpGrantDecision = S.Literals([
  "granted",
  "denied",
  "revoked",
  "expired",
  "blocked_by_policy",
])
export type OpenAgentsMcpGrantDecision = typeof OpenAgentsMcpGrantDecision.Type

export const OpenAgentsMcpGrant = S.Struct({
  grantRef: S.String,
  subjectRef: S.String,
  authorityClass: OpenAgentsMcpAuthorityClass,
  decision: OpenAgentsMcpGrantDecision,
  scopeRefs: S.Array(S.String),
  grantedAt: S.String,
  expiresAt: S.optional(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpGrant = typeof OpenAgentsMcpGrant.Type

export const decodeOpenAgentsMcpGrant = S.decodeUnknownSync(OpenAgentsMcpGrant)

export type OpenAgentsMcpGrantFilteredDescriptor = Readonly<{
  name: string
  requiredAuthorities: ReadonlyArray<OpenAgentsMcpAuthorityClass>
}>

export const openAgentsMcpGrantedAuthoritySet = (
  grants: ReadonlyArray<OpenAgentsMcpGrant>,
): ReadonlySet<OpenAgentsMcpAuthorityClass> =>
  new Set(
    grants
      .filter((grant) => grant.decision === "granted")
      .map((grant) => grant.authorityClass),
  )

export const openAgentsMcpDescriptorIsGranted = (
  descriptor: OpenAgentsMcpGrantFilteredDescriptor,
  grantedAuthorities: ReadonlySet<OpenAgentsMcpAuthorityClass>,
): boolean =>
  descriptor.requiredAuthorities.every((authorityClass) =>
    grantedAuthorities.has(authorityClass),
  )

export const filterOpenAgentsMcpDescriptorsByGrantSet = <
  Descriptor extends OpenAgentsMcpGrantFilteredDescriptor,
>(
  descriptors: ReadonlyArray<Descriptor>,
  grants: ReadonlyArray<OpenAgentsMcpGrant>,
): ReadonlyArray<Descriptor> => {
  const grantedAuthorities = openAgentsMcpGrantedAuthoritySet(grants)
  return descriptors.filter((descriptor) =>
    openAgentsMcpDescriptorIsGranted(descriptor, grantedAuthorities),
  )
}
