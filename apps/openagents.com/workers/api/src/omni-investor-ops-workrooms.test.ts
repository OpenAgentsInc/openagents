import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_INVESTOR_OPS_CONTRACT_ONLY_AUTHORITY,
  OMNI_INVESTOR_OPS_TEMPLATE_FIXTURE,
  OMNI_INVESTOR_OPS_WORKROOM_FIXTURE,
  OmniInvestorOpsTemplateProjection,
  OmniInvestorOpsUnsafe,
  OmniInvestorOpsWorkroomProjection,
  OmniInvestorOpsWorkroomRecord,
  omniInvestorOpsAuthorityIsContractOnly,
  omniInvestorOpsProjectionHasPrivateMaterial,
  projectOmniInvestorOpsTemplate,
  projectOmniInvestorOpsWorkroom,
} from './omni-investor-ops-workrooms'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T06:10:00.000Z'

const workroomRecord = (
  overrides: Partial<OmniInvestorOpsWorkroomRecord> = {},
): OmniInvestorOpsWorkroomRecord =>
  S.decodeUnknownSync(OmniInvestorOpsWorkroomRecord)({
    ...OMNI_INVESTOR_OPS_WORKROOM_FIXTURE,
    ...overrides,
  })

describe('Omni investor ops workroom contract', () => {
  test('projects investor ops template with friendly time labels', () => {
    const projection = projectOmniInvestorOpsTemplate(
      OMNI_INVESTOR_OPS_TEMPLATE_FIXTURE,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniInvestorOpsTemplateProjection)(projection))
      .toEqual(projection)
    expect(projection.createdAtDisplay).toBe('40 minutes ago')
    expect(projection.updatedAtDisplay).toBe('35 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(omniInvestorOpsProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('projects closed investor ops workroom without outreach or mutation authority', () => {
    const record = workroomRecord()
    const projection = projectOmniInvestorOpsWorkroom(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniInvestorOpsWorkroomProjection)(projection))
      .toEqual(projection)
    expect(omniInvestorOpsAuthorityIsContractOnly(record.authority)).toBe(true)
    expect(projection.outreachSendAllowed).toBe(false)
    expect(projection.creativePublishAllowed).toBe(false)
    expect(projection.dataRoomMutationAllowed).toBe(false)
    expect(projection.investorMutationAllowed).toBe(false)
    expect(projection.acceptedOutcomeMutationAllowed).toBe(false)
    expect(projection.prepPacketReady).toBe(true)
    expect(projection.dataRoomTaskReady).toBe(true)
    expect(projection.creativeWorkOrderReady).toBe(true)
    expect(projection.followUpQueued).toBe(true)
    expect(projection.decisionReceiptRecorded).toBe(true)
    expect(projection.acceptedOutcomeRecorded).toBe(true)
    expect(projection.closeoutReady).toBe(true)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('10 minutes ago')
    expect(omniInvestorOpsProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('hides investor, contact, data-room, deck/video, follow-up, decision, and workroom refs from public projection', () => {
    const projection = projectOmniInvestorOpsWorkroom(
      workroomRecord(),
      'public',
      nowIso,
    )

    expect(projection.acceptanceRefs).toEqual([])
    expect(projection.closeoutRefs).toEqual([])
    expect(projection.contactRefs).toEqual([])
    expect(projection.dataRoomTaskRefs).toEqual([])
    expect(projection.decisionReceiptRefs).toEqual([])
    expect(projection.deckWorkOrderRefs).toEqual([])
    expect(projection.followUpRefs).toEqual([])
    expect(projection.investorRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.prepPacketRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.videoWorkOrderRefs).toEqual([])
    expect(projection.workroomRef).toBe('redacted')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(omniInvestorOpsProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('keeps prep, data-room, creative, follow-up, decision, and accepted outcome states separate', () => {
    const prepOnly = workroomRecord({
      acceptanceRefs: [],
      closeoutRefs: [],
      dataRoomTaskRefs: [],
      decisionReceiptRefs: [],
      deckWorkOrderRefs: [],
      followUpRefs: [],
      videoWorkOrderRefs: [],
      state: 'prep_packet_ready',
    })
    const creativeOnly = workroomRecord({
      acceptanceRefs: [],
      closeoutRefs: [],
      decisionReceiptRefs: [],
      followUpRefs: [],
      state: 'creative_work_order_ready',
    })
    const decisionOnly = workroomRecord({
      acceptanceRefs: [],
      closeoutRefs: [],
      state: 'decision_receipt_recorded',
    })

    expect(projectOmniInvestorOpsWorkroom(prepOnly, 'operator', nowIso))
      .toMatchObject({
        acceptedOutcomeRecorded: false,
        creativeWorkOrderReady: false,
        dataRoomTaskReady: false,
        decisionReceiptRecorded: false,
        followUpQueued: false,
        prepPacketReady: true,
      })
    expect(projectOmniInvestorOpsWorkroom(creativeOnly, 'operator', nowIso))
      .toMatchObject({
        acceptedOutcomeRecorded: false,
        creativeWorkOrderReady: true,
        decisionReceiptRecorded: false,
        followUpQueued: false,
      })
    expect(projectOmniInvestorOpsWorkroom(decisionOnly, 'operator', nowIso))
      .toMatchObject({
        acceptedOutcomeRecorded: false,
        closeoutReady: false,
        decisionReceiptRecorded: true,
      })
  })

  test('rejects mutable authority and blocked records without blockers', () => {
    expect(() =>
      projectOmniInvestorOpsWorkroom(
        workroomRecord({
          authority: {
            ...OMNI_INVESTOR_OPS_CONTRACT_ONLY_AUTHORITY,
            noOutreachSend: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniInvestorOpsUnsafe)
    expect(() =>
      projectOmniInvestorOpsWorkroom(
        workroomRecord({
          blockerRefs: [],
          state: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniInvestorOpsUnsafe)
  })

  test('rejects raw investor, contact, data-room, deck/video, provider, private repo, and timestamp material', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'investor email', value: 'investor_email.primary' },
      { label: 'raw data room', value: 'raw_data_room.full' },
      { label: 'raw deck', value: 'raw_deck.private' },
      { label: 'raw video', value: 'raw_video.private' },
      { label: 'contact phone', value: 'contact_phone.mobile' },
    ]) {
      expect(() =>
        projectOmniInvestorOpsWorkroom(
          workroomRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OmniInvestorOpsUnsafe)
    }
  })
})
