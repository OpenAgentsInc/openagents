import { describe, expect, test } from 'vitest'

import type { OmniEvidenceBundleRecord } from './omni-evidence-bundles'
import { buildOmniMissionBriefing } from './omni-mission-briefing'
import type { OmniWorkroomRecord } from './omni-workrooms'
import type { OmniWorkroomLifecycleDecisionRecord } from './omni-workroom-lifecycle'

const nowIso = '2026-06-06T01:30:00.000Z'

const workroom = (
  overrides: Partial<OmniWorkroomRecord> = {},
): OmniWorkroomRecord => ({
  acceptedOutcomeContractId: null,
  archivedAt: null,
  artifactRefs: [],
  assignmentId: null,
  blockerRefs: [],
  classificationCaveatRef: 'classification_caveat_customer_reviewed',
  createdAt: '2026-06-06T00:00:00.000Z',
  customerIntentRef: 'customer_intent_1',
  dataClassification: 'customer',
  emailRefs: [],
  id: 'omni_workroom_site_1',
  idempotencyKey: 'omni-workroom:site-1',
  metadata: {},
  publicReceiptRef: 'omni_workroom_public_receipt_1',
  receiptRefs: [],
  siteId: 'site_project_1',
  softwareOrderId: 'software_order_1',
  sourceRefs: [],
  status: 'active',
  taskPacketRef: null,
  trustTier: 'reviewed',
  updatedAt: '2026-06-06T01:15:00.000Z',
  visibility: 'customer',
  workKind: 'site',
  ...overrides,
})

const evidenceBundle = (
  overrides: Partial<OmniEvidenceBundleRecord> = {},
): OmniEvidenceBundleRecord => ({
  archivedAt: null,
  createdAt: '2026-06-06T00:40:00.000Z',
  entries: [
    {
      caveatRef: null,
      entryKind: 'research_brief',
      publicSafe: true,
      redactionState: 'redacted',
      ref: 'research_brief_otec_1',
      required: true,
      sourceAuthority: 'operator_reviewed',
      summaryRef: 'research_brief_summary_otec_1',
      visibility: 'customer',
    },
    {
      caveatRef: null,
      entryKind: 'deployment_url',
      publicSafe: true,
      redactionState: 'not_needed',
      ref: 'https://sites.openagents.com/otec/revisions/2',
      required: true,
      sourceAuthority: 'system_receipt',
      summaryRef: 'deployment_summary_otec_2',
      visibility: 'public',
    },
    {
      caveatRef: null,
      entryKind: 'build_log',
      publicSafe: false,
      redactionState: 'private_only',
      ref: 'raw_run_log_1',
      required: true,
      sourceAuthority: 'system_receipt',
      summaryRef: 'raw_run_log_summary_1',
      visibility: 'team',
    },
  ],
  id: 'omni_evidence_bundle_1',
  idempotencyKey: 'omni-evidence:site-1',
  legalSensitive: false,
  metadata: {},
  publicReceiptRef: 'omni_evidence_public_receipt_1',
  sourceAuthorityCaveatRef: null,
  status: 'ready',
  summaryRef: 'evidence_summary_1',
  updatedAt: '2026-06-06T01:00:00.000Z',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
  ...overrides,
})

const lifecycleDecision = (
  overrides: Partial<OmniWorkroomLifecycleDecisionRecord> = {},
): OmniWorkroomLifecycleDecisionRecord => ({
  actorKind: 'customer',
  archivedAt: null,
  artifactRef: 'site_version_ref_2',
  createdAt: '2026-06-06T01:20:00.000Z',
  customerSafeExplanationRef: 'customer_explanation_revision_requested',
  decisionKind: 'request_revision',
  followupRequestRef: null,
  id: 'omni_lifecycle_decision_1',
  idempotencyKey: 'omni-lifecycle:site-revision-1',
  metadata: {},
  noSettlementImplication: true,
  receiptRef: 'receipt_site_revision_requested_1',
  resultingState: 'revision_requested',
  siteRevisionFeedbackRef: 'site_revision_feedback_1',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
  ...overrides,
})

describe('Omni Mission Briefing', () => {
  test('builds an empty briefing with a direct next action', () => {
    const briefing = buildOmniMissionBriefing({
      nowIso,
      workroom: workroom(),
    })

    expect(briefing.empty).toBe(true)
    expect(briefing.generatedAtDisplay).toBe('15 minutes ago')
    expect(briefing.sections.nextAction).toEqual([
      {
        displayTime: null,
        kind: 'next_action',
        ref: 'next_action_work_in_progress',
        status: 'pending',
        summaryRef: 'next_action_work_in_progress:summary',
      },
    ])
  })

  test('builds partial blocked and emailed briefing sections', () => {
    const briefing = buildOmniMissionBriefing({
      nowIso,
      workroom: workroom({
        blockerRefs: ['blocker_customer_assets_needed'],
        emailRefs: ['email_message_review_ready_1'],
        status: 'blocked',
      }),
    })

    expect(briefing.empty).toBe(false)
    expect(briefing.sections.blocked).toHaveLength(1)
    expect(briefing.sections.email).toHaveLength(1)
    expect(briefing.sections.nextAction[0]?.ref).toBe(
      'next_action_clear_blocker',
    )
  })

  test('builds full customer-safe changed, built, review, and email sections', () => {
    const briefing = buildOmniMissionBriefing({
      evidenceBundles: [evidenceBundle()],
      lifecycleDecisions: [lifecycleDecision()],
      nowIso,
      workroom: workroom({
        artifactRefs: ['site_version_ref_2'],
        emailRefs: ['email_message_review_ready_1'],
        status: 'waiting_review',
      }),
    })

    expect(briefing.sections.changed.map(item => item.ref)).toEqual([
      'research_brief_otec_1',
    ])
    expect(briefing.sections.built.map(item => item.ref)).toEqual([
      'site_version_ref_2',
      'https://sites.openagents.com/otec/revisions/2',
    ])
    expect(briefing.sections.review[0]).toMatchObject({
      ref: 'receipt_site_revision_requested_1',
      status: 'needs_review',
    })
    expect(briefing.sections.email).toHaveLength(1)
    expect(briefing.sections.nextAction[0]?.ref).toBe(
      'next_action_revision_queue',
    )
  })

  test('redacts unsafe refs and never projects raw timestamps', () => {
    const briefing = buildOmniMissionBriefing({
      evidenceBundles: [
        evidenceBundle({
          entries: [
            {
              caveatRef: null,
              entryKind: 'deployment_url',
              publicSafe: true,
              redactionState: 'not_needed',
              ref: 'raw_email_body_customer@example.com',
              required: true,
              sourceAuthority: 'system_receipt',
              summaryRef: '2026-06-06T01:00:00.000Z',
              visibility: 'public',
            },
          ],
        }),
      ],
      lifecycleDecisions: [
        lifecycleDecision({
          customerSafeExplanationRef: 'raw_run_log_private',
        }),
      ],
      nowIso,
      workroom: workroom({
        emailRefs: ['customer_email_ben@example.com'],
      }),
    })
    const projected = JSON.stringify(briefing)

    expect(briefing.sections.built).toEqual([])
    expect(briefing.sections.review).toEqual([])
    expect(briefing.sections.email).toEqual([])
    expect(projected).not.toContain('2026-06-06T')
    expect(projected).not.toContain('@example.com')
    expect(projected).not.toContain('raw_run_log')
  })
})
