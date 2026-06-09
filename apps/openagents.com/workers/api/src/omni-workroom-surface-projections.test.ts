import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import type { OmniEvidenceBundleRecord } from './omni-evidence-bundles'
import type { OmniRouteScorecardRecord } from './omni-route-scorecards'
import type { OmniWorkroomRecord } from './omni-workrooms'
import type { OmniWorkroomLifecycleDecisionRecord } from './omni-workroom-lifecycle'
import { buildOmniWorkroomSurfaceProjection } from './omni-workroom-surface-projections'

const workroom = (
  overrides: Partial<OmniWorkroomRecord> = {},
): OmniWorkroomRecord => ({
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
  archivedAt: null,
  artifactRefs: ['site_version_ref_2'],
  assignmentId: 'adjutant_assignment_1',
  blockerRefs: [],
  classificationCaveatRef: 'classification_caveat_public_reviewed',
  createdAt: '2026-06-06T02:00:00.000Z',
  customerIntentRef: 'customer_intent_otec',
  dataClassification: 'public',
  emailRefs: ['email_receipt_revision_2'],
  id: 'omni_workroom_site_1',
  idempotencyKey: 'omni-workroom:site-1',
  metadata: { operatorNoteRef: 'operator_note_private' },
  publicReceiptRef: 'omni_workroom_public_receipt_1',
  receiptRefs: ['receipt_revision_ready'],
  siteId: 'site_project_otec',
  softwareOrderId: 'software_order_otec',
  sourceRefs: ['exa_source_card_otec'],
  status: 'waiting_review',
  taskPacketRef: 'task_packet_otec_revision_2',
  trustTier: 'reviewed',
  updatedAt: '2026-06-06T02:30:00.000Z',
  visibility: 'public',
  workKind: 'site',
  ...overrides,
})

const evidenceBundle = (): OmniEvidenceBundleRecord => ({
  archivedAt: null,
  createdAt: '2026-06-06T02:10:00.000Z',
  entries: [
    {
      caveatRef: null,
      entryKind: 'deployment_url',
      publicSafe: true,
      redactionState: 'not_needed',
      ref: 'site_revision_2_url_ref',
      required: true,
      sourceAuthority: 'system_receipt',
      summaryRef: 'deployment_summary_revision_2',
      visibility: 'public',
    },
    {
      caveatRef: 'private_log_caveat',
      entryKind: 'build_log',
      publicSafe: false,
      redactionState: 'private_only',
      ref: 'raw_run_log_private',
      required: true,
      sourceAuthority: 'agent_generated',
      summaryRef: 'build_log_summary_private',
      visibility: 'private',
    },
  ],
  id: 'omni_evidence_bundle_1',
  idempotencyKey: 'evidence:site:revision-2',
  legalSensitive: false,
  metadata: { private: 'operator only' },
  publicReceiptRef: 'evidence_public_receipt_1',
  sourceAuthorityCaveatRef: null,
  status: 'ready',
  summaryRef: 'evidence_summary_revision_2',
  updatedAt: '2026-06-06T02:20:00.000Z',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
})

const lifecycleDecision = (): OmniWorkroomLifecycleDecisionRecord => ({
  actorKind: 'customer',
  archivedAt: null,
  artifactRef: 'site_version_ref_2',
  createdAt: '2026-06-06T02:25:00.000Z',
  customerSafeExplanationRef: 'revision_ready_summary',
  decisionKind: 'request_revision',
  followupRequestRef: 'followup_request_ref_1',
  id: 'omni_lifecycle_decision_1',
  idempotencyKey: 'lifecycle:site:revision-2',
  metadata: { private: 'operator only' },
  noSettlementImplication: true,
  receiptRef: 'lifecycle_receipt_revision_2',
  resultingState: 'revision_requested',
  siteRevisionFeedbackRef: 'site_feedback_ref_1',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
})

const economics = (): OmniAcceptedOutcomeEconomicsRecord => ({
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
  acceptedValueCents: 5000,
  archivedAt: null,
  artifactCostCents: 100,
  buyerPriceAsset: 'none',
  buyerPriceCents: 0,
  createdAt: '2026-06-06T02:15:00.000Z',
  creditsCharged: 0,
  fundingMode: 'free_beta',
  grossMarginCents: -1100,
  id: 'omni_economics_1',
  idempotencyKey: 'economics:site:revision-2',
  internalCaveatRef: 'internal_cost_caveat',
  metadata: { private: 'operator only' },
  noSettlementImplication: true,
  providerCostCents: 400,
  publicCaveatRef: 'free_beta_no_settlement_caveat',
  retryCostCents: 100,
  reviewCostCents: 300,
  reviewMinutes: 20,
  runnerCostCents: 200,
  satsCharged: 0,
  totalCostCents: 1100,
  updatedAt: '2026-06-06T02:18:00.000Z',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
})

const routeScorecard = (): OmniRouteScorecardRecord => ({
  archivedAt: null,
  costCents: 900,
  createdAt: '2026-06-06T02:12:00.000Z',
  decisionReasonRefs: ['route_reason_quality'],
  id: 'omni_route_scorecard_1',
  idempotencyKey: 'route:site:revision-2',
  latencyMs: 32000,
  metadata: { private: 'operator only' },
  observedResultKind: 'success',
  observedResultRef: 'observed_result_revision_2',
  postCloseoutScore: 87,
  privacyTier: 'customer',
  publicCaveatRef: 'route_public_caveat',
  rejectedCandidates: [
    {
      candidateRef: 'route_candidate_fast_low_quality',
      reasonKind: 'quality',
      reasonRef: 'route_reason_quality',
    },
  ],
  selectedAccountRef: 'provider_account_private',
  selectedModelRef: 'model_gpt_5_codex',
  selectedProviderRef: 'provider_openai',
  selectedRouteRef: 'route_shc_codex_pool',
  selectedRuntimeRef: 'runtime_shc_archlinux',
  trustTier: 'reviewed',
  updatedAt: '2026-06-06T02:13:00.000Z',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
})

const build = (surface: 'public' | 'customer' | 'team' | 'agent' | 'operator') =>
  buildOmniWorkroomSurfaceProjection({
    economics: [economics()],
    evidenceBundles: [evidenceBundle()],
    lifecycleDecisions: [lifecycleDecision()],
    routeScorecards: [routeScorecard()],
    surface,
    workroom: workroom(surface === 'team' ? { dataClassification: 'team' } : {}),
  })

describe('Omni workroom surface projections', () => {
  test('public projection keeps only public-safe proof material', () => {
    const projection = build('public')
    const publicEvidenceBundle = projection.evidenceBundles.at(0)

    expect(projection.workroom).not.toHaveProperty('sourceRefs')
    expect(projection.workroom).not.toHaveProperty('taskPacketRef')
    expect(publicEvidenceBundle).toBeDefined()
    expect(publicEvidenceBundle?.entries).toEqual([
      {
        entryKind: 'deployment_url',
        ref: 'site_revision_2_url_ref',
        sourceAuthority: 'system_receipt',
        summaryRef: 'deployment_summary_revision_2',
      },
    ])
    expect(projection.economics[0]).toEqual({
      fundingMode: 'free_beta',
      noSettlementImplication: true,
      publicCaveatRef: 'free_beta_no_settlement_caveat',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
  })

  test('customer and agent surfaces stay customer-safe', () => {
    const customer = build('customer')
    const agent = build('agent')
    const agentEvidenceBundle = agent.evidenceBundles.at(0)
    const agentEvidenceEntry = agentEvidenceBundle?.entries.at(0)

    expect(customer.workroom).toHaveProperty('customerIntentRef')
    expect(customer.workroom).not.toHaveProperty('sourceRefs')
    expect(customer.routeScorecards[0]).toHaveProperty('decisionReasonRefs')
    expect(agent.workroom).not.toHaveProperty('customerIntentRef')
    expect(agent.routeScorecards[0]).not.toHaveProperty('selectedRouteRef')
    expect(agentEvidenceEntry).toHaveProperty('redactionState')
  })

  test('team and operator surfaces have broader internal context without raw provider secrets', () => {
    const team = build('team')
    const operator = build('operator')

    expect(team.workroom).toHaveProperty('sourceRefs')
    expect(team.workroom).not.toHaveProperty('taskPacketRef')
    expect(operator.workroom).toHaveProperty('taskPacketRef')
    expect(operator.evidenceBundles[0]).toHaveProperty('metadata')
    expect(operator.routeScorecards[0]).toHaveProperty('selectedAccountRef')
    expect(operator.economics[0]).toHaveProperty('totalCostCents')
  })

  test('classification gates block the wrong surface', () => {
    expect(() =>
      buildOmniWorkroomSurfaceProjection({
        surface: 'public',
        workroom: workroom({ dataClassification: 'customer' }),
      }),
    ).toThrow()

    expect(() =>
      buildOmniWorkroomSurfaceProjection({
        surface: 'customer',
        workroom: workroom({ dataClassification: 'payment_private' }),
      }),
    ).toThrow()
  })
})
