import { describe, expect, test } from 'vitest'

import {
  AUTOPILOT_ROTATION_SMOKE_VERSION,
  type AutopilotRotationSmokeInput,
  rotationContextFingerprint,
  runAutopilotRateLimitRotationSmoke,
  verifyAutopilotRotationContinuity,
} from './autopilot-rate-limit-rotation-smoke'
import {
  codingAutopilotContinuationDecisionProjectionHasPrivateMaterial,
  projectCodingAutopilotContinuationDecisionRecord,
} from './coding-autopilot-continuation-decisions'
import {
  codingAutopilotMissionProjectionHasPrivateMaterial,
  projectCodingAutopilotMissionRecord,
} from './coding-autopilot-missions'
import {
  PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
  type ProviderAccountLeaseCandidate,
} from './provider-account-lease-policy'
import { isoTimestampAfterIso } from './runtime-primitives'

const fleetAccount = (
  providerAccountRef: string,
  overrides: Partial<ProviderAccountLeaseCandidate> = {},
): ProviderAccountLeaseCandidate => ({
  providerAccountRef,
  provider: 'chatgpt_codex',
  status: 'connected',
  health: 'healthy',
  hasSecretRef: true,
  activeLeaseCount: 0,
  leaseLimit: 2,
  operatorPriority: 100,
  connectedAt: '2026-06-11T00:00:00.000Z',
  createdAt: '2026-06-11T00:00:00.000Z',
  lastSelectedAt: null,
  lastSanityCheckAt: null,
  lastSanityCheckResult: null,
  lastParallelProbeAt: null,
  recentFailureClass: null,
  cooldownUntil: null,
  lowCredit: false,
  ...overrides,
})

const startedAtIso = '2026-06-11T12:00:00.000Z'

const rotationSmokeInput = (
  overrides: Partial<AutopilotRotationSmokeInput> = {},
): AutopilotRotationSmokeInput => ({
  fleet: [
    fleetAccount('provider-account_ref_alpha', {
      lastSelectedAt: '2026-06-11T00:05:00.000Z',
    }),
    fleetAccount('provider-account_ref_bravo'),
  ],
  inducedFailure: {
    atTurnRef: 'implement',
    failureClass: 'rate_limited',
  },
  missionSlug: 'rotation_smoke_m9',
  plannedTurnRefs: ['scaffold', 'implement', 'verify'],
  startedAtIso,
  workroomRef: 'workroom.rotation_smoke_m9',
  ...overrides,
})

describe('M9 rate-limit rotation smoke', () => {
  test('induced rate limit mid-mission rotates the account lease and completes with context intact', () => {
    const receipt = runAutopilotRateLimitRotationSmoke(rotationSmokeInput())

    expect(receipt.smokeVersion).toBe(AUTOPILOT_ROTATION_SMOKE_VERSION)
    expect(receipt.outcome).toBe('completed')
    expect(receipt.blockedReason).toBeNull()
    expect(receipt.generatedAt).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/,
    )

    expect(receipt.mission).toMatchObject({
      id: 'mission_rotation_smoke_m9',
      missionRef: 'mission.rotation_smoke_m9',
      status: 'delivered',
      accountLeaseRefs: [
        'account_lease.rotation_smoke_m9.lease_1',
        'account_lease.rotation_smoke_m9.lease_2',
      ],
      artifactRefs: [
        'artifact.scaffold.rotation_smoke_m9',
        'artifact.implement.rotation_smoke_m9',
        'artifact.verify.rotation_smoke_m9',
      ],
      blockerRefs: [],
    })

    expect(receipt.rotation).toMatchObject({
      outcome: 'retrying',
      failureClass: 'rate_limited',
      accountStateAction: 'timed_cooldown',
      inducedAtTurnRef: 'implement',
      previousLeaseRef: 'account_lease.rotation_smoke_m9.lease_1',
      nextLeaseRef: 'account_lease.rotation_smoke_m9.lease_2',
      policyVersion: 'provider-account-lease-policy:v2',
      customerSafeStatus:
        'Work is retrying with another account after a temporary provider limit.',
    })
    expect(receipt.rotation?.previousAccountSlug).not.toBe(
      receipt.rotation?.nextAccountSlug,
    )

    const faultAtIso = isoTimestampAfterIso(startedAtIso, 60_000)

    expect(receipt.rotation?.cooldownUntil).toBe(
      isoTimestampAfterIso(faultAtIso, 60 * 60 * 1_000),
    )

    expect(receipt.continuationDecision).toMatchObject({
      selectedContinuationAction: 'retry_account',
      queuedActionKind: 'retry_account',
      missionRef: 'mission.rotation_smoke_m9',
      workRef: 'mission.rotation_smoke_m9',
      evidenceRefs: ['evidence.account_rate_limit'],
      riskRefs: ['risk.account_rotation_needed'],
      receiptRefs: ['receipt.account_failover.rotation_smoke_m9'],
      evidenceOnly: true,
      directEffectPermitted: false,
      actionSubmissionRequiredForDirectEffects: true,
    })

    expect(receipt.turns).toHaveLength(3)
    const [scaffoldTurn, implementTurn, verifyTurn] = receipt.turns

    expect(scaffoldTurn).toMatchObject({
      leaseRef: 'account_lease.rotation_smoke_m9.lease_1',
      resumedAfterRotation: false,
      buildsOnArtifactRef: null,
      contextFingerprintBefore: rotationContextFingerprint(
        'genesis',
        'mission.rotation_smoke_m9',
      ),
    })
    expect(implementTurn).toMatchObject({
      leaseRef: 'account_lease.rotation_smoke_m9.lease_2',
      resumedAfterRotation: true,
      buildsOnArtifactRef: 'artifact.scaffold.rotation_smoke_m9',
      contextFingerprintBefore: scaffoldTurn?.contextFingerprintAfter,
    })
    expect(verifyTurn).toMatchObject({
      leaseRef: 'account_lease.rotation_smoke_m9.lease_2',
      resumedAfterRotation: false,
      buildsOnArtifactRef: 'artifact.implement.rotation_smoke_m9',
      contextFingerprintBefore: implementTurn?.contextFingerprintAfter,
    })
    expect(implementTurn?.accountSlug).not.toBe(scaffoldTurn?.accountSlug)

    expect(verifyAutopilotRotationContinuity(receipt)).toEqual([])
  })

  test('the smoke is deterministic for a fixed input', () => {
    const first = runAutopilotRateLimitRotationSmoke(rotationSmokeInput())
    const second = runAutopilotRateLimitRotationSmoke(rotationSmokeInput())

    expect(JSON.parse(JSON.stringify(second))).toEqual(
      JSON.parse(JSON.stringify(first)),
    )
  })

  test('failure arm: no eligible second account yields a typed blocked state, never a silent stall', () => {
    const receipt = runAutopilotRateLimitRotationSmoke(
      rotationSmokeInput({
        fleet: [fleetAccount('provider-account_ref_alpha')],
      }),
    )

    expect(receipt.outcome).toBe('blocked')
    expect(receipt.blockedReason).toBe(
      'No eligible second account was available after the induced provider limit.',
    )
    expect(receipt.mission).toMatchObject({
      status: 'blocked',
      blockerRefs: ['blocker.account_fleet_exhausted.rotation_smoke_m9'],
      accountLeaseRefs: ['account_lease.rotation_smoke_m9.lease_1'],
      artifactRefs: ['artifact.scaffold.rotation_smoke_m9'],
    })
    expect(receipt.rotation).toMatchObject({
      outcome: 'blocked',
      nextLeaseRef: null,
      nextAccountSlug: null,
      customerSafeStatus:
        'Work is blocked until another eligible account is available.',
    })
    expect(receipt.continuationDecision).toMatchObject({
      selectedContinuationAction: 'escalate',
      queuedActionKind: 'request_customer_input',
      guardrailState: 'blocked',
      riskRefs: [
        'risk.account_fleet_exhausted',
        'risk.account_rotation_needed',
      ],
    })

    expect(verifyAutopilotRotationContinuity(receipt)).toEqual([])
  })

  test('mission and continuation projections stay audience-safe and redaction-clean', () => {
    const receipt = runAutopilotRateLimitRotationSmoke(rotationSmokeInput())
    const nowIso = isoTimestampAfterIso(startedAtIso, 10 * 60_000)

    const operatorMission = projectCodingAutopilotMissionRecord(
      receipt.mission,
      'operator',
      nowIso,
    )

    expect(operatorMission.accountLeaseRefs).toEqual([
      'account_lease.rotation_smoke_m9.lease_1',
      'account_lease.rotation_smoke_m9.lease_2',
    ])
    expect(
      codingAutopilotMissionProjectionHasPrivateMaterial(operatorMission),
    ).toBe(false)

    const customerMission = projectCodingAutopilotMissionRecord(
      receipt.mission,
      'customer',
      nowIso,
    )

    expect(customerMission.accountLeaseRefs).toEqual([])
    expect(
      codingAutopilotMissionProjectionHasPrivateMaterial(customerMission),
    ).toBe(false)

    const decision = receipt.continuationDecision

    expect(decision).not.toBeNull()

    if (decision !== null) {
      const operatorDecision = projectCodingAutopilotContinuationDecisionRecord(
        decision,
        'operator',
        nowIso,
      )

      expect(operatorDecision.riskRefs).toEqual([
        'risk.account_rotation_needed',
      ])
      expect(
        codingAutopilotContinuationDecisionProjectionHasPrivateMaterial(
          operatorDecision,
        ),
      ).toBe(false)

      const customerDecision = projectCodingAutopilotContinuationDecisionRecord(
        decision,
        'customer',
        nowIso,
      )

      expect(
        codingAutopilotContinuationDecisionProjectionHasPrivateMaterial(
          customerDecision,
        ),
      ).toBe(false)
    }

    const receiptText = JSON.stringify(receipt).replaceAll(
      PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
      'lease-policy-version',
    )

    expect(receiptText).not.toMatch(/codex-auth|secret|bearer|oauth/i)
    expect(receiptText).not.toMatch(
      /provider[_-]?(account|grant|payload|token)/i,
    )
    expect(receiptText).not.toMatch(/\btoken\b|wallet|mnemonic|lnbc/i)
  })
})
