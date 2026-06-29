import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  PublicClaimProjection,
  PublicClaimProjectionAudience,
  PublicClaimProjectionRecord,
  PublicClaimProjectionUnsafe,
  projectPublicClaimRecord,
} from './public-claim-projections'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const PublicAgentTemplateSource = S.Literals([
  'adjutant',
  'artanis',
  'general',
])
export type PublicAgentTemplateSource = typeof PublicAgentTemplateSource.Type

export const PublicAgentTemplateHealth = S.Literals([
  'blocked',
  'healthy',
  'idle',
  'queued',
  'reviewing',
  'running',
  'unavailable',
  'waiting_for_input',
])
export type PublicAgentTemplateHealth = typeof PublicAgentTemplateHealth.Type

export const PublicAgentTemplateGateState = S.Literals([
  'blocked',
  'disabled',
  'open',
  'ready',
  'waiting',
])
export type PublicAgentTemplateGateState =
  typeof PublicAgentTemplateGateState.Type

export const PublicAgentTemplateTimelineState = S.Literals([
  'blocked',
  'completed',
  'failed',
  'measured',
  'planned',
  'running',
  'verified',
])
export type PublicAgentTemplateTimelineState =
  typeof PublicAgentTemplateTimelineState.Type

export const PublicAgentTemplateGateRecord = S.Struct({
  caveatRefs: S.Array(S.String),
  customerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  gateRef: S.String,
  labelRef: S.String,
  operatorRefs: S.Array(S.String),
  state: PublicAgentTemplateGateState,
  teamRefs: S.Array(S.String),
})
export type PublicAgentTemplateGateRecord =
  typeof PublicAgentTemplateGateRecord.Type

export const PublicAgentTemplateTimelineEventRecord = S.Struct({
  artifactRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  customerRefs: S.Array(S.String),
  eventRef: S.String,
  operatorRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  state: PublicAgentTemplateTimelineState,
  teamRefs: S.Array(S.String),
  titleRef: S.String,
  updatedAt: S.String,
})
export type PublicAgentTemplateTimelineEventRecord =
  typeof PublicAgentTemplateTimelineEventRecord.Type

export const PublicAgentTemplateRecord = S.Struct({
  agentId: S.String,
  agentRef: S.String,
  artifactRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claim: PublicClaimProjectionRecord,
  currentStateRef: S.String,
  customerRefs: S.Array(S.String),
  displayName: S.String,
  eventTimeline: S.Array(PublicAgentTemplateTimelineEventRecord),
  gates: S.Array(PublicAgentTemplateGateRecord),
  health: PublicAgentTemplateHealth,
  objectiveRef: S.String,
  operatorRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  source: PublicAgentTemplateSource,
  teamRefs: S.Array(S.String),
  updatedAt: S.String,
})
export type PublicAgentTemplateRecord =
  typeof PublicAgentTemplateRecord.Type

export const PublicAgentTemplateGateProjection = S.Struct({
  caveatRefs: S.Array(S.String),
  customerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  gateRef: S.String,
  labelRef: S.String,
  operatorRefs: S.Array(S.String),
  state: PublicAgentTemplateGateState,
  teamRefs: S.Array(S.String),
})
export type PublicAgentTemplateGateProjection =
  typeof PublicAgentTemplateGateProjection.Type

export const PublicAgentTemplateTimelineEventProjection = S.Struct({
  artifactRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  customerRefs: S.Array(S.String),
  eventRef: S.String,
  operatorRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  state: PublicAgentTemplateTimelineState,
  teamRefs: S.Array(S.String),
  titleRef: S.String,
  updatedAt: S.String,
})
export type PublicAgentTemplateTimelineEventProjection =
  typeof PublicAgentTemplateTimelineEventProjection.Type

export const PublicAgentTemplateProjection = S.Struct({
  agentId: S.String,
  agentRef: S.String,
  artifactRefs: S.Array(S.String),
  audience: PublicClaimProjectionAudience,
  caveatRefs: S.Array(S.String),
  claim: PublicClaimProjection,
  currentStateRef: S.String,
  customerRefs: S.Array(S.String),
  displayName: S.String,
  eventTimeline: S.Array(PublicAgentTemplateTimelineEventProjection),
  gates: S.Array(PublicAgentTemplateGateProjection),
  health: PublicAgentTemplateHealth,
  objectiveRef: S.String,
  operatorRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  source: PublicAgentTemplateSource,
  teamRefs: S.Array(S.String),
  updatedAt: S.String,
})
export type PublicAgentTemplateProjection =
  typeof PublicAgentTemplateProjection.Type

export class PublicAgentTemplateUnsafe extends S.TaggedErrorClass<PublicAgentTemplateUnsafe>()(
  'PublicAgentTemplateUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeValuePattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|token|wallet|workroom[_-]?private)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.values(value).some(valueHasPrivateMaterial)
  }

  return false
}

const safeRef = (ref: string): string | undefined => {
  const trimmed = ref.trim()

  return trimmed !== '' &&
    safeRefPattern.test(trimmed) &&
    !valueHasPrivateMaterial(trimmed)
    ? trimmed
    : undefined
}

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const projected = [...new Set(refs)].map(safeRef)

  if (projected.some(ref => ref === undefined)) {
    throw new PublicAgentTemplateUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, or private workroom material.`,
    })
  }

  return projected.filter((ref): ref is string => ref !== undefined).sort()
}

const parsePublicUrl = (url: string): URL | undefined => {
  try {
    return new URL(url)
  } catch {
    return undefined
  }
}

const safePublicUrl = (url: string): string | undefined => {
  const trimmed = url.trim()
  const parsed = parsePublicUrl(trimmed)

  return parsed !== undefined &&
    parsed.protocol === 'https:' &&
    parsed.hostname === 'openagents.com' &&
    parsed.search === '' &&
    parsed.hash === '' &&
    !valueHasPrivateMaterial(trimmed)
    ? trimmed
    : undefined
}

const safePublicUrls = (
  urls: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const projected = [...new Set(urls)].map(safePublicUrl)

  if (projected.some(url => url === undefined)) {
    throw new PublicAgentTemplateUnsafe({
      reason: 'Public agent URLs must be clean first-party HTTPS URLs without private state.',
    })
  }

  return projected.filter((url): url is string => url !== undefined).sort()
}

const customerRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'customer' || audience === 'team' || audience === 'operator'
    ? safeRefs('customer refs', refs)
    : []

const teamRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'team' || audience === 'operator'
    ? safeRefs('team refs', refs)
    : []

const operatorRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> =>
  audience === 'operator'
    ? safeRefs('operator refs', refs)
    : []

const assertRecordSafe = (record: PublicAgentTemplateRecord): void => {
  safeRefs('agent refs', [
    record.agentId,
    record.agentRef,
    record.currentStateRef,
    record.displayName,
    record.objectiveRef,
  ])
  safeRefs('artifact refs', record.artifactRefs)
  safeRefs('caveat refs', record.caveatRefs)
  safeRefs('proof refs', record.proofRefs)
  safeRefs('customer refs', record.customerRefs)
  safeRefs('team refs', record.teamRefs)
  safeRefs('operator refs', record.operatorRefs)
  safePublicUrls(record.publicUrls)

  if (valueHasPrivateMaterial(record.updatedAt)) {
    throw new PublicAgentTemplateUnsafe({
      reason: 'updatedAt contains private material.',
    })
  }
}

const projectGate = (
  gate: PublicAgentTemplateGateRecord,
  audience: PublicClaimProjectionAudience,
): PublicAgentTemplateGateProjection => ({
  caveatRefs: safeRefs('gate caveat refs', gate.caveatRefs),
  customerRefs: customerRefsForAudience(gate.customerRefs, audience),
  evidenceRefs: safeRefs('gate evidence refs', gate.evidenceRefs),
  gateRef: safeRef(gate.gateRef) ?? gate.gateRef,
  labelRef: safeRef(gate.labelRef) ?? gate.labelRef,
  operatorRefs: operatorRefsForAudience(gate.operatorRefs, audience),
  state: gate.state,
  teamRefs: teamRefsForAudience(gate.teamRefs, audience),
})

const projectTimelineEvent = (
  event: PublicAgentTemplateTimelineEventRecord,
  audience: PublicClaimProjectionAudience,
): PublicAgentTemplateTimelineEventProjection => {
  if (valueHasPrivateMaterial(event.updatedAt)) {
    throw new PublicAgentTemplateUnsafe({
      reason: 'Timeline updatedAt contains private material.',
    })
  }

  return {
    artifactRefs: safeRefs('timeline artifact refs', event.artifactRefs),
    caveatRefs: safeRefs('timeline caveat refs', event.caveatRefs),
    customerRefs: customerRefsForAudience(event.customerRefs, audience),
    eventRef: safeRef(event.eventRef) ?? event.eventRef,
    operatorRefs: operatorRefsForAudience(event.operatorRefs, audience),
    proofRefs: safeRefs('timeline proof refs', event.proofRefs),
    state: event.state,
    teamRefs: teamRefsForAudience(event.teamRefs, audience),
    titleRef: safeRef(event.titleRef) ?? event.titleRef,
    updatedAt: event.updatedAt,
  }
}

export const publicAgentTemplateHasPrivateMaterial =
  valueHasPrivateMaterial

export const projectPublicAgentTemplate = (
  record: PublicAgentTemplateRecord,
  audience: PublicClaimProjectionAudience,
): PublicAgentTemplateProjection => {
  assertRecordSafe(record)

  const claim = projectPublicAgentTemplateClaim(record.claim, audience)

  const projection: PublicAgentTemplateProjection = {
    agentId: safeRef(record.agentId) ?? record.agentId,
    agentRef: safeRef(record.agentRef) ?? record.agentRef,
    artifactRefs: safeRefs('artifact refs', record.artifactRefs),
    audience,
    caveatRefs: safeRefs('caveat refs', record.caveatRefs),
    claim,
    currentStateRef: safeRef(record.currentStateRef) ?? record.currentStateRef,
    customerRefs: customerRefsForAudience(record.customerRefs, audience),
    displayName: safeRef(record.displayName) ?? record.displayName,
    eventTimeline: record.eventTimeline.map(event =>
      projectTimelineEvent(event, audience),
    ),
    gates: record.gates.map(gate => projectGate(gate, audience)),
    health: record.health,
    objectiveRef: safeRef(record.objectiveRef) ?? record.objectiveRef,
    operatorRefs: operatorRefsForAudience(record.operatorRefs, audience),
    proofRefs: safeRefs('proof refs', record.proofRefs),
    publicUrls: safePublicUrls(record.publicUrls),
    source: record.source,
    teamRefs: teamRefsForAudience(record.teamRefs, audience),
    updatedAt: record.updatedAt,
  }

  if (valueHasPrivateMaterial(projection)) {
    throw new PublicAgentTemplateUnsafe({
      reason: 'Public agent template projection contains private material.',
    })
  }

  return projection
}

const projectPublicAgentTemplateClaim = (
  claim: PublicClaimProjectionRecord,
  audience: PublicClaimProjectionAudience,
): PublicClaimProjection => {
  try {
    return projectPublicClaimRecord(claim, audience)
  } catch (error) {
    if (error instanceof PublicClaimProjectionUnsafe) {
      throw new PublicAgentTemplateUnsafe({ reason: error.reason })
    }

    throw error
  }
}

const exampleClaimForSource = (
  source: Exclude<PublicAgentTemplateSource, 'general'>,
): PublicClaimProjectionRecord => ({
  caveatRefs: [`caveat.public_agent.${source}.scope`],
  claimId: `claim_${source}_public_template`,
  claimKind: 'agent_challenge',
  claimRef: `claim.public_agent.${source}.template`,
  customerRefs: [`customer_ref.public_agent.${source}.reviewer`],
  desiredState: source === 'adjutant' ? 'measured' : 'verified',
  evidenceRefs: [
    `agent:${source}`,
    `receipt:public_agent:${source}:projection`,
  ],
  operatorRefs: [`operator_ref.public_agent.${source}.runbook`],
  sourceRefs: [`source.public_agent.${source}.template`],
  subjectRef: `agent:${source}`,
  surface: 'public_agent',
  teamRefs: [`team_ref.public_agent.${source}.stewardship`],
  titleRef: `title.public_agent.${source}.template`,
  updatedAt: '2026-06-06T18:00:00.000Z',
})

const sourceExampleConfig = {
  adjutant: {
    currentStateRef: 'state.public_agent.adjutant.supervising_sites',
    displayName: 'Adjutant',
    health: 'running',
    objectiveRef: 'objective.public_agent.adjutant.sites_supervision',
    publicUrls: ['https://openagents.com/adjutant'],
  },
  artanis: {
    currentStateRef: 'state.public_agent.artanis.pylon_release',
    displayName: 'Artanis',
    health: 'healthy',
    objectiveRef: 'objective.public_agent.artanis.pylon_campaign',
    publicUrls: ['https://openagents.com/artanis'],
  },
} satisfies Record<Exclude<PublicAgentTemplateSource, 'general'>, {
  currentStateRef: string
  displayName: string
  health: PublicAgentTemplateHealth
  objectiveRef: string
  publicUrls: ReadonlyArray<string>
}>

export const publicAgentTemplateSourceExample = (
  source: Exclude<PublicAgentTemplateSource, 'general'>,
): PublicAgentTemplateRecord => {
  const config = sourceExampleConfig[source]

  return {
    agentId: `agent_${source}`,
    agentRef: source,
    artifactRefs: [`artifact_public_agent_${source}_overview`],
    caveatRefs: [`caveat.public_agent.${source}.public_projection_only`],
    claim: exampleClaimForSource(source),
    currentStateRef: config.currentStateRef,
    customerRefs: [`customer_ref.public_agent.${source}.reviewer`],
    displayName: config.displayName,
    eventTimeline: [
      {
        artifactRefs: [`artifact_public_agent_${source}_overview`],
        caveatRefs: [`caveat.public_agent.${source}.timeline_public_refs_only`],
        customerRefs: [`customer_ref.public_agent.${source}.reviewer`],
        eventRef: `event.public_agent.${source}.projection_ready`,
        operatorRefs: [`operator_ref.public_agent.${source}.source_audit`],
        proofRefs: [`receipt:public_agent:${source}:projection`],
        state: 'verified',
        teamRefs: [`team_ref.public_agent.${source}.stewardship`],
        titleRef: `title.public_agent.${source}.projection_ready`,
        updatedAt: '2026-06-06T18:00:00.000Z',
      },
    ],
    gates: [
      {
        caveatRefs: [`caveat.public_agent.${source}.public_only`],
        customerRefs: [`customer_ref.public_agent.${source}.reviewer`],
        evidenceRefs: [`agent:${source}`],
        gateRef: `gate.public_agent.${source}.safe_projection`,
        labelRef: `label.public_agent.${source}.safe_projection`,
        operatorRefs: [`operator_ref.public_agent.${source}.policy`],
        state: 'ready',
        teamRefs: [`team_ref.public_agent.${source}.stewardship`],
      },
    ],
    health: config.health,
    objectiveRef: config.objectiveRef,
    operatorRefs: [`operator_ref.public_agent.${source}.runbook`],
    proofRefs: [`receipt:public_agent:${source}:projection`],
    publicUrls: [...config.publicUrls],
    source,
    teamRefs: [`team_ref.public_agent.${source}.stewardship`],
    updatedAt: '2026-06-06T18:00:00.000Z',
  }
}
