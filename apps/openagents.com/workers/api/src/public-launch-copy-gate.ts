import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const PublicLaunchCopyGateSchemaVersion =
  'omega.public_launch_copy_gate.v1'

export const PublicLaunchCopySurfaceKind = S.Literals([
  'agents_doc',
  'artanis_report',
  'dashboard',
  'forum_seed',
  'launch_announcement',
  'manifest',
  'openapi',
  'page',
  'template',
])
export type PublicLaunchCopySurfaceKind =
  typeof PublicLaunchCopySurfaceKind.Type

export const PublicLaunchCopyGateState = S.Literals(['blocked', 'ready'])
export type PublicLaunchCopyGateState = typeof PublicLaunchCopyGateState.Type

export const PublicLaunchCopyClaimKind = S.Literals([
  'artanis_unbounded_autonomy',
  'creator_spendable_settlement',
  'full_gepa_network_live',
  'hosted_mdk_direct_payouts_live',
  'provider_capacity_live',
  'pylon_broad_earning_live',
  'qwen_remote_finetune_live',
  'referral_sats_stream_live',
])
export type PublicLaunchCopyClaimKind = typeof PublicLaunchCopyClaimKind.Type

export class PublicLaunchCopyEvidenceGate extends S.Class<PublicLaunchCopyEvidenceGate>(
  'PublicLaunchCopyEvidenceGate',
)({
  blockerRefs: S.Array(S.String),
  gateRef: S.String,
  state: PublicLaunchCopyGateState,
  unsafeCopyAllowed: S.Boolean,
}) {}

export class PublicLaunchCopySurface extends S.Class<PublicLaunchCopySurface>(
  'PublicLaunchCopySurface',
)({
  evidenceRefs: S.Array(S.String),
  kind: PublicLaunchCopySurfaceKind,
  surfaceRef: S.String,
  text: S.String,
}) {}

export class PublicLaunchCopyPhrasePolicy extends S.Class<PublicLaunchCopyPhrasePolicy>(
  'PublicLaunchCopyPhrasePolicy',
)({
  claimKind: PublicLaunchCopyClaimKind,
  description: S.String,
  phraseRef: S.String,
  requiredGateRefs: S.Array(S.String),
  safeCopy: S.String,
}) {}

export class PublicLaunchCopyViolation extends S.Class<PublicLaunchCopyViolation>(
  'PublicLaunchCopyViolation',
)({
  blockerRefs: S.Array(S.String),
  claimKind: PublicLaunchCopyClaimKind,
  matchedText: S.String,
  phraseRef: S.String,
  requiredGateRefs: S.Array(S.String),
  safeCopy: S.String,
  surfaceKind: PublicLaunchCopySurfaceKind,
  surfaceRef: S.String,
}) {}

export class PublicLaunchCopySurfaceProjection extends S.Class<PublicLaunchCopySurfaceProjection>(
  'PublicLaunchCopySurfaceProjection',
)({
  evidenceRefs: S.Array(S.String),
  kind: PublicLaunchCopySurfaceKind,
  state: PublicLaunchCopyGateState,
  surfaceRef: S.String,
  violationRefs: S.Array(S.String),
}) {}

export class PublicLaunchCopyGateProjection extends S.Class<PublicLaunchCopyGateProjection>(
  'PublicLaunchCopyGateProjection',
)({
  blockerRefs: S.Array(S.String),
  evidenceGateRefs: S.Array(S.String),
  healthFresh: S.Boolean,
  safeCopyRefs: S.Array(S.String),
  schemaVersion: S.Literal(PublicLaunchCopyGateSchemaVersion),
  state: PublicLaunchCopyGateState,
  surfaces: S.Array(PublicLaunchCopySurfaceProjection),
  violations: S.Array(PublicLaunchCopyViolation),
}) {}

export class PublicLaunchCopyGateUnsafe extends S.TaggedErrorClass<PublicLaunchCopyGateUnsafe>()(
  'PublicLaunchCopyGateUnsafe',
  {
    reason: S.String,
  },
) {}

type PhraseRule = Readonly<{
  policy: PublicLaunchCopyPhrasePolicy
  pattern: RegExp
}>

type PhraseMatch = Readonly<{
  index: number
  text: string
}>

export type PublicLaunchCopyGateInput = Readonly<{
  evidenceGates: ReadonlyArray<PublicLaunchCopyEvidenceGate>
  healthFresh?: boolean | undefined
  surfaces: ReadonlyArray<PublicLaunchCopySurface>
}>

export const PublicLaunchCopyGateRefs = {
  artanisProductionLaunch: 'gate.public.artanis.production_launch.v1',
  creatorSpendableSettlement:
    'gate.public.forum.creator_spendable_settlement.v1',
  fullGepaNetwork: 'gate.public.probe_gepa_paid_mode_campaign_ladder.v1',
  hostedMdkDirectPayouts: 'gate.public.mdk.hosted_direct_payouts.v1',
  providerCapacity: 'gate.public.provider_capacity_marketplace.v1',
  pylonBroadEarning: 'gate.public.pylon.earning_network_counters.v1',
  qwenRemoteFineTune: 'gate.public.qwen_remote_finetune.v1',
  referralSatsStream: 'gate.public.site_referral.bitcoin_rewards.v1',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,320}$/
const unsafeMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|bolt11|bolt12|callback[_-]?token|cookie|customer[_-]?(email|name|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(
    ref =>
      !safeRefPattern.test(ref) ||
      containsProviderSecretMaterial(ref) ||
      unsafeMaterialPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PublicLaunchCopyGateUnsafe({
      reason: `${label} must contain public-safe refs only, without secrets, private paths, raw payment material, wallet material, provider payloads, raw prompts, raw logs, private repos, or timestamps.`,
    })
  }

  return normalized
}

const phrasePolicy = (
  input: Readonly<{
    claimKind: PublicLaunchCopyClaimKind
    description: string
    phraseRef: string
    requiredGateRefs: ReadonlyArray<string>
    safeCopy: string
  }>,
): PublicLaunchCopyPhrasePolicy =>
  new PublicLaunchCopyPhrasePolicy({
    claimKind: input.claimKind,
    description: input.description,
    phraseRef: input.phraseRef,
    requiredGateRefs: [...input.requiredGateRefs],
    safeCopy: input.safeCopy,
  })

const phraseRules: ReadonlyArray<PhraseRule> = [
  {
    pattern:
      /\b(?:one install|install(?:ing)? pylon|pylon)\b[^.!?\n]{0,120}\b(?:earn|earns|earning|get paid|make|makes|making)\b[^.!?\n]{0,80}\b(?:bitcoin|sats?)\b/i,
    policy: phrasePolicy({
      claimKind: 'pylon_broad_earning_live',
      description: 'Broad Pylon earning copy requires live earning counters.',
      phraseRef: 'phrase.public_launch.pylon_broad_earning_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.pylonBroadEarning],
      safeCopy:
        'Pylon is a limited local-compute launcher with earning claims gated by public Pylon readiness and settlement refs.',
    }),
  },
  {
    pattern:
      /\b(?:full|whole|entire)\b[^.!?\n]{0,80}\bgepa\b[^.!?\n]{0,80}\bnetwork\b[^.!?\n]{0,80}\b(?:live|running|launched|paid)\b|\bgepa\b[^.!?\n]{0,80}\b(?:full|whole|entire)\b[^.!?\n]{0,80}\bnetwork\b[^.!?\n]{0,80}\b(?:live|running|launched|paid)\b/i,
    policy: phrasePolicy({
      claimKind: 'full_gepa_network_live',
      description:
        'Full GEPA network copy requires the paid-mode campaign ladder to be green.',
      phraseRef: 'phrase.public_launch.full_gepa_network_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.fullGepaNetwork],
      safeCopy:
        'GEPA has gated unpaid, payable, and settled campaign evidence; full-network paid operation remains gated unless the campaign ladder is green.',
    }),
  },
  {
    pattern:
      /\b(?:fine[- ]?tune|fine[- ]?tuned|training)\b[^.!?\n]{0,80}\bqwen\s*3\.?6\b[^.!?\n]{0,120}\b(?:live|launched|people'?s devices|remote pylon network)\b|\bqwen\s*3\.?6\b[^.!?\n]{0,120}\b(?:fine[- ]?tune|fine[- ]?tuned|training)\b[^.!?\n]{0,120}\b(?:live|launched|people'?s devices|remote pylon network)\b/i,
    policy: phrasePolicy({
      claimKind: 'qwen_remote_finetune_live',
      description:
        'Qwen 3.6 remote fine-tune copy requires the remote Pylon fine-tune gate.',
      phraseRef: 'phrase.public_launch.qwen_remote_finetune_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.qwenRemoteFineTune],
      safeCopy:
        'Qwen 3.6 remote fine-tune claims are blocked until remote worker, shard, eval, admission, payment, settlement, and public projection refs pass.',
    }),
  },
  {
    pattern:
      /\bprovider\b[^.!?\n]{0,100}\b(?:capacity|quota)\b[^.!?\n]{0,100}\b(?:live|marketplace|earning|sellable|paid)\b|\bcapacity marketplace\b[^.!?\n]{0,100}\bprovider\b[^.!?\n]{0,100}\b(?:live|enabled|paid)\b/i,
    policy: phrasePolicy({
      claimKind: 'provider_capacity_live',
      description:
        'Provider capacity marketplace copy requires provider capacity settlement evidence.',
      phraseRef: 'phrase.public_launch.provider_capacity_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.providerCapacity],
      safeCopy:
        'Provider-capacity marketplace claims remain provider-, grant-, ToS-, dispatch-, assignment-, and settlement-gated.',
    }),
  },
  {
    pattern:
      /\breferral\b[^.!?\n]{0,100}\b(?:sats?|bitcoin)\b[^.!?\n]{0,80}\b(?:stream|live|earning|payout|withdrawal)\b|\b(?:sats?|bitcoin)\b[^.!?\n]{0,100}\breferral\b[^.!?\n]{0,80}\b(?:stream|live|earning|payout|withdrawal)\b/i,
    policy: phrasePolicy({
      claimKind: 'referral_sats_stream_live',
      description:
        'Referral sats stream copy requires settlement-backed referral rewards.',
      phraseRef: 'phrase.public_launch.referral_sats_stream_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.referralSatsStream],
      safeCopy:
        'Referral attribution is separate from bitcoin withdrawal; sats copy requires settlement receipt refs.',
    }),
  },
  {
    pattern:
      /\bhosted\s+mdk\b[^.!?\n]{0,100}\b(?:direct\s+)?(?:payouts?|settlement|payments?)\b[^.!?\n]{0,80}\b(?:enabled|live|ready)\b|\bdirect programmatic payouts?\b[^.!?\n]{0,80}\b(?:enabled|live|ready)\b/i,
    policy: phrasePolicy({
      claimKind: 'hosted_mdk_direct_payouts_live',
      description:
        'Hosted MDK direct payout copy requires the hosted direct payout gate.',
      phraseRef: 'phrase.public_launch.hosted_mdk_direct_payouts_live',
      requiredGateRefs: [PublicLaunchCopyGateRefs.hostedMdkDirectPayouts],
      safeCopy:
        'MDK-backed payments are route-specific; hosted direct payouts stay gated unless the hosted payout gate is green.',
    }),
  },
  {
    pattern:
      /\bcreator\b[^.!?\n]{0,100}\b(?:spendable|settled|withdrawable)\b[^.!?\n]{0,80}\b(?:sats?|bitcoin|settlement)\b|\bcreator\b[^.!?\n]{0,80}\bsettlement\b[^.!?\n]{0,80}\b(?:live|confirmed|spendable|withdrawable)\b/i,
    policy: phrasePolicy({
      claimKind: 'creator_spendable_settlement',
      description:
        'Creator spendable settlement copy requires settlement claim evidence.',
      phraseRef: 'phrase.public_launch.creator_spendable_settlement',
      requiredGateRefs: [PublicLaunchCopyGateRefs.creatorSpendableSettlement],
      safeCopy:
        'Creator tips can be described by preview, payment, receipt, and settlement state; spendable settlement requires a public settlement claim ref.',
    }),
  },
  {
    pattern:
      /\bartanis\b[^.!?\n]{0,100}\b(?:fully autonomous|continuously running autonomously|production administrator|autonomously administers)\b/i,
    policy: phrasePolicy({
      claimKind: 'artanis_unbounded_autonomy',
      description:
        'Artanis autonomy copy requires production launch and authority gates.',
      phraseRef: 'phrase.public_launch.artanis_unbounded_autonomy',
      requiredGateRefs: [PublicLaunchCopyGateRefs.artanisProductionLaunch],
      safeCopy:
        'Artanis has a public evidence surface and operator-gated launch path; production authority remains explicitly gated.',
    }),
  },
]

export const PublicLaunchCopyPhrasePolicies: ReadonlyArray<PublicLaunchCopyPhrasePolicy> =
  phraseRules.map(rule => rule.policy)

const firstMatch = (pattern: RegExp, text: string): PhraseMatch | null => {
  const match = pattern.exec(text)
  const matchedText = match?.[0]?.trim()

  return matchedText === undefined || matchedText === ''
    ? null
    : { index: match?.index ?? 0, text: matchedText }
}

const guardedWarningPattern =
  /\b(can(?:not|'t)|do not|don't|must not|never|not yet|should not|unless|without)\b/i
const claimVerbPattern =
  /\b(assume|claim|describe|overclaim|promise|say|state|tell|treat)\b/i

const isGuardedWarningContext = (
  surfaceText: string,
  match: PhraseMatch,
): boolean => {
  const context = surfaceText.slice(
    Math.max(0, match.index - 180),
    Math.min(surfaceText.length, match.index + match.text.length + 180),
  )

  return guardedWarningPattern.test(context) && claimVerbPattern.test(context)
}

const evidenceGateReady = (
  gate: PublicLaunchCopyEvidenceGate,
  healthFresh: boolean,
): boolean =>
  healthFresh && gate.state === 'ready' && gate.unsafeCopyAllowed === true

const surfaceHasEvidenceRef = (
  surface: PublicLaunchCopySurface,
  gateRef: string,
): boolean =>
  surface.evidenceRefs.includes(gateRef) || surface.text.includes(gateRef)

const violationRef = (
  surface: PublicLaunchCopySurface,
  policy: PublicLaunchCopyPhrasePolicy,
): string =>
  `violation.public_launch_copy.${surface.surfaceRef}.${policy.claimKind}`

export const projectPublicLaunchCopyGate = (
  input: PublicLaunchCopyGateInput,
): PublicLaunchCopyGateProjection => {
  const healthFresh = input.healthFresh ?? true
  const evidenceGates = input.evidenceGates.map(
    gate =>
      new PublicLaunchCopyEvidenceGate({
        blockerRefs: assertSafeRefs(
          'Public launch copy evidence gate blocker refs',
          gate.blockerRefs,
        ),
        gateRef:
          assertSafeRefs('Public launch copy evidence gate ref', [
            gate.gateRef,
          ])[0] ?? gate.gateRef,
        state: gate.state,
        unsafeCopyAllowed: gate.unsafeCopyAllowed,
      }),
  )
  const surfaces = input.surfaces.map(
    surface =>
      new PublicLaunchCopySurface({
        evidenceRefs: assertSafeRefs(
          'Public launch copy surface evidence refs',
          surface.evidenceRefs,
        ),
        kind: surface.kind,
        surfaceRef:
          assertSafeRefs('Public launch copy surface ref', [
            surface.surfaceRef,
          ])[0] ?? surface.surfaceRef,
        text: surface.text,
      }),
  )
  const gatesByRef = new Map(evidenceGates.map(gate => [gate.gateRef, gate]))
  const violations = surfaces.flatMap(surface =>
    phraseRules.flatMap(rule => {
      const matchedText = firstMatch(rule.pattern, surface.text)

      if (matchedText === null) {
        return []
      }

      if (isGuardedWarningContext(surface.text, matchedText)) {
        return []
      }

      const matchingGates = rule.policy.requiredGateRefs
        .map(gateRef => gatesByRef.get(gateRef))
        .filter(
          (gate): gate is PublicLaunchCopyEvidenceGate => gate !== undefined,
        )
      const greenGate = matchingGates.find(gate =>
        evidenceGateReady(gate, healthFresh),
      )
      const linkedEvidence =
        greenGate !== undefined &&
        rule.policy.requiredGateRefs.some(gateRef =>
          surfaceHasEvidenceRef(surface, gateRef),
        )

      if (greenGate !== undefined && linkedEvidence) {
        return []
      }

      return [
        new PublicLaunchCopyViolation({
          blockerRefs: uniqueRefs([
            ...(healthFresh ? [] : ['blocker.public_launch_copy.health_stale']),
            ...(greenGate === undefined
              ? [
                  `blocker.public_launch_copy.${rule.policy.claimKind}.gate_not_green`,
                ]
              : []),
            ...(linkedEvidence
              ? []
              : [
                  `blocker.public_launch_copy.${rule.policy.claimKind}.evidence_ref_missing`,
                ]),
            ...matchingGates.flatMap(gate => gate.blockerRefs),
          ]),
          claimKind: rule.policy.claimKind,
          matchedText: matchedText.text,
          phraseRef: rule.policy.phraseRef,
          requiredGateRefs: rule.policy.requiredGateRefs,
          safeCopy: rule.policy.safeCopy,
          surfaceKind: surface.kind,
          surfaceRef: surface.surfaceRef,
        }),
      ]
    }),
  )
  const violationRefs = new Set(
    violations.map(violation =>
      violationRef(
        new PublicLaunchCopySurface({
          evidenceRefs: [],
          kind: violation.surfaceKind,
          surfaceRef: violation.surfaceRef,
          text: '',
        }),
        new PublicLaunchCopyPhrasePolicy({
          claimKind: violation.claimKind,
          description: '',
          phraseRef: violation.phraseRef,
          requiredGateRefs: violation.requiredGateRefs,
          safeCopy: violation.safeCopy,
        }),
      ),
    ),
  )
  const surfaceProjections = surfaces.map(
    surface =>
      new PublicLaunchCopySurfaceProjection({
        evidenceRefs: surface.evidenceRefs,
        kind: surface.kind,
        state: [...violationRefs].some(ref =>
          ref.startsWith(`violation.public_launch_copy.${surface.surfaceRef}.`),
        )
          ? 'blocked'
          : 'ready',
        surfaceRef: surface.surfaceRef,
        violationRefs: [...violationRefs].filter(ref =>
          ref.startsWith(`violation.public_launch_copy.${surface.surfaceRef}.`),
        ),
      }),
  )
  const blockerRefs = uniqueRefs([
    ...(healthFresh ? [] : ['blocker.public_launch_copy.health_stale']),
    ...violations.flatMap(violation => violation.blockerRefs),
  ])
  const state: PublicLaunchCopyGateState =
    blockerRefs.length === 0 && violations.length === 0 ? 'ready' : 'blocked'

  return new PublicLaunchCopyGateProjection({
    blockerRefs,
    evidenceGateRefs: uniqueRefs(evidenceGates.map(gate => gate.gateRef)),
    healthFresh,
    safeCopyRefs: uniqueRefs(
      violations.map(violation => `safe_copy.${violation.claimKind}`),
    ),
    schemaVersion: PublicLaunchCopyGateSchemaVersion,
    state,
    surfaces: surfaceProjections,
    violations,
  })
}

export const publicLaunchCopyProjectionHasPrivateMaterial = (
  projection: PublicLaunchCopyGateProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return (
    unsafeMaterialPattern.test(serialized) ||
    rawTimestampPattern.test(serialized) ||
    containsProviderSecretMaterial(serialized)
  )
}
