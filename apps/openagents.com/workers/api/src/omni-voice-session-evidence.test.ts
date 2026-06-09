import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
  OmniVoiceCommandProposal,
  OmniVoiceSessionEvidenceUnsafe,
  OmniVoiceSessionReportProjection,
  OmniVoiceSessionReportRecord,
  OmniVoiceTranscriptSegment,
  exampleOmniVoiceCommandProposal,
  exampleOmniVoiceSessionReport,
  exampleOmniVoiceTranscriptSegment,
  projectOmniVoiceSessionReport,
} from './omni-voice-session-evidence'

const nowIso = '2026-06-06T22:30:00.000Z'

const sessionRecord = (
  overrides: Partial<OmniVoiceSessionReportRecord> = {},
): OmniVoiceSessionReportRecord =>
  S.decodeUnknownSync(OmniVoiceSessionReportRecord)({
    ...exampleOmniVoiceSessionReport(),
    ...overrides,
  })

const transcriptSegment = (
  overrides: Partial<OmniVoiceTranscriptSegment> = {},
): OmniVoiceTranscriptSegment =>
  S.decodeUnknownSync(OmniVoiceTranscriptSegment)({
    ...exampleOmniVoiceTranscriptSegment(),
    ...overrides,
  })

const commandProposal = (
  overrides: Partial<OmniVoiceCommandProposal> = {},
): OmniVoiceCommandProposal =>
  S.decodeUnknownSync(OmniVoiceCommandProposal)({
    ...exampleOmniVoiceCommandProposal(),
    ...overrides,
  })

describe('Omni voice session evidence', () => {
  test('projects voice session reports with transcript evidence, proposal counts, friendly time, and no side effects', () => {
    const projection = projectOmniVoiceSessionReport(
      exampleOmniVoiceSessionReport(),
      'customer',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniVoiceSessionReportProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      approvalMutationAllowed: false,
      approvedProposalCount: 0,
      audioCaptureMutationAllowed: false,
      captureState: 'transcribed',
      captureStateLabel: 'Transcribed',
      commandExecutionAllowed: false,
      createdAtDisplay: '30 minutes ago',
      executedProposalCount: 1,
      pendingApprovalCount: 1,
      proposalMutationAllowed: false,
      providerKind: 'realtime_api',
      providerMutationAllowed: false,
      publicClaimUpgradeAllowed: false,
      transcriptMutationAllowed: false,
      transcriptSegmentCount: 2,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
    )
    expect(projection.commandProposals[0]).toMatchObject({
      approvalRequired: true,
      expiresAtDisplay: 'Expires in 1 hour',
      routeLabel: 'Site revision feedback',
      riskLabel: 'Medium risk',
      stateLabel: 'Needs approval',
    })
    expect(projection.commandProposals[1]).toMatchObject({
      routeLabel: 'Forum post',
      stateLabel: 'Executed',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('requires transcript source/evidence refs, valid confidence, valid duration, and transcript segments for transcribed sessions', () => {
    for (const badSession of [
      sessionRecord({ sourceRefs: [] }),
      sessionRecord({ redactionPolicyRefs: [] }),
      sessionRecord({ transcriptSegments: [] }),
      sessionRecord({
        transcriptSegments: [transcriptSegment({ evidenceRefs: [] })],
      }),
      sessionRecord({
        transcriptSegments: [transcriptSegment({ sourceRefs: [] })],
      }),
      sessionRecord({
        transcriptSegments: [transcriptSegment({ confidenceBps: 10_001 })],
      }),
      sessionRecord({
        transcriptSegments: [transcriptSegment({ durationMillis: -1 })],
      }),
    ]) {
      expect(() =>
        projectOmniVoiceSessionReport(badSession, 'operator', nowIso),
      ).toThrow(OmniVoiceSessionEvidenceUnsafe)
    }
  })

  test('validates proposal state transitions, source segment refs, approval receipts, execution receipts, blockers, and expiry', () => {
    for (const badProposal of [
      commandProposal({ evidenceRefs: [] }),
      commandProposal({ sourceSegmentRefs: [] }),
      commandProposal({ sourceSegmentRefs: ['segment.public.missing'] }),
      commandProposal({
        approvalRequirement: 'not_required',
        riskLevel: 'high',
      }),
      commandProposal({
        approvalRequirement: 'not_required',
        routeKind: 'payment',
        riskLevel: 'medium',
      }),
      commandProposal({
        approvalReceiptRefs: [],
        state: 'approved',
      }),
      commandProposal({
        approvalReceiptRefs: ['approval.public.operator_approved'],
        executionReceiptRefs: [],
        state: 'executed',
      }),
      commandProposal({
        blockedReasonRefs: [],
        state: 'blocked',
      }),
      commandProposal({
        expiresAtIso: null,
        state: 'expired',
      }),
    ]) {
      expect(() =>
        projectOmniVoiceSessionReport(
          sessionRecord({ commandProposals: [badProposal] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OmniVoiceSessionEvidenceUnsafe)
    }
  })

  test('redacts private names, provider refs, idempotency refs, transcript refs, receipts, and private source material publicly', () => {
    const projection = projectOmniVoiceSessionReport(
      sessionRecord({
        approvalReceiptRefs: [
          'approval.public.operator_approved',
          'approval.private.operator_notes',
        ],
        languageRef: 'language.private.customer_language',
        providerRef: 'provider.private.openai_realtime_payload',
        receiptRefs: [
          'receipt.public.voice_session_report',
          'receipt.private.operator_receipt',
        ],
        sessionRef: 'session.private.operator_session',
        sourceRefs: [
          'source.public.browser_audio_summary',
          'source.private.operator_audio_notes',
        ],
        transcriptSegments: [
          transcriptSegment({
            segmentRef: 'segment.private.operator_segment',
            sourceRefs: [
              'source.public.browser_audio_summary',
              'source.private.operator_audio_notes',
            ],
            textRef: 'text.private.operator_segment',
          }),
        ],
        commandProposals: [
          commandProposal({
            approvalReceiptRefs: [
              'approval.public.operator_approved',
              'approval.private.operator_notes',
            ],
            idempotencyKeyRef: 'idempotency.private.operator_key_ref',
            proposalRef: 'proposal.private.operator_proposal',
            sourceSegmentRefs: ['segment.private.operator_segment'],
            titleRef: 'title.private.operator_title',
          }),
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.languageRef).toBe('language.redacted')
    expect(projection.providerRef).toBe('provider_ref.redacted')
    expect(projection.sessionRef).toBe('session.redacted')
    expect(projection.receiptRefs).toEqual(['receipt.public.voice_session_report'])
    expect(projection.sourceRefs).toEqual(['source.public.browser_audio_summary'])
    expect(projection.transcriptSegments[0]?.segmentRef).toBe('segment.redacted')
    expect(projection.transcriptSegments[0]?.textRef).toBe('text.redacted')
    expect(projection.commandProposals[0]?.idempotencyKeyRef).toBe(
      'idempotency.redacted',
    )
    expect(projection.commandProposals[0]?.proposalRef).toBe('proposal.redacted')
    expect(projection.commandProposals[0]?.titleRef).toBe('title.redacted')
    expect(serialized).not.toMatch(
      /(approval|idempotency|language|proposal|provider|receipt|segment|session|source|text|title)\.private/,
    )
  })

  test('rejects side-effect authority, raw provider payloads, private names or contact refs, payment material, and raw timestamps', () => {
    for (const badInput of [
      () =>
        projectOmniVoiceSessionReport(
          sessionRecord({
            authority: {
              ...OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
              noCommandExecution: false,
            },
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniVoiceSessionReport(
          sessionRecord({
            providerRef: 'provider.public.raw_provider_payload',
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniVoiceSessionReport(
          sessionRecord({
            transcriptSegments: [
              transcriptSegment({
                textRef: 'text.public.customer_email_removed',
              }),
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniVoiceSessionReport(
          sessionRecord({
            commandProposals: [
              commandProposal({
                receiptRefs: ['receipt.public.payment_hash_abcd'],
              }),
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniVoiceSessionReport(
          sessionRecord({
            sourceRefs: ['source.public.2026-06-06T22:25:00.000Z'],
          }),
          'operator',
          nowIso,
        ),
    ]) {
      expect(badInput).toThrow(OmniVoiceSessionEvidenceUnsafe)
    }
  })
})
