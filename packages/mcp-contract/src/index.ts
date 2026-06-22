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

export const OpenAgentsMcpTransportKind = S.Literals([
  "stdio",
  "loopback_http",
  "streamable_http",
  "sse",
  "websocket",
  "ide_local",
  "in_process",
  "bridge_proxy",
])
export type OpenAgentsMcpTransportKind = typeof OpenAgentsMcpTransportKind.Type

export const openAgentsMcpTransportKinds: ReadonlyArray<OpenAgentsMcpTransportKind> = [
  "stdio",
  "loopback_http",
  "streamable_http",
  "sse",
  "websocket",
  "ide_local",
  "in_process",
  "bridge_proxy",
]

export const OpenAgentsMcpConfigSource = S.Literals([
  "local_private",
  "shared_project",
  "user",
  "managed",
  "dynamic",
  "plugin",
  "ide",
  "desktop_discovered",
])
export type OpenAgentsMcpConfigSource = typeof OpenAgentsMcpConfigSource.Type

export const openAgentsMcpConfigSources: ReadonlyArray<OpenAgentsMcpConfigSource> = [
  "local_private",
  "shared_project",
  "user",
  "managed",
  "dynamic",
  "plugin",
  "ide",
  "desktop_discovered",
]

export const OpenAgentsMcpLifecycleStatus = S.Literals([
  "discovered",
  "pending_approval",
  "enabled",
  "connecting",
  "connected",
  "needs_auth",
  "disabled",
  "rejected",
  "failed",
  "revoked",
  "blocked_by_policy",
])
export type OpenAgentsMcpLifecycleStatus = typeof OpenAgentsMcpLifecycleStatus.Type

export const openAgentsMcpLifecycleStatuses: ReadonlyArray<OpenAgentsMcpLifecycleStatus> = [
  "discovered",
  "pending_approval",
  "enabled",
  "connecting",
  "connected",
  "needs_auth",
  "disabled",
  "rejected",
  "failed",
  "revoked",
  "blocked_by_policy",
]

const OpenAgentsMcpBaseTransportConfig = {
  label: S.String,
  sourceRefs: S.Array(S.String),
}

export const OpenAgentsMcpTransportConfig = S.Union([
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("stdio"),
    commandRef: S.String,
    argumentRefs: S.Array(S.String),
    environmentRef: S.optional(S.String),
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("loopback_http"),
    origin: S.String,
    streamPath: S.String,
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("streamable_http"),
    origin: S.String,
    endpointPath: S.String,
    authRef: S.optional(S.String),
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("sse"),
    origin: S.String,
    eventsPath: S.String,
    messagesPath: S.optional(S.String),
    authRef: S.optional(S.String),
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("websocket"),
    url: S.String,
    protocolRef: S.optional(S.String),
    authRef: S.optional(S.String),
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("ide_local"),
    ideRef: S.String,
    serverRef: S.String,
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("in_process"),
    runtimeRef: S.String,
    serviceRef: S.String,
  }),
  S.Struct({
    ...OpenAgentsMcpBaseTransportConfig,
    kind: S.Literal("bridge_proxy"),
    bridgeRef: S.String,
    targetRef: S.String,
    authRef: S.optional(S.String),
  }),
])
export type OpenAgentsMcpTransportConfig = typeof OpenAgentsMcpTransportConfig.Type

export const OpenAgentsMcpServerConfig = S.Struct({
  serverRef: S.String,
  displayName: S.String,
  source: OpenAgentsMcpConfigSource,
  lifecycleStatus: OpenAgentsMcpLifecycleStatus,
  transport: OpenAgentsMcpTransportConfig,
  requestedAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  secretRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpServerConfig = typeof OpenAgentsMcpServerConfig.Type

export const OpenAgentsMcpServerConfigPublicProjection = S.Struct({
  serverRef: S.String,
  displayName: S.String,
  source: OpenAgentsMcpConfigSource,
  lifecycleStatus: OpenAgentsMcpLifecycleStatus,
  transportKind: OpenAgentsMcpTransportKind,
  requestedAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpServerConfigPublicProjection =
  typeof OpenAgentsMcpServerConfigPublicProjection.Type

export const decodeOpenAgentsMcpTransportConfig = S.decodeUnknownSync(
  OpenAgentsMcpTransportConfig,
)
export const decodeOpenAgentsMcpServerConfig = S.decodeUnknownSync(
  OpenAgentsMcpServerConfig,
)

export const projectOpenAgentsMcpServerConfigPublic = (
  config: OpenAgentsMcpServerConfig,
): OpenAgentsMcpServerConfigPublicProjection => ({
  serverRef: config.serverRef,
  displayName: config.displayName,
  source: config.source,
  lifecycleStatus: config.lifecycleStatus,
  transportKind: config.transport.kind,
  requestedAuthorities: [...config.requestedAuthorities],
  sourceRefs: [...config.sourceRefs],
})

export const OpenAgentsMcpCapabilityRiskClass = S.Literals([
  "read_only",
  "low",
  "medium",
  "high",
  "critical",
])
export type OpenAgentsMcpCapabilityRiskClass =
  typeof OpenAgentsMcpCapabilityRiskClass.Type

export const OpenAgentsMcpReceiptBehavior = S.Literals([
  "none",
  "read_receipt",
  "mutation_receipt",
  "approval_receipt",
  "payment_receipt",
  "deployment_receipt",
  "admin_receipt",
])
export type OpenAgentsMcpReceiptBehavior = typeof OpenAgentsMcpReceiptBehavior.Type

export const OpenAgentsMcpProgressBehavior = S.Literals([
  "none",
  "bounded",
  "streaming",
  "long_running",
])
export type OpenAgentsMcpProgressBehavior = typeof OpenAgentsMcpProgressBehavior.Type

export const OpenAgentsMcpOutputHandlingPolicy = S.Literals([
  "public_safe",
  "operator_only",
  "private_account",
  "local_only",
  "unsafe_omitted",
])
export type OpenAgentsMcpOutputHandlingPolicy =
  typeof OpenAgentsMcpOutputHandlingPolicy.Type

export const OpenAgentsMcpPromptAudience = S.Literals([
  "public",
  "operator",
  "contributor",
  "coding_agent",
  "admin",
])
export type OpenAgentsMcpPromptAudience = typeof OpenAgentsMcpPromptAudience.Type

export const OpenAgentsMcpResourceNamespace = S.Literals([
  "pylon",
  "autopilot",
  "verse",
  "worker",
  "forum",
  "payments",
  "coding-session",
])
export type OpenAgentsMcpResourceNamespace =
  typeof OpenAgentsMcpResourceNamespace.Type

export const openAgentsMcpResourceNamespaces: ReadonlyArray<OpenAgentsMcpResourceNamespace> = [
  "pylon",
  "autopilot",
  "verse",
  "worker",
  "forum",
  "payments",
  "coding-session",
]

export const OpenAgentsMcpStalenessContract = S.Struct({
  generatedAt: S.optional(S.String),
  maxStalenessSeconds: S.Number,
  transitionRefs: S.Array(S.String),
})
export type OpenAgentsMcpStalenessContract =
  typeof OpenAgentsMcpStalenessContract.Type

export const OpenAgentsMcpToolDescriptor = S.Struct({
  name: S.String,
  title: S.String,
  description: S.String,
  requiredAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  riskClass: OpenAgentsMcpCapabilityRiskClass,
  inputSchemaRef: S.String,
  outputSchemaRef: S.String,
  receiptBehavior: OpenAgentsMcpReceiptBehavior,
  progressBehavior: OpenAgentsMcpProgressBehavior,
  publicSummary: S.String,
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpToolDescriptor = typeof OpenAgentsMcpToolDescriptor.Type

export const OpenAgentsMcpResourceDescriptor = S.Struct({
  uri: S.String,
  name: S.String,
  title: S.String,
  description: S.String,
  namespace: OpenAgentsMcpResourceNamespace,
  requiredAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  staleness: OpenAgentsMcpStalenessContract,
  publicProjectionSafe: S.Boolean,
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpResourceDescriptor =
  typeof OpenAgentsMcpResourceDescriptor.Type

export const OpenAgentsMcpPromptDescriptor = S.Struct({
  name: S.String,
  title: S.String,
  description: S.String,
  audience: OpenAgentsMcpPromptAudience,
  requiredAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  inputSchemaRef: S.String,
  outputHandling: OpenAgentsMcpOutputHandlingPolicy,
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpPromptDescriptor = typeof OpenAgentsMcpPromptDescriptor.Type

export const decodeOpenAgentsMcpToolDescriptor = S.decodeUnknownSync(
  OpenAgentsMcpToolDescriptor,
)
export const decodeOpenAgentsMcpResourceDescriptor = S.decodeUnknownSync(
  OpenAgentsMcpResourceDescriptor,
)
export const decodeOpenAgentsMcpPromptDescriptor = S.decodeUnknownSync(
  OpenAgentsMcpPromptDescriptor,
)

const openAgentsMcpNamePattern = /^[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)+$/

export const isValidOpenAgentsMcpName = (name: string): boolean =>
  openAgentsMcpNamePattern.test(name)

export const assertValidOpenAgentsMcpName = (name: string): string => {
  if (!isValidOpenAgentsMcpName(name)) {
    throw new Error(`Invalid OpenAgents MCP name: ${name}`)
  }
  return name
}

export type OpenAgentsMcpParsedResourceUri = Readonly<{
  uri: string
  namespace: OpenAgentsMcpResourceNamespace
  path: string
}>

export const parseOpenAgentsMcpResourceUri = (
  uri: string,
): OpenAgentsMcpParsedResourceUri => {
  const match = /^mcp:\/\/openagents\/([a-z][a-z-]*)\/(.+)$/.exec(uri)
  if (match === null) {
    throw new Error(`Invalid OpenAgents MCP resource URI: ${uri}`)
  }
  const namespace = match[1] as OpenAgentsMcpResourceNamespace
  const path = match[2]
  if (
    !openAgentsMcpResourceNamespaces.includes(namespace) ||
    path === undefined ||
    path.length === 0 ||
    path.includes("..")
  ) {
    throw new Error(`Invalid OpenAgents MCP resource URI: ${uri}`)
  }
  return { uri, namespace, path }
}
