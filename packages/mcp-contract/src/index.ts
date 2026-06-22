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

export const OpenAgentsMcpErrorTag = S.Literals([
  "denied",
  "missing_grant",
  "needs_auth",
  "blocked_by_policy",
  "validation_failed",
  "transport_failed",
  "target_unavailable",
  "unsafe_output_omitted",
])
export type OpenAgentsMcpErrorTag = typeof OpenAgentsMcpErrorTag.Type

export const OpenAgentsMcpError = S.Struct({
  tag: OpenAgentsMcpErrorTag,
  message: S.String,
  retryable: S.Boolean,
  authorityClass: S.optional(OpenAgentsMcpAuthorityClass),
  blockerRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpError = typeof OpenAgentsMcpError.Type

export const decodeOpenAgentsMcpError = S.decodeUnknownSync(OpenAgentsMcpError)

export const openAgentsMcpErrorHttpStatus = (
  tag: OpenAgentsMcpErrorTag,
): number => {
  switch (tag) {
    case "denied":
    case "missing_grant":
      return 403
    case "needs_auth":
      return 401
    case "blocked_by_policy":
      return 423
    case "validation_failed":
      return 400
    case "transport_failed":
    case "target_unavailable":
      return 503
    case "unsafe_output_omitted":
      return 206
  }
}

export const OpenAgentsMcpReceiptKind = S.Literals([
  "noop",
  "read",
  "mutation",
  "approval",
  "payment_receive",
  "payment_spend",
  "deployment",
  "admin",
])
export type OpenAgentsMcpReceiptKind = typeof OpenAgentsMcpReceiptKind.Type

export const OpenAgentsMcpReceiptStatus = S.Literals([
  "recorded",
  "applied",
  "duplicate",
  "rejected",
  "failed",
])
export type OpenAgentsMcpReceiptStatus = typeof OpenAgentsMcpReceiptStatus.Type

export const OpenAgentsMcpReceipt = S.Struct({
  receiptRef: S.String,
  kind: OpenAgentsMcpReceiptKind,
  status: OpenAgentsMcpReceiptStatus,
  generatedAt: S.String,
  authorityClass: OpenAgentsMcpAuthorityClass,
  targetRef: S.String,
  summary: S.String,
  amountSats: S.optional(S.Number),
  artifactRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpReceipt = typeof OpenAgentsMcpReceipt.Type

export const decodeOpenAgentsMcpReceipt = S.decodeUnknownSync(OpenAgentsMcpReceipt)

export const OpenAgentsMcpProgressStatus = S.Literals([
  "queued",
  "running",
  "waiting",
  "completed",
  "failed",
  "cancelled",
])
export type OpenAgentsMcpProgressStatus = typeof OpenAgentsMcpProgressStatus.Type

export const OpenAgentsMcpProgressEvent = S.Struct({
  progressRef: S.String,
  operationRef: S.String,
  sequence: S.Number,
  status: OpenAgentsMcpProgressStatus,
  message: S.String,
  percent: S.optional(S.Number),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpProgressEvent = typeof OpenAgentsMcpProgressEvent.Type

export const decodeOpenAgentsMcpProgressEvent = S.decodeUnknownSync(
  OpenAgentsMcpProgressEvent,
)

export const OpenAgentsMcpElicitationKind = S.Literals([
  "approval_prompt",
  "auth_prompt",
  "missing_config",
  "amount_cap",
  "human_confirmation",
])
export type OpenAgentsMcpElicitationKind =
  typeof OpenAgentsMcpElicitationKind.Type

export const OpenAgentsMcpElicitationRequest = S.Struct({
  requestRef: S.String,
  kind: OpenAgentsMcpElicitationKind,
  title: S.String,
  message: S.String,
  requiredAuthorities: S.Array(OpenAgentsMcpAuthorityClass),
  inputSchemaRef: S.String,
  expiresAt: S.optional(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpElicitationRequest =
  typeof OpenAgentsMcpElicitationRequest.Type

export const OpenAgentsMcpElicitationDecision = S.Literals([
  "approved",
  "denied",
  "provided",
  "cancelled",
  "expired",
])
export type OpenAgentsMcpElicitationDecision =
  typeof OpenAgentsMcpElicitationDecision.Type

export const OpenAgentsMcpElicitationResponse = S.Struct({
  requestRef: S.String,
  responseRef: S.String,
  decision: OpenAgentsMcpElicitationDecision,
  generatedAt: S.String,
  valueRef: S.optional(S.String),
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpElicitationResponse =
  typeof OpenAgentsMcpElicitationResponse.Type

export const decodeOpenAgentsMcpElicitationRequest = S.decodeUnknownSync(
  OpenAgentsMcpElicitationRequest,
)
export const decodeOpenAgentsMcpElicitationResponse = S.decodeUnknownSync(
  OpenAgentsMcpElicitationResponse,
)

export const OpenAgentsMcpOutputSafetyClass = S.Literals([
  "public",
  "operator",
  "private_account",
  "local_only",
  "workspace_private",
  "secret_bearing",
  "omitted",
])
export type OpenAgentsMcpOutputSafetyClass =
  typeof OpenAgentsMcpOutputSafetyClass.Type

export const OpenAgentsMcpPersistencePolicy = S.Literals([
  "do_not_persist",
  "persist_public_summary",
  "persist_operator_summary",
  "persist_private_ref_only",
])
export type OpenAgentsMcpPersistencePolicy =
  typeof OpenAgentsMcpPersistencePolicy.Type

export const OpenAgentsMcpTruncationMetadata = S.Struct({
  truncated: S.Boolean,
  originalBytes: S.optional(S.Number),
  retainedBytes: S.optional(S.Number),
  omittedBytes: S.optional(S.Number),
  reason: S.optional(S.String),
})
export type OpenAgentsMcpTruncationMetadata =
  typeof OpenAgentsMcpTruncationMetadata.Type

export const OpenAgentsMcpOutputProjection = S.Struct({
  outputRef: S.String,
  safetyClass: OpenAgentsMcpOutputSafetyClass,
  summary: S.String,
  text: S.optional(S.String),
  unsafeMaterialTags: S.Array(S.String),
  truncation: OpenAgentsMcpTruncationMetadata,
  persistence: OpenAgentsMcpPersistencePolicy,
  sourceRefs: S.Array(S.String),
})
export type OpenAgentsMcpOutputProjection =
  typeof OpenAgentsMcpOutputProjection.Type

export const decodeOpenAgentsMcpOutputProjection = S.decodeUnknownSync(
  OpenAgentsMcpOutputProjection,
)

export const OpenAgentsMcpUnsafeMaterialTag = S.Literals([
  "mnemonic",
  "access_token",
  "bearer_token",
  "private_prompt",
  "local_path",
  "wallet_secret",
  "credential_material",
])
export type OpenAgentsMcpUnsafeMaterialTag =
  typeof OpenAgentsMcpUnsafeMaterialTag.Type

const openAgentsMcpUnsafePatterns: ReadonlyArray<Readonly<{
  tag: OpenAgentsMcpUnsafeMaterialTag
  pattern: RegExp
  replacement: string
}>> = [
  {
    tag: "mnemonic",
    pattern: /\b(?:mnemonic|seed phrase)\s*[:=]\s*[a-z]+(?:\s+[a-z]+){5,}\b/gi,
    replacement: "mnemonic:[REDACTED]",
  },
  {
    tag: "access_token",
    pattern: /\b(?:access[_-]?token|MDK_ACCESS_TOKEN)\s*[:=]\s*[\w.-]+/gi,
    replacement: "access_token:[REDACTED]",
  },
  {
    tag: "bearer_token",
    pattern: /\bBearer\s+[\w.-]+/gi,
    replacement: "Bearer [REDACTED]",
  },
  {
    tag: "private_prompt",
    pattern: /\b(?:raw_prompt|private prompt|system prompt)\s*[:=]\s*[^\\n]+/gi,
    replacement: "private_prompt:[REDACTED]",
  },
  {
    tag: "local_path",
    pattern: /(?:\/Users\/|\/home\/|\/var\/folders\/)[^\s"'`]+/g,
    replacement: "[LOCAL_PATH_REDACTED]",
  },
  {
    tag: "wallet_secret",
    pattern: /\b(?:MDK_MNEMONIC|wallet secret|wallet_seed)\s*[:=]\s*[\w\s.-]+/gi,
    replacement: "wallet_secret:[REDACTED]",
  },
  {
    tag: "credential_material",
    pattern: /\b(?:sk-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9_]{8,}|api[_-]?key\s*[:=]\s*[\w.-]+)/g,
    replacement: "credential:[REDACTED]",
  },
]

export const detectOpenAgentsMcpUnsafeMaterial = (
  value: unknown,
): ReadonlyArray<OpenAgentsMcpUnsafeMaterialTag> => {
  const serialized = typeof value === "string" ? value : JSON.stringify(value)
  if (serialized === undefined) return []
  const tags = new Set<OpenAgentsMcpUnsafeMaterialTag>()
  for (const { tag, pattern } of openAgentsMcpUnsafePatterns) {
    pattern.lastIndex = 0
    if (pattern.test(serialized)) tags.add(tag)
  }
  return [...tags]
}

export const redactOpenAgentsMcpUnsafeText = (text: string): string =>
  openAgentsMcpUnsafePatterns.reduce(
    (current, { pattern, replacement }) => current.replace(pattern, replacement),
    text,
  )

export type ProjectOpenAgentsMcpOutputInput = Readonly<{
  outputRef: string
  safetyClass: OpenAgentsMcpOutputSafetyClass
  text: string
  sourceRefs: ReadonlyArray<string>
  maxTextBytes?: number
}>

export const projectOpenAgentsMcpOutput = (
  input: ProjectOpenAgentsMcpOutputInput,
): OpenAgentsMcpOutputProjection => {
  const unsafeMaterialTags = detectOpenAgentsMcpUnsafeMaterial(input.text)
  const mayExposeText =
    input.safetyClass !== "secret_bearing" &&
    input.safetyClass !== "omitted" &&
    unsafeMaterialTags.length === 0
  const maxTextBytes = input.maxTextBytes ?? 4096
  const encodedLength = new TextEncoder().encode(input.text).length
  const truncated = mayExposeText && encodedLength > maxTextBytes
  const text = mayExposeText
    ? input.text.slice(0, maxTextBytes)
    : undefined
  const omittedBytes = mayExposeText
    ? truncated ? Math.max(encodedLength - maxTextBytes, 0) : 0
    : encodedLength
  const base = {
    outputRef: input.outputRef,
    safetyClass: unsafeMaterialTags.length > 0 ? "omitted" as const : input.safetyClass,
    summary: mayExposeText ? "Output projected." : "Output omitted by MCP safety policy.",
    unsafeMaterialTags: [...unsafeMaterialTags],
    truncation: {
      truncated,
      originalBytes: encodedLength,
      retainedBytes: text === undefined ? 0 : new TextEncoder().encode(text).length,
      omittedBytes,
      ...(truncated ? { reason: "max_text_bytes" } : {}),
      ...(!mayExposeText ? { reason: "unsafe_or_secret_output" } : {}),
    },
    persistence: input.safetyClass === "public"
      ? "persist_public_summary" as const
      : input.safetyClass === "operator"
        ? "persist_operator_summary" as const
        : "persist_private_ref_only" as const,
    sourceRefs: [...input.sourceRefs],
  }
  return text === undefined ? base : { ...base, text }
}
