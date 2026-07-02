import { describe, expect, test } from 'vitest'

import {
  ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
} from './artanis-authority-scope'
import {
  ARTANIS_AUTONOMY_CLEAN_TICK_TRACK_RECORD_REF,
  ARTANIS_AUTONOMY_TREASURY_ENVELOPE_REF,
  ArtanisAutonomyLadderEvaluationInput,
  ArtanisAutonomyTreasuryEnvelope,
  defaultArtanisAutonomyCleanTickTrackRecord,
  defaultArtanisAutonomyLadderInput,
  defaultArtanisAutonomyTreasuryEnvelope,
  evaluateArtanisAutonomyLadder,
  notApplicableArtanisAutonomyTreasuryEnvelope,
  ownerCapArtanisAutonomyTreasuryEnvelope,
  retainedArtanisAutonomyCleanTickTrackRecord,
  terminalArtanisAutonomySignatureGateEvidence,
} from './artanis-autonomy-ladder'

describe('Artanis autonomy ladder', () => {
  test('keeps current pylon dispatch standing approval owner-self and no-spend', () => {
    const projection = evaluateArtanisAutonomyLadder(
      defaultArtanisAutonomyLadderInput({
        authorityScope: ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
        riskyActionKind: 'pylon_job_dispatch',
      }),
    )

    expect(projection).toMatchObject({
      authorityScope: 'owner_self',
      nextGateEligible: false,
      riskyActionKind: 'pylon_job_dispatch',
      rung: 'owner_self_no_spend_dispatch',
      standingApprovalAllowed: true,
      treasuryEnvelopeBounded: false,
    })
    expect(projection.blockerRefs).toEqual([])
    expect(projection.caveatRefs).toEqual(
      expect.arrayContaining([
        'authority.public.artanis.scope.owner_self',
        'caveat.public.artanis.autonomy.current_standing_approval_only',
        'caveat.public.artanis.autonomy.no_shared_fleet_or_money_movement',
      ]),
    )
  })

  test('keeps current forum post standing approval owner-operator only', () => {
    const projection = evaluateArtanisAutonomyLadder(
      defaultArtanisAutonomyLadderInput({
        authorityScope: ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
        riskyActionKind: 'forum_post',
      }),
    )

    expect(projection).toMatchObject({
      authorityScope: 'owner_operator',
      nextGateEligible: false,
      riskyActionKind: 'forum_post',
      rung: 'owner_operator_forum_post',
      standingApprovalAllowed: true,
    })
    expect(projection.blockerRefs).toEqual([])
  })

  test('blocks shared-fleet standing approval until signatures and clean ticks exist', () => {
    const projection = evaluateArtanisAutonomyLadder(
      defaultArtanisAutonomyLadderInput({
        authorityScope: ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
        riskyActionKind: 'pylon_job_dispatch',
      }),
    )

    expect(projection).toMatchObject({
      nextGateEligible: false,
      rung: 'shared_fleet_admin_candidate',
      signatureGatesTerminal: false,
      standingApprovalAllowed: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.autonomy.standing_approval_not_granted',
        'blocker.public.artanis.autonomy.signature.fleet-liveness.not_terminal',
        'blocker.public.artanis.autonomy.signature.diagnosis-grounding.not_terminal',
        'blocker.public.artanis.autonomy.signature.issue-close-safe.not_terminal',
        'blocker.public.artanis.autonomy.signature.command-source-verified.not_terminal',
        'blocker.public.artanis.autonomy.signature.merge-deploy.not_terminal',
        'blocker.public.artanis.autonomy.clean_unattended_ticks_not_retained',
      ]),
    )
    expect(projection.missingSignatureGateRefs).toEqual(
      expect.arrayContaining([
        'gate.public.artanis.autonomy.signature.fleet-liveness.PROVEN_ALIVE',
        'gate.public.artanis.autonomy.signature.diagnosis-grounding.GROUNDED',
        'gate.public.artanis.autonomy.signature.issue-close-safe.SAFE_TO_CLOSE',
        'gate.public.artanis.autonomy.signature.command-source-verified.SAFE_TO_PROPOSE',
        'gate.public.artanis.autonomy.signature.merge-deploy.LIVE',
      ]),
    )
  })

  test('makes shared-fleet the next eligible gate only after signatures and clean ticks', () => {
    const projection = evaluateArtanisAutonomyLadder(
      new ArtanisAutonomyLadderEvaluationInput({
        actionRef: 'action.public.artanis.shared_fleet_dispatch',
        authorityScope: ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
        cleanTickTrackRecord: retainedArtanisAutonomyCleanTickTrackRecord(),
        riskyActionKind: 'pylon_job_dispatch',
        signatureGates: terminalArtanisAutonomySignatureGateEvidence(),
        treasuryEnvelope: notApplicableArtanisAutonomyTreasuryEnvelope(),
      }),
    )

    expect(projection).toMatchObject({
      cleanTickTrackRecordRetained: true,
      nextGateEligible: true,
      rung: 'shared_fleet_admin_candidate',
      signatureGatesTerminal: true,
      standingApprovalAllowed: false,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.public.artanis.autonomy.standing_approval_not_granted',
    ])
    expect(projection.evidenceRefs).toEqual(
      expect.arrayContaining([
        'evidence.khala_coding.authority_scope.shared_fleet',
        ARTANIS_AUTONOMY_CLEAN_TICK_TRACK_RECORD_REF,
      ]),
    )
  })

  test('keeps treasury spend blocked without an owner cap envelope', () => {
    const projection = evaluateArtanisAutonomyLadder(
      new ArtanisAutonomyLadderEvaluationInput({
        actionRef: 'action.public.artanis.wallet_spend',
        authorityScope: ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
        cleanTickTrackRecord: retainedArtanisAutonomyCleanTickTrackRecord(),
        riskyActionKind: 'wallet_spend',
        signatureGates: terminalArtanisAutonomySignatureGateEvidence(),
        treasuryEnvelope: defaultArtanisAutonomyTreasuryEnvelope(),
      }),
    )

    expect(projection).toMatchObject({
      nextGateEligible: false,
      rung: 'treasury_enveloped_spend_candidate',
      standingApprovalAllowed: false,
      treasuryEnvelopeBounded: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.autonomy.standing_approval_not_granted',
        'blocker.public.artanis.autonomy.treasury_envelope_missing',
      ]),
    )
  })

  test('treats active owner caps as next-gate evidence without standing approving spend', () => {
    const projection = evaluateArtanisAutonomyLadder(
      new ArtanisAutonomyLadderEvaluationInput({
        actionRef: 'action.public.artanis.wallet_spend',
        authorityScope: ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
        cleanTickTrackRecord: retainedArtanisAutonomyCleanTickTrackRecord(),
        riskyActionKind: 'wallet_spend',
        signatureGates: terminalArtanisAutonomySignatureGateEvidence(),
        treasuryEnvelope: ownerCapArtanisAutonomyTreasuryEnvelope({
          perDayCapSat: 10_000,
          perPayoutCapSat: 1_000,
        }),
      }),
    )

    expect(projection).toMatchObject({
      nextGateEligible: true,
      rung: 'treasury_enveloped_spend_candidate',
      standingApprovalAllowed: false,
      treasuryEnvelopeBounded: true,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.public.artanis.autonomy.standing_approval_not_granted',
    ])
    expect(projection.evidenceRefs).toContain(
      ARTANIS_AUTONOMY_TREASURY_ENVELOPE_REF,
    )
  })

  test('forbids unbounded autonomy even if other evidence is clean', () => {
    const projection = evaluateArtanisAutonomyLadder(
      new ArtanisAutonomyLadderEvaluationInput({
        actionRef: 'action.public.artanis.unbounded_autonomy',
        authorityScope: ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
        cleanTickTrackRecord: retainedArtanisAutonomyCleanTickTrackRecord(),
        riskyActionKind: 'unbounded_autonomy',
        signatureGates: terminalArtanisAutonomySignatureGateEvidence(),
        treasuryEnvelope: new ArtanisAutonomyTreasuryEnvelope({
          blockerRefs: [],
          evidenceRefs: [],
          perDayCapSat: null,
          perPayoutCapSat: null,
          state: 'unbounded_requested',
        }),
      }),
    )

    expect(projection).toMatchObject({
      nextGateEligible: false,
      rung: 'unbounded_autonomy_forbidden',
      standingApprovalAllowed: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.artanis.autonomy.standing_approval_not_granted',
        'blocker.public.artanis.autonomy.unbounded_autonomy_forbidden',
      ]),
    )
  })

  test('requires retained clean ticks even when signatures are terminal', () => {
    const projection = evaluateArtanisAutonomyLadder(
      new ArtanisAutonomyLadderEvaluationInput({
        actionRef: 'action.public.artanis.shared_fleet_dispatch',
        authorityScope: ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
        cleanTickTrackRecord: defaultArtanisAutonomyCleanTickTrackRecord(),
        riskyActionKind: 'pylon_job_dispatch',
        signatureGates: terminalArtanisAutonomySignatureGateEvidence(),
        treasuryEnvelope: notApplicableArtanisAutonomyTreasuryEnvelope(),
      }),
    )

    expect(projection).toMatchObject({
      cleanTickTrackRecordRetained: false,
      nextGateEligible: false,
      signatureGatesTerminal: true,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.artanis.autonomy.clean_unattended_ticks_not_retained',
    )
  })
})
