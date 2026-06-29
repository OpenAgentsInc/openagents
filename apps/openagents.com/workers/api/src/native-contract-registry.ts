import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'

export const OpenAgentsNativeContractConsumer = S.Literals([
  'ai_agent',
  'nexus',
  'oa_node',
  'oa_workroomd',
  'omega_worker',
  'probe',
  'psionic',
  'pylon',
  'treasury',
])
export type OpenAgentsNativeContractConsumer =
  typeof OpenAgentsNativeContractConsumer.Type

export const OpenAgentsNativeContractRefKind = S.Literals([
  'artifact',
  'assignment',
  'capability',
  'heartbeat',
  'lifecycle_event',
  'policy',
  'receipt',
  'redaction',
  'route',
])
export type OpenAgentsNativeContractRefKind =
  typeof OpenAgentsNativeContractRefKind.Type

export const OpenAgentsNativeContractStability = S.Literals([
  'draft',
  'seed',
  'stable',
])
export type OpenAgentsNativeContractStability =
  typeof OpenAgentsNativeContractStability.Type

export const OpenAgentsNativeContractAuthorityBoundary = S.Literals([
  'approval_required_action',
  'evidence_only',
  'executed_action_receipt',
])
export type OpenAgentsNativeContractAuthorityBoundary =
  typeof OpenAgentsNativeContractAuthorityBoundary.Type

export const OpenAgentsNativeContractPrivacyPolicy = S.Literals([
  'operator_refs_only',
  'public_refs_only',
  'team_refs_only',
])
export type OpenAgentsNativeContractPrivacyPolicy =
  typeof OpenAgentsNativeContractPrivacyPolicy.Type

export class OpenAgentsNativeContractRegistryEntry extends S.Class<OpenAgentsNativeContractRegistryEntry>(
  'OpenAgentsNativeContractRegistryEntry',
)({
  authorityBoundary: OpenAgentsNativeContractAuthorityBoundary,
  caveatRefs: S.Array(S.String),
  consumerRefs: S.Array(OpenAgentsNativeContractConsumer),
  correlationRefs: S.Array(S.String),
  createdAtIso: S.String,
  eventRef: S.NullOr(S.String),
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  name: S.String,
  payloadSchemaRef: S.String,
  policyRefs: S.Array(S.String),
  privacyPolicy: OpenAgentsNativeContractPrivacyPolicy,
  producerRefs: S.Array(OpenAgentsNativeContractConsumer),
  receiptRefs: S.Array(S.String),
  receiptSchemaRef: S.NullOr(S.String),
  redactionPolicyRefs: S.Array(S.String),
  refKind: OpenAgentsNativeContractRefKind,
  sourceAuthorityRefs: S.Array(S.String),
  stability: OpenAgentsNativeContractStability,
  topicRef: S.String,
  updatedAtIso: S.String,
}) {}

export class OpenAgentsNativeContractRegistryEntryProjection extends S.Class<OpenAgentsNativeContractRegistryEntryProjection>(
  'OpenAgentsNativeContractRegistryEntryProjection',
)({
  authorityBoundary: OpenAgentsNativeContractAuthorityBoundary,
  caveatRefs: S.Array(S.String),
  consumerRefs: S.Array(OpenAgentsNativeContractConsumer),
  correlationRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  eventRef: S.NullOr(S.String),
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  name: S.String,
  payloadSchemaRef: S.String,
  policyRefs: S.Array(S.String),
  privacyPolicy: OpenAgentsNativeContractPrivacyPolicy,
  producerRefs: S.Array(OpenAgentsNativeContractConsumer),
  receiptRefs: S.Array(S.String),
  receiptSchemaRef: S.NullOr(S.String),
  redactionPolicyRefs: S.Array(S.String),
  refKind: OpenAgentsNativeContractRefKind,
  sourceAuthorityRefs: S.Array(S.String),
  stability: OpenAgentsNativeContractStability,
  topicRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class OpenAgentsNativeContractRegistry extends S.Class<OpenAgentsNativeContractRegistry>(
  'OpenAgentsNativeContractRegistry',
)({
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  entries: S.Array(OpenAgentsNativeContractRegistryEntry),
  id: S.String,
  policyRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  updatedAtIso: S.String,
  versionRef: S.String,
}) {}

export class OpenAgentsNativeContractRegistryProjection extends S.Class<OpenAgentsNativeContractRegistryProjection>(
  'OpenAgentsNativeContractRegistryProjection',
)({
  audience: OmniProjectionAudience,
  authorityActionCount: S.Number,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  entries: S.Array(OpenAgentsNativeContractRegistryEntryProjection),
  entryCount: S.Number,
  evidenceOnlyCount: S.Number,
  id: S.String,
  policyRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  versionRef: S.String,
}) {}

export class OpenAgentsNativeContractRegistryUnsafe extends S.TaggedErrorClass<OpenAgentsNativeContractRegistryUnsafe>()(
  'OpenAgentsNativeContractRegistryUnsafe',
  {
    reason: S.String,
  },
) {}

export const OPENAGENTS_NATIVE_CONTRACT_CONSUMERS:
  ReadonlyArray<OpenAgentsNativeContractConsumer> = [
    'ai_agent',
    'nexus',
    'oa_node',
    'oa_workroomd',
    'omega_worker',
    'probe',
    'psionic',
    'pylon',
    'treasury',
  ]

export const OPENAGENTS_NATIVE_CONTRACT_REF_KINDS:
  ReadonlyArray<OpenAgentsNativeContractRefKind> = [
    'artifact',
    'assignment',
    'capability',
    'heartbeat',
    'lifecycle_event',
    'policy',
    'receipt',
    'redaction',
    'route',
  ]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeNativeContractPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(capability\.operator|policy\.operator|route\.private|source\.private|topic\.operator|workroom\.private)/i
const customerUnsafeRefPattern =
  /(capability\.operator|policy\.operator|route\.private|source\.private|topic\.operator|workroom\.private)/i
const teamUnsafeRefPattern =
  /(route\.private|source\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeNativeContractPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsNativeContractRegistryUnsafe({
      reason: `${label} contains secrets, provider grants, raw auth payloads, private repo material, wallet/payment material, payout targets, raw logs, raw source archives, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeNullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: typeof OmniProjectionAudience.Type,
): string | null =>
  ref === null
    ? null
    : safeRefsForAudience(label, [ref], audience)[0] ?? null

const entryRefs = (
  entry: OpenAgentsNativeContractRegistryEntry,
): ReadonlyArray<string> => [
  entry.id,
  entry.payloadSchemaRef,
  entry.topicRef,
  ...(entry.eventRef === null ? [] : [entry.eventRef]),
  ...(entry.receiptSchemaRef === null ? [] : [entry.receiptSchemaRef]),
  ...entry.caveatRefs,
  ...entry.correlationRefs,
  ...entry.idempotencyRefs,
  ...entry.policyRefs,
  ...entry.receiptRefs,
  ...entry.redactionPolicyRefs,
  ...entry.sourceAuthorityRefs,
]

const registryRefs = (
  registry: OpenAgentsNativeContractRegistry,
): ReadonlyArray<string> => [
  registry.id,
  registry.versionRef,
  ...registry.caveatRefs,
  ...registry.policyRefs,
  ...registry.redactionPolicyRefs,
  ...registry.entries.flatMap(entryRefs),
]

const assertRegistrySafe = (
  registry: OpenAgentsNativeContractRegistry,
): void => {
  assertSafeRefs('native contract registry refs', registryRefs(registry))
}

const entry = (
  refKind: OpenAgentsNativeContractRefKind,
  authorityBoundary: OpenAgentsNativeContractAuthorityBoundary,
  name: string,
  consumers: ReadonlyArray<OpenAgentsNativeContractConsumer>,
): OpenAgentsNativeContractRegistryEntry => ({
  authorityBoundary,
  caveatRefs: [`caveat.native_contract.${refKind}.seed`],
  consumerRefs: [...consumers],
  correlationRefs: [`correlation.${refKind}.v1`],
  createdAtIso: '2026-06-06T23:50:00.000Z',
  eventRef: `event.native.${refKind}.v1`,
  id: `native_contract.${refKind}.v1`,
  idempotencyRefs: [`idempotency.${refKind}.v1`],
  name,
  payloadSchemaRef: `schema.native.${refKind}.v1`,
  policyRefs: [`policy.native_contract.${refKind}.v1`],
  privacyPolicy:
    authorityBoundary === 'evidence_only'
      ? 'public_refs_only'
      : 'operator_refs_only',
  producerRefs: ['omega_worker'],
  receiptRefs:
    refKind === 'receipt'
      ? ['receipt.native.closeout.v1']
      : [`receipt.native.${refKind}.observed.v1`],
  receiptSchemaRef:
    refKind === 'receipt'
      ? 'schema.native.receipt_evidence.v1'
      : null,
  redactionPolicyRefs: [`redaction.native_contract.${refKind}.public_safe`],
  refKind,
  sourceAuthorityRefs: [`source_authority.native_contract.${refKind}.v1`],
  stability: 'seed',
  topicRef: `topic.native.${refKind}.v1`,
  updatedAtIso: '2026-06-06T23:55:00.000Z',
})

export const OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1:
  OpenAgentsNativeContractRegistry = {
    caveatRefs: ['caveat.native_contract.seed_only'],
    createdAtIso: '2026-06-06T23:50:00.000Z',
    entries: [
      entry(
        'assignment',
        'approval_required_action',
        'Assignment intake and dispatch intent',
        ['omega_worker', 'oa_workroomd', 'oa_node', 'probe'],
      ),
      entry(
        'heartbeat',
        'evidence_only',
        'Managed machine heartbeat',
        ['omega_worker', 'oa_node', 'pylon'],
      ),
      entry(
        'lifecycle_event',
        'evidence_only',
        'Workroom lifecycle event',
        ['omega_worker', 'oa_workroomd', 'oa_node', 'probe', 'pylon'],
      ),
      entry(
        'artifact',
        'evidence_only',
        'Artifact manifest reference',
        ['omega_worker', 'oa_workroomd', 'probe', 'psionic'],
      ),
      entry(
        'receipt',
        'executed_action_receipt',
        'Action closeout receipt reference',
        ['omega_worker', 'oa_workroomd', 'nexus', 'pylon', 'treasury'],
      ),
      entry(
        'route',
        'evidence_only',
        'Route selection evidence',
        ['ai_agent', 'omega_worker', 'oa_workroomd', 'probe', 'psionic', 'pylon'],
      ),
      entry(
        'capability',
        'evidence_only',
        'Capability snapshot reference',
        ['omega_worker', 'oa_node', 'pylon', 'probe', 'psionic'],
      ),
      entry(
        'redaction',
        'evidence_only',
        'Redaction report reference',
        ['omega_worker', 'oa_workroomd', 'probe', 'psionic', 'pylon'],
      ),
      entry(
        'policy',
        'evidence_only',
        'Policy decision reference',
        ['omega_worker', 'oa_workroomd', 'oa_node', 'probe', 'psionic', 'pylon'],
      ),
    ],
    id: 'native_contract_registry.v1',
    policyRefs: ['policy.native_contract.registry.seed'],
    redactionPolicyRefs: ['redaction.native_contract.registry.public_safe'],
    updatedAtIso: '2026-06-06T23:55:00.000Z',
    versionRef: 'version.native_contract_registry.v1',
  }

export const openAgentsNativeContractEntryIsEvidenceOnly = (
  entryValue: OpenAgentsNativeContractRegistryEntry,
): boolean => entryValue.authorityBoundary === 'evidence_only'

export const openAgentsNativeContractEntryCarriesAuthorityBoundary = (
  entryValue: OpenAgentsNativeContractRegistryEntry,
): boolean => entryValue.authorityBoundary !== 'evidence_only'

export const openAgentsNativeContractRegistryCoversRefKinds = (
  registry: OpenAgentsNativeContractRegistry,
): boolean =>
  OPENAGENTS_NATIVE_CONTRACT_REF_KINDS.every(kind =>
    registry.entries.some(entryValue => entryValue.refKind === kind)
  )

export const openAgentsNativeContractRegistryCoversConsumers = (
  registry: OpenAgentsNativeContractRegistry,
): boolean =>
  OPENAGENTS_NATIVE_CONTRACT_CONSUMERS.every(consumer =>
    registry.entries.some(entryValue =>
      entryValue.consumerRefs.includes(consumer)
    )
  )

const projectEntry = (
  entryValue: OpenAgentsNativeContractRegistryEntry,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsNativeContractRegistryEntryProjection => ({
  authorityBoundary: entryValue.authorityBoundary,
  caveatRefs: safeRefsForAudience(
    'native contract entry caveat refs',
    entryValue.caveatRefs,
    audience,
  ),
  consumerRefs: entryValue.consumerRefs,
  correlationRefs: safeRefsForAudience(
    'native contract correlation refs',
    entryValue.correlationRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    entryValue.createdAtIso,
    nowIso,
  ),
  eventRef: safeNullableRefForAudience(
    'native contract event ref',
    entryValue.eventRef,
    audience,
  ),
  id: safeRefsForAudience('native contract entry id', [entryValue.id], audience)
    [0] ?? 'native_contract.redacted',
  idempotencyRefs: safeRefsForAudience(
    'native contract idempotency refs',
    entryValue.idempotencyRefs,
    audience,
  ),
  name: entryValue.name,
  payloadSchemaRef: safeRefsForAudience(
    'native contract payload schema ref',
    [entryValue.payloadSchemaRef],
    audience,
  )[0] ?? 'schema.native.redacted',
  policyRefs: safeRefsForAudience(
    'native contract policy refs',
    entryValue.policyRefs,
    audience,
  ),
  privacyPolicy: entryValue.privacyPolicy,
  producerRefs: entryValue.producerRefs,
  receiptRefs: safeRefsForAudience(
    'native contract receipt refs',
    entryValue.receiptRefs,
    audience,
  ),
  receiptSchemaRef: safeNullableRefForAudience(
    'native contract receipt schema ref',
    entryValue.receiptSchemaRef,
    audience,
  ),
  redactionPolicyRefs: safeRefsForAudience(
    'native contract redaction refs',
    entryValue.redactionPolicyRefs,
    audience,
  ),
  refKind: entryValue.refKind,
  sourceAuthorityRefs: safeRefsForAudience(
    'native contract source authority refs',
    entryValue.sourceAuthorityRefs,
    audience,
  ),
  stability: entryValue.stability,
  topicRef: safeRefsForAudience(
    'native contract topic ref',
    [entryValue.topicRef],
    audience,
  )[0] ?? 'topic.native.redacted',
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    entryValue.updatedAtIso,
    nowIso,
  ),
})

export const projectOpenAgentsNativeContractRegistry = (
  registry: OpenAgentsNativeContractRegistry,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsNativeContractRegistryProjection => {
  assertRegistrySafe(registry)

  const entries = registry.entries.map(entryValue =>
    projectEntry(entryValue, audience, nowIso)
  )
  const projection: OpenAgentsNativeContractRegistryProjection = {
    audience,
    authorityActionCount: entries.filter(entryValue =>
      entryValue.authorityBoundary !== 'evidence_only'
    ).length,
    caveatRefs: safeRefsForAudience(
      'native contract registry caveat refs',
      registry.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      registry.createdAtIso,
      nowIso,
    ),
    entries,
    entryCount: entries.length,
    evidenceOnlyCount: entries.filter(entryValue =>
      entryValue.authorityBoundary === 'evidence_only'
    ).length,
    id: safeRefsForAudience('native contract registry id', [registry.id], audience)
      [0] ?? 'native_contract_registry.redacted',
    policyRefs: safeRefsForAudience(
      'native contract registry policy refs',
      registry.policyRefs,
      audience,
    ),
    redactionPolicyRefs: safeRefsForAudience(
      'native contract registry redaction refs',
      registry.redactionPolicyRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      registry.updatedAtIso,
      nowIso,
    ),
    versionRef: safeRefsForAudience(
      'native contract registry version ref',
      [registry.versionRef],
      audience,
    )[0] ?? 'version.native_contract_registry.redacted',
  }

  if (openAgentsNativeContractRegistryProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsNativeContractRegistryUnsafe({
      reason: 'Native contract registry projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsNativeContractRegistryProjectionHasPrivateMaterial = (
  projection: OpenAgentsNativeContractRegistryProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafeNativeContractPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}
