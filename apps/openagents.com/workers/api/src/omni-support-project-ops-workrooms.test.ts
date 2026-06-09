import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_PROJECT_OPS_TEMPLATE_FIXTURE,
  OMNI_PROJECT_OPS_WORKROOM_FIXTURE,
  OMNI_SUPPORT_OPS_TEMPLATE_FIXTURE,
  OMNI_SUPPORT_OPS_WORKROOM_FIXTURE,
  OMNI_SUPPORT_PROJECT_OPS_CONTRACT_ONLY_AUTHORITY,
  OmniSupportProjectOpsTemplateProjection,
  OmniSupportProjectOpsUnsafe,
  OmniSupportProjectOpsWorkroomProjection,
  OmniSupportProjectOpsWorkroomRecord,
  omniSupportProjectOpsAuthorityIsContractOnly,
  omniSupportProjectOpsProjectionHasPrivateMaterial,
  projectOmniSupportProjectOpsTemplate,
  projectOmniSupportProjectOpsWorkroom,
} from './omni-support-project-ops-workrooms'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T06:40:00.000Z'

const supportRecord = (
  overrides: Partial<OmniSupportProjectOpsWorkroomRecord> = {},
): OmniSupportProjectOpsWorkroomRecord =>
  S.decodeUnknownSync(OmniSupportProjectOpsWorkroomRecord)({
    ...OMNI_SUPPORT_OPS_WORKROOM_FIXTURE,
    ...overrides,
  })

describe('Omni support/project ops workroom contract', () => {
  test('projects support and project-ops templates with friendly time labels', () => {
    const supportProjection = projectOmniSupportProjectOpsTemplate(
      OMNI_SUPPORT_OPS_TEMPLATE_FIXTURE,
      'operator',
      nowIso,
    )
    const projectProjection = projectOmniSupportProjectOpsTemplate(
      OMNI_PROJECT_OPS_TEMPLATE_FIXTURE,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniSupportProjectOpsTemplateProjection)(
      supportProjection,
    )).toEqual(supportProjection)
    expect(S.decodeUnknownSync(OmniSupportProjectOpsTemplateProjection)(
      projectProjection,
    )).toEqual(projectProjection)
    expect(supportProjection.kind).toBe('support')
    expect(projectProjection.kind).toBe('project_ops')
    expect(supportProjection.createdAtDisplay).toBe('30 minutes ago')
    expect(supportProjection.updatedAtDisplay).toBe('25 minutes ago')
    expect(JSON.stringify(supportProjection)).not.toContain('2026-06-07T')
  })

  test('projects closed support and project ops workrooms without send or mutation authority', () => {
    const support = supportRecord()
    const project = S.decodeUnknownSync(OmniSupportProjectOpsWorkroomRecord)(
      OMNI_PROJECT_OPS_WORKROOM_FIXTURE,
    )
    const supportProjection = projectOmniSupportProjectOpsWorkroom(
      support,
      'operator',
      nowIso,
    )
    const projectProjection = projectOmniSupportProjectOpsWorkroom(
      project,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniSupportProjectOpsWorkroomProjection)(
      supportProjection,
    )).toEqual(supportProjection)
    expect(omniSupportProjectOpsAuthorityIsContractOnly(support.authority))
      .toBe(true)
    expect(supportProjection.supportResponseSendAllowed).toBe(false)
    expect(supportProjection.projectManagementMutationAllowed).toBe(false)
    expect(supportProjection.customerRecordMutationAllowed).toBe(false)
    expect(supportProjection.externalEscalationAllowed).toBe(false)
    expect(supportProjection.acceptedOutcomeMutationAllowed).toBe(false)
    expect(supportProjection.issueTimelineReady).toBe(true)
    expect(supportProjection.proposedResponseReady).toBe(true)
    expect(supportProjection.escalationRecorded).toBe(true)
    expect(supportProjection.projectTaskUpdated).toBe(true)
    expect(supportProjection.decisionRecorded).toBe(true)
    expect(supportProjection.riskRecorded).toBe(true)
    expect(supportProjection.statusReportReady).toBe(true)
    expect(supportProjection.receiptRecorded).toBe(true)
    expect(supportProjection.closeoutReady).toBe(true)
    expect(projectProjection.kind).toBe('project_ops')
    expect(projectProjection.statusReportReady).toBe(true)
    expect(omniSupportProjectOpsProjectionHasPrivateMaterial(supportProjection))
      .toBe(false)
  })

  test('hides customer, ticket, timeline, response, task, decision, risk, status, receipt, and workroom refs from public projection', () => {
    const projection = projectOmniSupportProjectOpsWorkroom(
      supportRecord(),
      'public',
      nowIso,
    )

    expect(projection.closeoutRefs).toEqual([])
    expect(projection.customerRefs).toEqual([])
    expect(projection.decisionRefs).toEqual([])
    expect(projection.escalationRefs).toEqual([])
    expect(projection.issueTimelineRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.projectTaskRefs).toEqual([])
    expect(projection.proposedResponseRefs).toEqual([])
    expect(projection.receiptRefs).toEqual([])
    expect(projection.riskRefs).toEqual([])
    expect(projection.sourceRefs).toEqual([])
    expect(projection.statusReportRefs).toEqual([])
    expect(projection.ticketRefs).toEqual([])
    expect(projection.workroomRef).toBe('redacted')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(omniSupportProjectOpsProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps timeline, response, escalation, task, decision, risk, status, receipt, and closeout states separate', () => {
    const timelineOnly = supportRecord({
      closeoutRefs: [],
      decisionRefs: [],
      escalationRefs: [],
      projectTaskRefs: [],
      proposedResponseRefs: [],
      receiptRefs: [],
      riskRefs: [],
      statusReportRefs: [],
      state: 'issue_timeline_reconstructed',
    })
    const decisionOnly = supportRecord({
      closeoutRefs: [],
      receiptRefs: [],
      riskRefs: [],
      statusReportRefs: [],
      state: 'decision_recorded',
    })
    const receiptOnly = supportRecord({
      closeoutRefs: [],
      state: 'receipt_recorded',
    })

    expect(projectOmniSupportProjectOpsWorkroom(
      timelineOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      decisionRecorded: false,
      issueTimelineReady: true,
      proposedResponseReady: false,
      receiptRecorded: false,
      statusReportReady: false,
    })
    expect(projectOmniSupportProjectOpsWorkroom(
      decisionOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      closeoutReady: false,
      decisionRecorded: true,
      receiptRecorded: false,
      riskRecorded: false,
      statusReportReady: false,
    })
    expect(projectOmniSupportProjectOpsWorkroom(
      receiptOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      closeoutReady: false,
      receiptRecorded: true,
    })
  })

  test('rejects mutable authority and blocked records without blockers', () => {
    expect(() =>
      projectOmniSupportProjectOpsWorkroom(
        supportRecord({
          authority: {
            ...OMNI_SUPPORT_PROJECT_OPS_CONTRACT_ONLY_AUTHORITY,
            noSupportResponseSend: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSupportProjectOpsUnsafe)
    expect(() =>
      projectOmniSupportProjectOpsWorkroom(
        supportRecord({
          blockerRefs: [],
          state: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSupportProjectOpsUnsafe)
  })

  test('rejects customer private data, raw support transcripts, private tickets, provider material, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw transcript', value: 'raw_transcript.full' },
      { label: 'support transcript', value: 'support_transcript.full' },
      { label: 'private ticket', value: 'ticket_private.customer_issue' },
      { label: 'customer email', value: 'customer_email.primary' },
    ]) {
      expect(() =>
        projectOmniSupportProjectOpsWorkroom(
          supportRecord({ evidenceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OmniSupportProjectOpsUnsafe)
    }
  })
})
