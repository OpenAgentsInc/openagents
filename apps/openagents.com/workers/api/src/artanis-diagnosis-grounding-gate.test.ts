import { describe, expect, test } from 'vitest'

import {
  artanisDiagnosisGroundingPolicy,
  evaluateArtanisDiagnosisGroundingGate,
  type ArtanisDiagnosisEvidence,
} from './artanis-diagnosis-grounding-gate'

const dispatchLog = {
  lastEntriesExamined: 20,
  outcomes: ['accepted', 'rate_limited'],
  ref: 'supervisor-dispatch-log.20260628.last20',
}

const providerHeaders = {
  ref: 'provider-rate-limit-headers.20260628.openai.429',
  retryAfterSeconds: 60,
}

const gate = (evidence: ArtanisDiagnosisEvidence) =>
  evaluateArtanisDiagnosisGroundingGate({
    claimKind: 'rate_limited',
    evidence,
  })

describe('Artanis diagnosis grounding gate (#6647)', () => {
  test('advances only through the required evidence states before remediation unlocks', () => {
    expect(gate({}).state).toBe('UNGROUNDED')
    expect(gate({}).canProposeRemediation).toBe(false)

    expect(gate({ quotaLedgerReadRef: 'quota-ledger-read.20260628' }).state).toBe(
      'LEDGER_READ',
    )

    expect(
      gate({
        quotaLedgerReadRef: 'quota-ledger-read.20260628',
        supervisorDispatchLog: dispatchLog,
      }).state,
    ).toBe('DISPATCH_LOG_EXAMINED')

    const grounded = gate({
      providerRateLimitHeaders: providerHeaders,
      quotaLedgerReadRef: 'quota-ledger-read.20260628',
      supervisorDispatchLog: dispatchLog,
    })
    expect(grounded.state).toBe('GROUNDED')
    expect(grounded.canProposeRemediation).toBe(true)
    expect(grounded.missingRefs).toEqual([])
  })

  test('rejects shallow dispatch logs and rate-limit claims without real reset evidence', () => {
    const shallowDispatch = gate({
      providerRateLimitHeaders: providerHeaders,
      quotaLedgerReadRef: 'quota-ledger-read.20260628',
      supervisorDispatchLog: {
        ...dispatchLog,
        lastEntriesExamined: 19,
      },
    })
    expect(shallowDispatch.state).toBe('LEDGER_READ')
    expect(shallowDispatch.missingRefs).toContain('supervisor-dispatch-log')
    expect(shallowDispatch.canProposeRemediation).toBe(false)

    const unverifiedProvider = gate({
      providerRateLimitHeaders: {
        ref: 'provider-rate-limit-headers.20260628.openai.429',
      },
      quotaLedgerReadRef: 'quota-ledger-read.20260628',
      supervisorDispatchLog: dispatchLog,
    })
    expect(unverifiedProvider.state).toBe('PROVIDER_VERIFIED')
    expect(unverifiedProvider.blockerRefs).toContain(
      'blocker.artanis.diagnosis.provider_headers_do_not_match_claim',
    )
    expect(unverifiedProvider.canProposeRemediation).toBe(false)
  })

  test('publishes the autonomous-ops-v1 signature policy refs for the composer', () => {
    const policy = artanisDiagnosisGroundingPolicy()
    expect(policy.signature).toBe(
      'autonomous-ops-v1.signature-2.diagnosis-grounding',
    )
    expect(policy.requiredRefs).toEqual([
      'quota-ledger-read',
      'supervisor-dispatch-log',
      'provider-rate-limit-headers',
    ])
    expect(policy.stateOrder).toEqual([
      'UNGROUNDED',
      'LEDGER_READ',
      'DISPATCH_LOG_EXAMINED',
      'PROVIDER_VERIFIED',
      'GROUNDED',
    ])
  })
})
