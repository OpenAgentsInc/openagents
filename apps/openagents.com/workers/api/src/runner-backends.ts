import { Schema as S } from 'effect'

export const OpenAgentsRunnerBackendKind = S.Literals([
  'cloudflare_container',
  'gcloud_vm',
  'shc_vm',
])
export type OpenAgentsRunnerBackendKind =
  typeof OpenAgentsRunnerBackendKind.Type

export const OpenAgentsRunnerWorkloadTrust = S.Literals([
  'low',
  'medium',
  'sensitive',
])
export type OpenAgentsRunnerWorkloadTrust =
  typeof OpenAgentsRunnerWorkloadTrust.Type

export const OpenAgentsRunnerDispatchStatus = S.Literals([
  'artifact_ready',
  'blocked',
  'cancelled',
  'completed',
  'failed',
  'queued',
  'running',
  'starting',
])
export type OpenAgentsRunnerDispatchStatus =
  typeof OpenAgentsRunnerDispatchStatus.Type

export const OpenAgentsRunnerLifecycleEventKind = S.Literals([
  'artifact',
  'blocked',
  'cancelled',
  'completed',
  'failed',
  'queued',
  'started',
])
export type OpenAgentsRunnerLifecycleEventKind =
  typeof OpenAgentsRunnerLifecycleEventKind.Type

export const OpenAgentsRunnerProjectionAudience = S.Literals([
  'customer',
  'operator',
  'public',
])
export type OpenAgentsRunnerProjectionAudience =
  typeof OpenAgentsRunnerProjectionAudience.Type

export const OpenAgentsRunnerBackendRecord = S.Struct({
  artifactRefs: S.Array(S.String),
  backendKind: OpenAgentsRunnerBackendKind,
  capacityRefs: S.Array(S.String),
  configured: S.Boolean,
  costRefs: S.Array(S.String),
  dispatchStatus: OpenAgentsRunnerDispatchStatus,
  displayNameRef: S.String,
  enabled: S.Boolean,
  healthRefs: S.Array(S.String),
  id: S.String,
  lifecycleEventRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  trustLevel: OpenAgentsRunnerWorkloadTrust,
})
export type OpenAgentsRunnerBackendRecord =
  typeof OpenAgentsRunnerBackendRecord.Type

export const OpenAgentsRunnerBackendProjection = S.Struct({
  artifactRefs: S.Array(S.String),
  audience: OpenAgentsRunnerProjectionAudience,
  backendKind: OpenAgentsRunnerBackendKind,
  capacityRefs: S.Array(S.String),
  configured: S.Boolean,
  costRefs: S.Array(S.String),
  dispatchStatus: OpenAgentsRunnerDispatchStatus,
  displayNameRef: S.String,
  enabled: S.Boolean,
  healthRefs: S.Array(S.String),
  id: S.String,
  lifecycleEventRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  trustLevel: OpenAgentsRunnerWorkloadTrust,
})
export type OpenAgentsRunnerBackendProjection =
  typeof OpenAgentsRunnerBackendProjection.Type

const unsafeRunnerProjectionPattern =
  /(bearer\s+|callback[_-]?token|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|gho_[a-z0-9_]+|github\.com\/[^:/]+\/private|mnemonic|oauth|oa_agent_|openagents_admin|password|payment[_-]?proof|preimage|private[_-]?key|provider[_-]?grant|provider[_-]?payload|provider[_-]?token|raw[_-]?email|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|\S+@\S+|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/i
const operatorPrivateRunnerProjectionPattern =
  /(provider[_-]?account|provider[_-]?grant)/i

const safeRefForAudience = (
  ref: string,
  audience: OpenAgentsRunnerProjectionAudience,
): boolean =>
  ref.trim() !== '' &&
  !unsafeRunnerProjectionPattern.test(ref) &&
  (audience === 'operator' ||
    !operatorPrivateRunnerProjectionPattern.test(ref))

const safeRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: OpenAgentsRunnerProjectionAudience,
): ReadonlyArray<string> =>
  [...new Set(refs)].filter(ref => safeRefForAudience(ref, audience))

const safeRefOrFallback = (
  ref: string,
  audience: OpenAgentsRunnerProjectionAudience,
  fallback: string,
): string => safeRefsForAudience([ref], audience)[0] ?? fallback

export const projectOpenAgentsRunnerBackend = (
  record: OpenAgentsRunnerBackendRecord,
  audience: OpenAgentsRunnerProjectionAudience,
): OpenAgentsRunnerBackendProjection => ({
  artifactRefs: safeRefsForAudience(record.artifactRefs, audience),
  audience,
  backendKind: record.backendKind,
  capacityRefs: safeRefsForAudience(record.capacityRefs, audience),
  configured: record.configured,
  costRefs: safeRefsForAudience(record.costRefs, audience),
  dispatchStatus: record.dispatchStatus,
  displayNameRef: safeRefOrFallback(
    record.displayNameRef,
    audience,
    'runner_backend.display_name.redacted',
  ),
  enabled: record.enabled,
  healthRefs: safeRefsForAudience(record.healthRefs, audience),
  id: safeRefOrFallback(record.id, audience, 'runner_backend.redacted'),
  lifecycleEventRefs: safeRefsForAudience(
    record.lifecycleEventRefs,
    audience,
  ),
  operatorDiagnosticRefs:
    audience === 'operator'
      ? safeRefsForAudience(record.operatorDiagnosticRefs, audience)
      : [],
  policyRefs: safeRefsForAudience(record.policyRefs, audience),
  publicSummaryRef: safeRefOrFallback(
    record.publicSummaryRef,
    audience,
    'runner_backend.summary.redacted',
  ),
  receiptRefs: safeRefsForAudience(record.receiptRefs, audience),
  trustLevel: record.trustLevel,
})

export const openAgentsRunnerBackendProjectionHasPrivateMaterial = (
  projection: OpenAgentsRunnerBackendProjection,
): boolean => unsafeRunnerProjectionPattern.test(JSON.stringify(projection))
