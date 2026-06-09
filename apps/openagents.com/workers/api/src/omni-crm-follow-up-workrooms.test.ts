import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_CRM_FOLLOW_UP_CONTRACT_ONLY_AUTHORITY,
  OMNI_CRM_FOLLOW_UP_TEMPLATE_FIXTURE,
  OMNI_CRM_FOLLOW_UP_WORKROOM_FIXTURE,
  OmniCrmFollowUpTemplateProjection,
  OmniCrmFollowUpWorkroomProjection,
  OmniCrmFollowUpWorkroomRecord,
  OmniCrmFollowUpUnsafe,
  omniCrmFollowUpAuthorityIsContractOnly,
  omniCrmFollowUpProjectionHasPrivateMaterial,
  projectOmniCrmFollowUpTemplate,
  projectOmniCrmFollowUpWorkroom,
} from './omni-crm-follow-up-workrooms'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T05:30:00.000Z'

const workroomRecord = (
  overrides: Partial<OmniCrmFollowUpWorkroomRecord> = {},
): OmniCrmFollowUpWorkroomRecord =>
  S.decodeUnknownSync(OmniCrmFollowUpWorkroomRecord)({
    ...OMNI_CRM_FOLLOW_UP_WORKROOM_FIXTURE,
    ...overrides,
  })

describe('Omni CRM follow-up workroom contract', () => {
  test('projects CRM follow-up template with friendly time labels', () => {
    const projection = projectOmniCrmFollowUpTemplate(
      OMNI_CRM_FOLLOW_UP_TEMPLATE_FIXTURE,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniCrmFollowUpTemplateProjection)(projection))
      .toEqual(projection)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('25 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(omniCrmFollowUpProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('projects closed CRM follow-up without email-send or CRM mutation authority', () => {
    const record = workroomRecord()
    const projection = projectOmniCrmFollowUpWorkroom(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniCrmFollowUpWorkroomProjection)(projection))
      .toEqual(projection)
    expect(omniCrmFollowUpAuthorityIsContractOnly(record.authority)).toBe(true)
    expect(projection.emailSendAllowed).toBe(false)
    expect(projection.crmMutationAllowed).toBe(false)
    expect(projection.relationshipMemoryMutationAllowed).toBe(false)
    expect(projection.approvalRecorded).toBe(true)
    expect(projection.draftPrepared).toBe(true)
    expect(projection.emailReceiptRecorded).toBe(true)
    expect(projection.relationshipMemoryRecorded).toBe(true)
    expect(projection.closeoutReady).toBe(true)
    expect(projection.createdAtDisplay).toBe('20 minutes ago')
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(omniCrmFollowUpProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('hides CRM contact, company, source, draft, receipt, memory, and workroom refs from public projection', () => {
    const projection = projectOmniCrmFollowUpWorkroom(
      workroomRecord(),
      'public',
      nowIso,
    )

    expect(projection.approvalRefs).toEqual([])
    expect(projection.closeoutRefs).toEqual([])
    expect(projection.companyRefs).toEqual([])
    expect(projection.contactRefs).toEqual([])
    expect(projection.draftMessageRefs).toEqual([])
    expect(projection.emailReceiptRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.prepPacketRefs).toEqual([])
    expect(projection.relationshipMemoryRefs).toEqual([])
    expect(projection.sendRequestRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.workroomRef).toBe('redacted')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(omniCrmFollowUpProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('keeps draft, approval, send, receipt, closeout, and relationship memory states separate', () => {
    const draftOnly = workroomRecord({
      approvalRefs: [],
      closeoutRefs: [],
      emailReceiptRefs: [],
      relationshipMemoryRefs: [],
      sendRequestRefs: [],
      state: 'draft_prepared',
    })
    const approvalOnly = workroomRecord({
      closeoutRefs: [],
      emailReceiptRefs: [],
      relationshipMemoryRefs: [],
      sendRequestRefs: [],
      state: 'approval_recorded',
    })
    const receiptOnly = workroomRecord({
      closeoutRefs: [],
      relationshipMemoryRefs: [],
      state: 'email_receipt_recorded',
    })

    expect(projectOmniCrmFollowUpWorkroom(draftOnly, 'operator', nowIso))
      .toMatchObject({
        approvalRecorded: false,
        draftPrepared: true,
        emailReceiptRecorded: false,
        relationshipMemoryRecorded: false,
        sendPrepared: false,
      })
    expect(projectOmniCrmFollowUpWorkroom(approvalOnly, 'operator', nowIso))
      .toMatchObject({
        approvalRecorded: true,
        closeoutReady: false,
        emailReceiptRecorded: false,
        sendPrepared: false,
      })
    expect(projectOmniCrmFollowUpWorkroom(receiptOnly, 'operator', nowIso))
      .toMatchObject({
        closeoutReady: false,
        emailReceiptRecorded: true,
        relationshipMemoryRecorded: false,
      })
  })

  test('rejects mutable authority and blocked records without blockers', () => {
    expect(() =>
      projectOmniCrmFollowUpWorkroom(
        workroomRecord({
          authority: {
            ...OMNI_CRM_FOLLOW_UP_CONTRACT_ONLY_AUTHORITY,
            noEmailSend: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniCrmFollowUpUnsafe)
    expect(() =>
      projectOmniCrmFollowUpWorkroom(
        workroomRecord({
          blockerRefs: [],
          state: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniCrmFollowUpUnsafe)
  })

  test('rejects raw email, private contact, customer, provider, source, secret, and timestamp material', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw email body', value: 'email_body.follow_up_full' },
      { label: 'contact email', value: 'contact_email.primary' },
      { label: 'raw source payload', value: 'raw_source.crm_dump' },
      { label: 'private contact', value: 'private_contact.mobile' },
    ]) {
      expect(() =>
        projectOmniCrmFollowUpWorkroom(
          workroomRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OmniCrmFollowUpUnsafe)
    }
  })
})
