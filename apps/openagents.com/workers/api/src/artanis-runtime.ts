import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisRuntimeAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type ArtanisRuntimeAudience = typeof ArtanisRuntimeAudience.Type

export const ArtanisRuntimeState = S.Literals([
  'blocked',
  'idle',
  'paused',
  'queued',
  'running',
  'waiting_for_approval',
])
export type ArtanisRuntimeState = typeof ArtanisRuntimeState.Type

export const ArtanisRuntimeOperatingMode = S.Literals([
  'operator_steered',
  'paused',
  'standalone_autonomous',
])
export type ArtanisRuntimeOperatingMode =
  typeof ArtanisRuntimeOperatingMode.Type

export const ArtanisRuntimeAuthorityBoundary = S.Literals([
  'read_only_artanis_runtime',
])
export type ArtanisRuntimeAuthorityBoundary =
  typeof ArtanisRuntimeAuthorityBoundary.Type

export class ArtanisRuntimeAuthority extends S.Class<ArtanisRuntimeAuthority>(
  'ArtanisRuntimeAuthority',
)({
  authorityBoundary: ArtanisRuntimeAuthorityBoundary,
  noAdapterInstall: S.Boolean,
  noDeployment: S.Boolean,
  noPaymentSpend: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noSettlementMutation: S.Boolean,
  noTrainingLaunch: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export class ArtanisRuntimeRecord extends S.Class<ArtanisRuntimeRecord>(
  'ArtanisRuntimeRecord',
)({
  adjutantBoundaryRefs: S.Array(S.String),
  agentId: S.String,
  agentRef: S.String,
  authority: ArtanisRuntimeAuthority,
  blockerRefs: S.Array(S.String),
  campaignRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  displayName: S.String,
  forumRefs: S.Array(S.String),
  genericAgentBoundaryRefs: S.Array(S.String),
  goalRefs: S.Array(S.String),
  mode: ArtanisRuntimeOperatingMode,
  modelLabRefs: S.Array(S.String),
  nexusRefs: S.Array(S.String),
  operatorSteeringRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicProjectionRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  pylonRefs: S.Array(S.String),
  runtimeRef: S.String,
  state: ArtanisRuntimeState,
  updatedAtIso: S.String,
  workLoopRefs: S.Array(S.String),
}) {}

export class ArtanisRuntimeProjection extends S.Class<ArtanisRuntimeProjection>(
  'ArtanisRuntimeProjection',
)({
  adapterInstallAllowed: S.Boolean,
  adjutantBoundaryRefs: S.Array(S.String),
  agentId: S.String,
  agentRef: S.String,
  audience: ArtanisRuntimeAudience,
  authority: ArtanisRuntimeAuthority,
  blockerRefs: S.Array(S.String),
  campaignRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deploymentAllowed: S.Boolean,
  differsFromAdjutant: S.Boolean,
  differsFromGenericPublicAgent: S.Boolean,
  displayName: S.String,
  forumRefs: S.Array(S.String),
  genericAgentBoundaryRefs: S.Array(S.String),
  goalRefs: S.Array(S.String),
  mode: ArtanisRuntimeOperatingMode,
  modelLabRefs: S.Array(S.String),
  nexusRefs: S.Array(S.String),
  operatorSteerable: S.Boolean,
  operatorSteeringRefs: S.Array(S.String),
  paymentSpendAllowed: S.Boolean,
  privateEvidenceRefs: S.Array(S.String),
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  publicProjectionRefs: S.Array(S.String),
  publicUrls: S.Array(S.String),
  pylonRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Boolean,
  runtimeRef: S.String,
  settlementMutationAllowed: S.Boolean,
  standalone: S.Boolean,
  state: ArtanisRuntimeState,
  trainingLaunchAllowed: S.Boolean,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
  workLoopRefs: S.Array(S.String),
}) {}

export class ArtanisRuntimeUnsafe extends S.TaggedErrorClass<ArtanisRuntimeUnsafe>()(
  'ArtanisRuntimeUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_RUNTIME_READ_ONLY_AUTHORITY: ArtanisRuntimeAuthority = {
  authorityBoundary: 'read_only_artanis_runtime',
  noAdapterInstall: true,
  noDeployment: true,
  noPaymentSpend: true,
  noProviderMutation: true,
  noPublicClaimUpgrade: true,
  noRuntimePromotion: true,
  noSettlementMutation: true,
  noTrainingLaunch: true,
  noWalletSpend: true,
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeArtanisRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(agent\.private|blocker\.private|campaign\.private|caveat\.private|evidence\.private|forum\.private|goal\.private|loop\.private|model_lab\.private|nexus\.private|operator\.private|projection\.private|pylon\.private|runtime\.private|source\.|steering\.private)/i
const agentUnsafeRefPattern =
  /(agent\.private|campaign\.private|evidence\.private|goal\.private|loop\.private|model_lab\.private|nexus\.private|operator\.private|projection\.private|pylon\.private|runtime\.private|source\.private|steering\.private)/i
const customerUnsafeRefPattern = agentUnsafeRefPattern
const privateEvidenceVisibleAudiences: ReadonlyArray<ArtanisRuntimeAudience> = [
  'operator',
  'team',
]

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeArtanisRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisRuntimeUnsafe({
      reason: `${label} contains provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, raw timestamp, or other unsafe material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: ArtanisRuntimeAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisRuntimeAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: ArtanisRuntimeAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const safePublicUrl = (url: string): string | undefined => {
  const trimmed = url.trim()

  try {
    const parsed = new URL(trimmed)

    return parsed.protocol === 'https:' &&
      parsed.hostname === 'openagents.com' &&
      parsed.search === '' &&
      parsed.hash === '' &&
      !unsafeArtanisRefPattern.test(trimmed)
      ? trimmed
      : undefined
  } catch {
    return undefined
  }
}

const assertPublicUrls = (urls: ReadonlyArray<string>): void => {
  if (
    !hasAny(urls) ||
    urls.map(safePublicUrl).some(url => url === undefined)
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason:
        'Artanis public URLs must be clean first-party HTTPS OpenAgents URLs.',
    })
  }
}

const publicUrlsForAudience = (
  urls: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(urls).map(url => safePublicUrl(url)).filter(
    (url): url is string => url !== undefined,
  )

const assertReadOnlyAuthority = (authority: ArtanisRuntimeAuthority): void => {
  if (
    authority.noAdapterInstall !== true ||
    authority.noDeployment !== true ||
    authority.noPaymentSpend !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noSettlementMutation !== true ||
    authority.noTrainingLaunch !== true ||
    authority.noWalletSpend !== true
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason:
        'Artanis runtime records are not authority to spend, mutate providers, launch training, install adapters, promote runtime behavior, deploy, settle, or upgrade public claims.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisRuntimeUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertRecord = (record: ArtanisRuntimeRecord): void => {
  assertReadOnlyAuthority(record.authority)
  assertValidIso('createdAtIso', record.createdAtIso)
  assertValidIso('updatedAtIso', record.updatedAtIso)
  assertPublicUrls(record.publicUrls)
  assertSafeRefs('Artanis identity refs', [
    record.agentId,
    record.agentRef,
    record.displayName,
    record.runtimeRef,
  ])
  assertSafeRefs('Artanis Adjutant boundary refs', record.adjutantBoundaryRefs)
  assertSafeRefs('Artanis blocker refs', record.blockerRefs)
  assertSafeRefs('Artanis campaign refs', record.campaignRefs)
  assertSafeRefs('Artanis caveat refs', record.caveatRefs)
  assertSafeRefs('Artanis Forum refs', record.forumRefs)
  assertSafeRefs(
    'Artanis generic public-agent boundary refs',
    record.genericAgentBoundaryRefs,
  )
  assertSafeRefs('Artanis goal refs', record.goalRefs)
  assertSafeRefs('Artanis Model Lab refs', record.modelLabRefs)
  assertSafeRefs('Artanis Nexus refs', record.nexusRefs)
  assertSafeRefs('Artanis operator steering refs', record.operatorSteeringRefs)
  assertSafeRefs('Artanis private evidence refs', record.privateEvidenceRefs)
  assertSafeRefs(
    'Artanis public projection refs',
    record.publicProjectionRefs,
  )
  assertSafeRefs('Artanis Pylon refs', record.pylonRefs)
  assertSafeRefs('Artanis work loop refs', record.workLoopRefs)

  if (
    record.agentId !== 'agent_artanis' ||
    record.agentRef !== 'artanis' ||
    record.displayName !== 'Artanis'
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason:
        'Standalone Artanis runtime records must use agent_artanis / artanis / Artanis identity.',
    })
  }

  if (
    !hasAny(record.goalRefs) ||
    !hasAny(record.workLoopRefs) ||
    !hasAny(record.privateEvidenceRefs) ||
    !hasAny(record.publicProjectionRefs) ||
    !hasAny(record.forumRefs) ||
    !hasAny(record.modelLabRefs) ||
    !hasAny(record.pylonRefs) ||
    !hasAny(record.nexusRefs) ||
    !hasAny(record.campaignRefs)
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason:
        'Standalone Artanis requires goal, work-loop, private-evidence, public-projection, Forum, Model Lab, Pylon, Nexus, and campaign refs.',
    })
  }

  if (
    !hasAny(record.adjutantBoundaryRefs) ||
    !hasAny(record.genericAgentBoundaryRefs)
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason:
        'Artanis runtime records must define how Artanis differs from Adjutant and generic public agents.',
    })
  }

  if (record.state === 'blocked' && !hasAny(record.blockerRefs)) {
    throw new ArtanisRuntimeUnsafe({
      reason: 'Blocked Artanis runtime state requires blocker refs.',
    })
  }

  if (
    record.state === 'waiting_for_approval' &&
    !hasAny(record.operatorSteeringRefs)
  ) {
    throw new ArtanisRuntimeUnsafe({
      reason: 'Approval-waiting Artanis state requires operator steering refs.',
    })
  }
}

export const projectArtanisRuntime = (
  record: ArtanisRuntimeRecord,
  audience: ArtanisRuntimeAudience,
  nowIso: string,
): ArtanisRuntimeProjection => {
  assertRecord(record)

  return {
    adapterInstallAllowed: !record.authority.noAdapterInstall,
    adjutantBoundaryRefs: refsForAudience(
      'Artanis Adjutant boundary refs',
      record.adjutantBoundaryRefs,
      audience,
    ),
    agentId: refForAudience(
      'Artanis agent id',
      record.agentId,
      audience,
      'agent.redacted.artanis',
    ),
    agentRef: refForAudience(
      'Artanis agent ref',
      record.agentRef,
      audience,
      'agent.redacted.artanis',
    ),
    audience,
    authority: record.authority,
    blockerRefs: refsForAudience(
      'Artanis blocker refs',
      record.blockerRefs,
      audience,
    ),
    campaignRefs: refsForAudience(
      'Artanis campaign refs',
      record.campaignRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    deploymentAllowed: !record.authority.noDeployment,
    differsFromAdjutant: hasAny(record.adjutantBoundaryRefs),
    differsFromGenericPublicAgent: hasAny(record.genericAgentBoundaryRefs),
    displayName: record.displayName,
    forumRefs: refsForAudience('Artanis Forum refs', record.forumRefs, audience),
    genericAgentBoundaryRefs: refsForAudience(
      'Artanis generic public-agent boundary refs',
      record.genericAgentBoundaryRefs,
      audience,
    ),
    goalRefs: refsForAudience('Artanis goal refs', record.goalRefs, audience),
    mode: record.mode,
    modelLabRefs: refsForAudience(
      'Artanis Model Lab refs',
      record.modelLabRefs,
      audience,
    ),
    nexusRefs: refsForAudience(
      'Artanis Nexus refs',
      record.nexusRefs,
      audience,
    ),
    operatorSteerable: hasAny(record.operatorSteeringRefs),
    operatorSteeringRefs: refsForAudience(
      'Artanis operator steering refs',
      record.operatorSteeringRefs,
      audience,
    ),
    paymentSpendAllowed: !record.authority.noPaymentSpend,
    privateEvidenceRefs: privateEvidenceVisibleAudiences.includes(audience)
      ? refsForAudience(
        'Artanis private evidence refs',
        record.privateEvidenceRefs,
        audience,
      )
      : [],
    providerMutationAllowed: !record.authority.noProviderMutation,
    publicClaimUpgradeAllowed: !record.authority.noPublicClaimUpgrade,
    publicProjectionRefs: refsForAudience(
      'Artanis public projection refs',
      record.publicProjectionRefs,
      audience,
    ),
    publicUrls: publicUrlsForAudience(record.publicUrls),
    pylonRefs: refsForAudience('Artanis Pylon refs', record.pylonRefs, audience),
    runtimePromotionAllowed: !record.authority.noRuntimePromotion,
    runtimeRef: refForAudience(
      'Artanis runtime ref',
      record.runtimeRef,
      audience,
      'runtime.redacted.artanis',
    ),
    settlementMutationAllowed: !record.authority.noSettlementMutation,
    standalone: true,
    state: record.state,
    trainingLaunchAllowed: !record.authority.noTrainingLaunch,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    walletSpendAllowed: !record.authority.noWalletSpend,
    workLoopRefs: refsForAudience(
      'Artanis work loop refs',
      record.workLoopRefs,
      audience,
    ),
  }
}

const projectionStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStringValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStringValues)
  }

  return []
}

export const artanisRuntimeProjectionHasPrivateMaterial = (
  projection: ArtanisRuntimeProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      unsafeArtanisRefPattern.test(value) ||
      rawTimestampPattern.test(value),
  )

export const exampleArtanisRuntime = (): ArtanisRuntimeRecord => ({
  adjutantBoundaryRefs: ['boundary.public.artanis_not_sites_supervisor'],
  agentId: 'agent_artanis',
  agentRef: 'artanis',
  authority: ARTANIS_RUNTIME_READ_ONLY_AUTHORITY,
  blockerRefs: [],
  campaignRefs: ['campaign.public.pylon_r10_episode_232'],
  caveatRefs: ['caveat.public.artanis_runtime_not_authority'],
  createdAtIso: '2026-06-07T00:30:00.000Z',
  displayName: 'Artanis',
  forumRefs: ['forum.public.artanis.status'],
  genericAgentBoundaryRefs: ['boundary.public.artanis_standalone_runtime'],
  goalRefs: ['goal.public.artanis.pylon_model_lab'],
  mode: 'standalone_autonomous',
  modelLabRefs: ['model_lab.public.autopilot_continual_learning'],
  nexusRefs: ['nexus.public.pylon_work_market'],
  operatorSteeringRefs: ['steering.public.autopilot_artanis'],
  privateEvidenceRefs: ['evidence.private.artanis.operator_loop_packet'],
  publicProjectionRefs: ['projection.public.artanis.status'],
  publicUrls: [
    'https://openagents.com/artanis',
    'https://openagents.com/agents/artanis',
  ],
  pylonRefs: ['pylon.public.v0_2_readiness'],
  runtimeRef: 'runtime.public.artanis.standalone_v1',
  state: 'running',
  updatedAtIso: '2026-06-07T00:36:00.000Z',
  workLoopRefs: ['loop.public.artanis.autonomous_tick'],
})
