import type { BlueprintGateId } from '@openagentsinc/blueprint-contracts'
import { Schema as S } from 'effect'

import {
  ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
  ArtanisAuthorityScope,
  artanisAuthorityScopeEvidenceRef,
  artanisAuthorityScopePublicRef,
} from './artanis-authority-scope'

export const ArtanisAutonomyLadderRung = S.Literals([
  'owner_self_no_spend_dispatch',
  'owner_operator_forum_post',
  'shared_fleet_admin_candidate',
  'treasury_enveloped_spend_candidate',
  'explicit_operator_gate_required',
  'unbounded_autonomy_forbidden',
])
export type ArtanisAutonomyLadderRung = typeof ArtanisAutonomyLadderRung.Type

export const ArtanisAutonomySignatureGateId = S.Literals([
  'fleet-liveness',
  'diagnosis-grounding',
  'issue-close-safe',
  'command-source-verified',
  'merge-deploy',
])
export type ArtanisAutonomySignatureGateId =
  typeof ArtanisAutonomySignatureGateId.Type

export const ArtanisAutonomyCleanTickState = S.Literals([
  'missing',
  'regressed',
  'retained',
])
export type ArtanisAutonomyCleanTickState =
  typeof ArtanisAutonomyCleanTickState.Type

export const ArtanisAutonomyTreasuryEnvelopeState = S.Literals([
  'missing',
  'not_applicable',
  'owner_cap_active',
  'unbounded_requested',
])
export type ArtanisAutonomyTreasuryEnvelopeState =
  typeof ArtanisAutonomyTreasuryEnvelopeState.Type

export class ArtanisAutonomySignatureGateEvidence extends S.Class<ArtanisAutonomySignatureGateEvidence>(
  'ArtanisAutonomySignatureGateEvidence',
)({
  evidenceRefs: S.Array(S.String),
  gateId: ArtanisAutonomySignatureGateId,
  state: S.String,
}) {}

export class ArtanisAutonomyCleanTickTrackRecord extends S.Class<ArtanisAutonomyCleanTickTrackRecord>(
  'ArtanisAutonomyCleanTickTrackRecord',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  state: ArtanisAutonomyCleanTickState,
  tickCount: S.Number,
}) {}

export class ArtanisAutonomyTreasuryEnvelope extends S.Class<ArtanisAutonomyTreasuryEnvelope>(
  'ArtanisAutonomyTreasuryEnvelope',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  perDayCapSat: S.NullOr(S.Number),
  perPayoutCapSat: S.NullOr(S.Number),
  state: ArtanisAutonomyTreasuryEnvelopeState,
}) {}

export class ArtanisAutonomyLadderEvaluationInput extends S.Class<ArtanisAutonomyLadderEvaluationInput>(
  'ArtanisAutonomyLadderEvaluationInput',
)({
  actionRef: S.String,
  authorityScope: ArtanisAuthorityScope,
  cleanTickTrackRecord: ArtanisAutonomyCleanTickTrackRecord,
  riskyActionKind: S.String,
  signatureGates: S.Array(ArtanisAutonomySignatureGateEvidence),
  treasuryEnvelope: ArtanisAutonomyTreasuryEnvelope,
}) {}

export class ArtanisAutonomyLadderProjection extends S.Class<ArtanisAutonomyLadderProjection>(
  'ArtanisAutonomyLadderProjection',
)({
  actionRef: S.String,
  authorityScope: ArtanisAuthorityScope,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  cleanTickTrackRecordRetained: S.Boolean,
  evidenceRefs: S.Array(S.String),
  missingSignatureGateRefs: S.Array(S.String),
  nextGateEligible: S.Boolean,
  requiredSignatureGateRefs: S.Array(S.String),
  riskyActionKind: S.String,
  rung: ArtanisAutonomyLadderRung,
  signatureGatesTerminal: S.Boolean,
  standingApprovalAllowed: S.Boolean,
  treasuryEnvelopeBounded: S.Boolean,
}) {}

type SignatureTerminal = Readonly<{
  gateId: BlueprintGateId
  publicRef: string
  terminalState: string
}>

export const ARTANIS_AUTONOMY_SIGNATURE_TERMINALS: ReadonlyArray<SignatureTerminal> =
  [
    {
      gateId: 'fleet-liveness',
      publicRef: 'gate.public.artanis.autonomy.signature.fleet-liveness.PROVEN_ALIVE',
      terminalState: 'PROVEN_ALIVE',
    },
    {
      gateId: 'diagnosis-grounding',
      publicRef: 'gate.public.artanis.autonomy.signature.diagnosis-grounding.GROUNDED',
      terminalState: 'GROUNDED',
    },
    {
      gateId: 'issue-close-safe',
      publicRef: 'gate.public.artanis.autonomy.signature.issue-close-safe.SAFE_TO_CLOSE',
      terminalState: 'SAFE_TO_CLOSE',
    },
    {
      gateId: 'command-source-verified',
      publicRef:
        'gate.public.artanis.autonomy.signature.command-source-verified.SAFE_TO_PROPOSE',
      terminalState: 'SAFE_TO_PROPOSE',
    },
    {
      gateId: 'merge-deploy',
      publicRef: 'gate.public.artanis.autonomy.signature.merge-deploy.LIVE',
      terminalState: 'LIVE',
    },
  ]

export const ARTANIS_AUTONOMY_CLEAN_TICK_TRACK_RECORD_REF =
  'evidence.public.artanis.autonomy.clean_unattended_ticks.retained'
export const ARTANIS_AUTONOMY_TREASURY_ENVELOPE_REF =
  'policy.public.artanis.autonomy.treasury_owner_cap_envelope'
export const ARTANIS_AUTONOMY_OWNER_PROMOTION_STEP_REF =
  'policy.public.artanis.autonomy.owner_promotes_one_gate_at_a_time'

const treasurySpendKinds = new Set([
  'wallet_spend',
  'settlement',
  'l402_redemption',
])

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(refs.filter(ref => ref.trim().length > 0)),
]

const signatureBlockerRef = (gateId: string): string =>
  `blocker.public.artanis.autonomy.signature.${gateId}.not_terminal`

const currentStandingRung = (
  riskyActionKind: string,
  authorityScope: ArtanisAuthorityScope,
): ArtanisAutonomyLadderRung | null =>
  riskyActionKind === 'pylon_job_dispatch' &&
  authorityScope === ARTANIS_OWNER_SELF_AUTHORITY_SCOPE
    ? 'owner_self_no_spend_dispatch'
    : riskyActionKind === 'forum_post' &&
        authorityScope === ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE
      ? 'owner_operator_forum_post'
      : null

export const artanisAutonomyLadderRungForRiskyAction = (
  riskyActionKind: string,
  authorityScope: ArtanisAuthorityScope,
): ArtanisAutonomyLadderRung => {
  const current = currentStandingRung(riskyActionKind, authorityScope)
  if (current !== null) {
    return current
  }

  if (
    riskyActionKind === 'unbounded_autonomy' ||
    riskyActionKind === 'ungated_production_admin'
  ) {
    return 'unbounded_autonomy_forbidden'
  }

  if (
    authorityScope === ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE ||
    riskyActionKind === 'fleet_mutation'
  ) {
    return 'shared_fleet_admin_candidate'
  }

  if (treasurySpendKinds.has(riskyActionKind)) {
    return 'treasury_enveloped_spend_candidate'
  }

  return 'explicit_operator_gate_required'
}

export const defaultArtanisAutonomyCleanTickTrackRecord =
  (): ArtanisAutonomyCleanTickTrackRecord =>
    new ArtanisAutonomyCleanTickTrackRecord({
      blockerRefs: ['blocker.public.artanis.autonomy.clean_unattended_ticks_missing'],
      evidenceRefs: [],
      state: 'missing',
      tickCount: 0,
    })

export const retainedArtanisAutonomyCleanTickTrackRecord = (input?: {
  evidenceRefs?: ReadonlyArray<string> | undefined
  tickCount?: number | undefined
}): ArtanisAutonomyCleanTickTrackRecord =>
  new ArtanisAutonomyCleanTickTrackRecord({
    blockerRefs: [],
    evidenceRefs: uniqueRefs([
      ARTANIS_AUTONOMY_CLEAN_TICK_TRACK_RECORD_REF,
      ...(input?.evidenceRefs ?? []),
    ]),
    state: 'retained',
    tickCount: input?.tickCount ?? 3,
  })

export const defaultArtanisAutonomyTreasuryEnvelope =
  (): ArtanisAutonomyTreasuryEnvelope =>
    new ArtanisAutonomyTreasuryEnvelope({
      blockerRefs: ['blocker.public.artanis.autonomy.treasury_envelope_missing'],
      evidenceRefs: [],
      perDayCapSat: null,
      perPayoutCapSat: null,
      state: 'missing',
    })

export const ownerCapArtanisAutonomyTreasuryEnvelope = (input: {
  evidenceRefs?: ReadonlyArray<string> | undefined
  perDayCapSat: number
  perPayoutCapSat: number
}): ArtanisAutonomyTreasuryEnvelope =>
  new ArtanisAutonomyTreasuryEnvelope({
    blockerRefs: [],
    evidenceRefs: uniqueRefs([
      ARTANIS_AUTONOMY_TREASURY_ENVELOPE_REF,
      ...(input.evidenceRefs ?? []),
    ]),
    perDayCapSat: input.perDayCapSat,
    perPayoutCapSat: input.perPayoutCapSat,
    state: 'owner_cap_active',
  })

export const notApplicableArtanisAutonomyTreasuryEnvelope =
  (): ArtanisAutonomyTreasuryEnvelope =>
    new ArtanisAutonomyTreasuryEnvelope({
      blockerRefs: [],
      evidenceRefs: [],
      perDayCapSat: null,
      perPayoutCapSat: null,
      state: 'not_applicable',
    })

export const terminalArtanisAutonomySignatureGateEvidence =
  (): ReadonlyArray<ArtanisAutonomySignatureGateEvidence> =>
    ARTANIS_AUTONOMY_SIGNATURE_TERMINALS.map(
      terminal =>
        new ArtanisAutonomySignatureGateEvidence({
          evidenceRefs: [terminal.publicRef],
          gateId: terminal.gateId,
          state: terminal.terminalState,
        }),
    )

export const defaultArtanisAutonomyLadderInput = (input: {
  actionRef?: string | undefined
  authorityScope: ArtanisAuthorityScope
  riskyActionKind: string
}): ArtanisAutonomyLadderEvaluationInput =>
  new ArtanisAutonomyLadderEvaluationInput({
    actionRef: input.actionRef ?? `action.public.artanis.${input.riskyActionKind}`,
    authorityScope: input.authorityScope,
    cleanTickTrackRecord: defaultArtanisAutonomyCleanTickTrackRecord(),
    riskyActionKind: input.riskyActionKind,
    signatureGates: [],
    treasuryEnvelope: notApplicableArtanisAutonomyTreasuryEnvelope(),
  })

export const evaluateArtanisAutonomyLadder = (
  input: ArtanisAutonomyLadderEvaluationInput,
): ArtanisAutonomyLadderProjection => {
  const rung = artanisAutonomyLadderRungForRiskyAction(
    input.riskyActionKind,
    input.authorityScope,
  )
  const signatureEvidenceByGate = new Map(
    input.signatureGates.map(gate => [gate.gateId, gate]),
  )
  const missingSignatureTerminals = ARTANIS_AUTONOMY_SIGNATURE_TERMINALS.filter(
    terminal =>
      signatureEvidenceByGate.get(terminal.gateId)?.state !==
      terminal.terminalState,
  )
  const cleanTickTrackRecordRetained =
    input.cleanTickTrackRecord.state === 'retained' &&
    input.cleanTickTrackRecord.tickCount > 0 &&
    input.cleanTickTrackRecord.blockerRefs.length === 0
  const treasuryEnvelopeBounded =
    input.treasuryEnvelope.state === 'owner_cap_active' &&
    (input.treasuryEnvelope.perDayCapSat ?? 0) > 0 &&
    (input.treasuryEnvelope.perPayoutCapSat ?? 0) > 0 &&
    input.treasuryEnvelope.blockerRefs.length === 0
  const signaturesAndCleanTicksSatisfied =
    missingSignatureTerminals.length === 0 && cleanTickTrackRecordRetained

  const currentStandingAllowed =
    rung === 'owner_self_no_spend_dispatch' ||
    rung === 'owner_operator_forum_post'
  const sharedFleetCandidate = rung === 'shared_fleet_admin_candidate'
  const treasuryCandidate = rung === 'treasury_enveloped_spend_candidate'
  const nextGateEligible =
    (sharedFleetCandidate && signaturesAndCleanTicksSatisfied) ||
    (treasuryCandidate &&
      signaturesAndCleanTicksSatisfied &&
      treasuryEnvelopeBounded)

  const blockerRefs = [
    ...(currentStandingAllowed
      ? []
      : ['blocker.public.artanis.autonomy.standing_approval_not_granted']),
    ...(rung === 'unbounded_autonomy_forbidden'
      ? ['blocker.public.artanis.autonomy.unbounded_autonomy_forbidden']
      : []),
    ...(sharedFleetCandidate || treasuryCandidate
      ? missingSignatureTerminals.map(terminal =>
          signatureBlockerRef(terminal.gateId),
        )
      : []),
    ...(sharedFleetCandidate || treasuryCandidate
      ? cleanTickTrackRecordRetained
        ? []
        : [
            'blocker.public.artanis.autonomy.clean_unattended_ticks_not_retained',
            ...input.cleanTickTrackRecord.blockerRefs,
          ]
      : []),
    ...(treasuryCandidate
      ? treasuryEnvelopeBounded
        ? []
        : [
            ...(input.treasuryEnvelope.state === 'unbounded_requested'
              ? [
                  'blocker.public.artanis.autonomy.unbounded_treasury_forbidden',
                ]
              : ['blocker.public.artanis.autonomy.treasury_envelope_missing']),
            ...input.treasuryEnvelope.blockerRefs,
          ]
      : []),
  ]

  return new ArtanisAutonomyLadderProjection({
    actionRef: input.actionRef,
    authorityScope: input.authorityScope,
    blockerRefs: uniqueRefs(blockerRefs),
    caveatRefs: uniqueRefs([
      artanisAuthorityScopePublicRef(input.authorityScope),
      ...(currentStandingAllowed
        ? [
            'caveat.public.artanis.autonomy.current_standing_approval_only',
            'caveat.public.artanis.autonomy.no_shared_fleet_or_money_movement',
          ]
        : []),
      ...(nextGateEligible
        ? [ARTANIS_AUTONOMY_OWNER_PROMOTION_STEP_REF]
        : []),
      ...(treasuryCandidate
        ? ['caveat.public.artanis.autonomy.treasury_owner_cap_required']
        : []),
    ]),
    cleanTickTrackRecordRetained,
    evidenceRefs: uniqueRefs([
      artanisAuthorityScopeEvidenceRef(input.authorityScope),
      ...input.signatureGates.flatMap(gate => gate.evidenceRefs),
      ...input.cleanTickTrackRecord.evidenceRefs,
      ...input.treasuryEnvelope.evidenceRefs,
    ]),
    missingSignatureGateRefs: uniqueRefs(
      missingSignatureTerminals.map(terminal => terminal.publicRef),
    ),
    nextGateEligible,
    requiredSignatureGateRefs: uniqueRefs(
      ARTANIS_AUTONOMY_SIGNATURE_TERMINALS.map(terminal => terminal.publicRef),
    ),
    riskyActionKind: input.riskyActionKind,
    rung,
    signatureGatesTerminal: missingSignatureTerminals.length === 0,
    standingApprovalAllowed: currentStandingAllowed,
    treasuryEnvelopeBounded,
  })
}

export const artanisAutonomyLadderAllowsStandingApproval = (input: {
  authorityScope: ArtanisAuthorityScope
  riskyActionKind: string
}): boolean =>
  evaluateArtanisAutonomyLadder(
    defaultArtanisAutonomyLadderInput({
      authorityScope: input.authorityScope,
      riskyActionKind: input.riskyActionKind,
    }),
  ).standingApprovalAllowed
