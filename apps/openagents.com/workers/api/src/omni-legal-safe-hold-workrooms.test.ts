import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_LEGAL_SAFE_HOLD_CONTRACT_ONLY_AUTHORITY,
  OMNI_LEGAL_SAFE_HOLD_TEMPLATE_FIXTURE,
  OMNI_LEGAL_SAFE_HOLD_WORKROOM_FIXTURE,
  OmniLegalSafeHoldTemplateProjection,
  OmniLegalSafeHoldUnsafe,
  OmniLegalSafeHoldWorkroomProjection,
  OmniLegalSafeHoldWorkroomRecord,
  omniLegalSafeHoldAuthorityIsContractOnly,
  omniLegalSafeHoldProjectionHasPrivateMaterial,
  projectOmniLegalSafeHoldTemplate,
  projectOmniLegalSafeHoldWorkroom,
} from './omni-legal-safe-hold-workrooms'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T07:40:00.000Z'

const legalRecord = (
  overrides: Partial<OmniLegalSafeHoldWorkroomRecord> = {},
): OmniLegalSafeHoldWorkroomRecord =>
  S.decodeUnknownSync(OmniLegalSafeHoldWorkroomRecord)({
    ...OMNI_LEGAL_SAFE_HOLD_WORKROOM_FIXTURE,
    ...overrides,
  })

describe('Omni legal safe-hold workroom contract', () => {
  test('projects legal safe-hold template with friendly time labels', () => {
    const projection = projectOmniLegalSafeHoldTemplate(
      OMNI_LEGAL_SAFE_HOLD_TEMPLATE_FIXTURE,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniLegalSafeHoldTemplateProjection)(projection))
      .toEqual(projection)
    expect(projection.createdAtDisplay).toBe('40 minutes ago')
    expect(projection.updatedAtDisplay).toBe('35 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(omniLegalSafeHoldProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('projects closed legal safe-hold workroom without execution or legal authority', () => {
    const record = legalRecord()
    const projection = projectOmniLegalSafeHoldWorkroom(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniLegalSafeHoldWorkroomProjection)(projection))
      .toEqual(projection)
    expect(omniLegalSafeHoldAuthorityIsContractOnly(record.authority))
      .toBe(true)
    expect(projection.automaticExecutionAllowed).toBe(false)
    expect(projection.externalSendAllowed).toBe(false)
    expect(projection.filingAllowed).toBe(false)
    expect(projection.legalAdviceClaimsAllowed).toBe(false)
    expect(projection.paymentSettlementAllowed).toBe(false)
    expect(projection.publicProjectionUpgradeAllowed).toBe(false)
    expect(projection.holdRecorded).toBe(true)
    expect(projection.scopingReady).toBe(true)
    expect(projection.sourceBackedSummaryReady).toBe(true)
    expect(projection.legalReviewRequested).toBe(true)
    expect(projection.legalReviewRecorded).toBe(true)
    expect(projection.releaseRecorded).toBe(true)
    expect(projection.declineRecorded).toBe(false)
    expect(projection.closeoutReady).toBe(true)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('10 minutes ago')
    expect(omniLegalSafeHoldProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('hides client, matter, source, review, hold, release, decline, closeout, and workroom refs from public projection', () => {
    const projection = projectOmniLegalSafeHoldWorkroom(
      legalRecord(),
      'public',
      nowIso,
    )

    expect(projection.clientRefs).toEqual([])
    expect(projection.closeoutRefs).toEqual([])
    expect(projection.declineRefs).toEqual([])
    expect(projection.holdRefs).toEqual([])
    expect(projection.jurisdictionRefs).toEqual([])
    expect(projection.legalReviewRefs).toEqual([])
    expect(projection.matterRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.releaseRefs).toEqual([])
    expect(projection.scopingRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.workroomRef).toBe('redacted')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(omniLegalSafeHoldProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps hold, scoping, source summary, review, release, decline, and closeout states separate', () => {
    const holdOnly = legalRecord({
      closeoutRefs: [],
      declineRefs: [],
      legalReviewRefs: [],
      releaseRefs: [],
      scopingRefs: [],
      sourceRefs: [],
      state: 'safe_hold_recorded',
    })
    const sourceOnly = legalRecord({
      closeoutRefs: [],
      declineRefs: [],
      legalReviewRefs: [],
      releaseRefs: [],
      state: 'source_backed_summary_ready',
    })
    const reviewOnly = legalRecord({
      closeoutRefs: [],
      declineRefs: [],
      releaseRefs: [],
      state: 'legal_review_recorded',
    })
    const releaseOnly = legalRecord({
      closeoutRefs: [],
      state: 'released',
    })
    const declineOnly = legalRecord({
      closeoutRefs: [],
      declineRefs: ['decline.legal.operator_declined'],
      releaseRefs: [],
      state: 'declined',
    })

    expect(projectOmniLegalSafeHoldWorkroom(holdOnly, 'operator', nowIso))
      .toMatchObject({
        closeoutReady: false,
        holdRecorded: true,
        legalReviewRecorded: false,
        legalReviewRequested: false,
        releaseRecorded: false,
        scopingReady: false,
        sourceBackedSummaryReady: false,
      })
    expect(projectOmniLegalSafeHoldWorkroom(sourceOnly, 'operator', nowIso))
      .toMatchObject({
        holdRecorded: true,
        legalReviewRequested: false,
        scopingReady: true,
        sourceBackedSummaryReady: true,
      })
    expect(projectOmniLegalSafeHoldWorkroom(reviewOnly, 'operator', nowIso))
      .toMatchObject({
        legalReviewRecorded: true,
        legalReviewRequested: true,
        releaseRecorded: false,
      })
    expect(projectOmniLegalSafeHoldWorkroom(releaseOnly, 'operator', nowIso))
      .toMatchObject({
        closeoutReady: false,
        declineRecorded: false,
        releaseRecorded: true,
      })
    expect(projectOmniLegalSafeHoldWorkroom(declineOnly, 'operator', nowIso))
      .toMatchObject({
        closeoutReady: false,
        declineRecorded: true,
        releaseRecorded: false,
      })
  })

  test('rejects mutable authority and blocked records without blockers', () => {
    expect(() =>
      projectOmniLegalSafeHoldWorkroom(
        legalRecord({
          authority: {
            ...OMNI_LEGAL_SAFE_HOLD_CONTRACT_ONLY_AUTHORITY,
            noLegalAdviceClaims: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniLegalSafeHoldUnsafe)
    expect(() =>
      projectOmniLegalSafeHoldWorkroom(
        legalRecord({
          blockerRefs: [],
          state: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniLegalSafeHoldUnsafe)
  })

  test('rejects legal-sensitive data, client identity, matter data, raw docs, provider material, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'client email', value: 'client_email.primary' },
      { label: 'confidential matter', value: 'matter_confidential.file' },
      { label: 'privileged source', value: 'privileged.source.memo' },
      { label: 'raw document', value: 'raw_document.pleading' },
      { label: 'legal advice claim', value: 'legal_advice_claim.final' },
    ]) {
      expect(() =>
        projectOmniLegalSafeHoldWorkroom(
          legalRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OmniLegalSafeHoldUnsafe)
    }
  })
})
