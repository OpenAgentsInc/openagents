import { Schema as S } from 'effect'

import {
  type BlueprintContinuationDecisionKind as BlueprintContinuationDecisionKindType,
  BlueprintContinuationDecisionKind,
  BlueprintContinuationTurnResult,
} from '../schemas/continuation-decision'
import { BlueprintEvalFixture } from '../schemas/release-gate'

export const BlueprintContinuationDecisionFixture = S.Struct({
  evalFixture: BlueprintEvalFixture,
  expectedDecision: BlueprintContinuationDecisionKind,
  id: S.String,
  programSignatureId: S.String,
  publicSafeSummaryRef: S.String,
  redactionPolicyRef: S.String,
  scorecardRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  turnResult: BlueprintContinuationTurnResult,
})
export type BlueprintContinuationDecisionFixture =
  typeof BlueprintContinuationDecisionFixture.Type

const fixture = (input: {
  expectedDecision: BlueprintContinuationDecisionKindType
  id: string
  publicSafeSummaryRef: string
  sourceRefs: ReadonlyArray<string>
  turnResult: BlueprintContinuationDecisionFixture['turnResult']
}): BlueprintContinuationDecisionFixture => {
  const programSignatureId = `program_signature.autopilot.${input.expectedDecision}.v1`
  const scorecardRefs = [`scorecard.continuation.${input.id}`]

  return {
    evalFixture: {
      evidenceRefs: input.turnResult.evidenceRefs,
      expectedOutputRef: `expected.continuation.${input.expectedDecision}`,
      fixtureKind: 'continuation_decision',
      id: `fixture.continuation.${input.id}`,
      inputRef: input.turnResult.id,
      result: 'passed',
      scorecardRefs,
    },
    expectedDecision: input.expectedDecision,
    id: input.id,
    programSignatureId,
    publicSafeSummaryRef: input.publicSafeSummaryRef,
    redactionPolicyRef: 'redaction.blueprint.continuation_fixture.public_v1',
    scorecardRefs,
    sourceRefs: input.sourceRefs,
    turnResult: input.turnResult,
  }
}

const baseTurn = {
  accountFailureRefs: [],
  actorRef: 'actor.openagents.operator',
  blockerRefs: [],
  buildFailureRefs: [],
  classifierConfidence: 0.84,
  constraintRefs: ['constraint.no_direct_mutation'],
  evidenceRefs: ['evidence.first_batch.context_pack'],
  missingContextRefs: [],
  readyArtifactRefs: [],
  receiptRefs: ['receipt.program_run.first_batch'],
  runtimeFailureRefs: [],
  sourceAuthorityRefs: ['source.docs.public_safe'],
  state: 'completed' as const,
  summaryNeeded: false,
  testFailureRefs: [],
  unverifiedChangeRefs: [],
  updatedAt: '2026-06-06T00:00:00.000Z',
  userRequestedEscalation: false,
  userRequestedStop: false,
  workRef: 'workroom.first_batch',
}

export const BLUEPRINT_CONTINUATION_DECISION_FIXTURES = [
  fixture({
    expectedDecision: 'continue',
    id: 'successful_pylon_campaign_continuation',
    publicSafeSummaryRef:
      'summary.continuation.successful_public_surface_slice_continue',
    sourceRefs: [
      'docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md',
      'docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: [
        'evidence.preflight.ok',
        'evidence.public_surface_slice.merged',
      ],
      id: 'turn_result.successful_pylon_campaign_continuation',
      workRef: 'workroom.pylon_campaign',
    },
  }),
  fixture({
    expectedDecision: 'test',
    id: 'site_builder_generated_changes_need_verification',
    publicSafeSummaryRef: 'summary.continuation.generated_changes_need_tests',
    sourceRefs: [
      'docs/sites/2026-06-05-sites-builder-saved-version-handoff.md',
      'docs/sites/2026-06-05-sites-self-serve-builder-ui.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: ['evidence.builder.saved_candidate'],
      id: 'turn_result.site_builder_generated_changes_need_verification',
      unverifiedChangeRefs: ['artifact.site_builder.generated_files'],
      workRef: 'workroom.site_builder',
    },
  }),
  fixture({
    expectedDecision: 'fix',
    id: 'site_builder_failed_repair_attempt',
    publicSafeSummaryRef: 'summary.continuation.builder_failure_needs_fix',
    sourceRefs: ['docs/sites/2026-06-05-sites-builder-repair-loop.md'],
    turnResult: {
      ...baseTurn,
      buildFailureRefs: ['failure.site_builder.redacted_build_error'],
      evidenceRefs: ['evidence.site_builder.repair_attempt.failed'],
      id: 'turn_result.site_builder_failed_repair_attempt',
      state: 'failed',
      workRef: 'workroom.site_builder',
    },
  }),
  fixture({
    expectedDecision: 'summarize',
    id: 'returning_operator_needs_state_summary',
    publicSafeSummaryRef: 'summary.continuation.operator_needs_summary',
    sourceRefs: [
      'docs/autopilot-tasks/2026-06-05-adjutant-site-fulfillment-runbook.md',
      'docs/omni/2026-06-05-mission-briefing-v1.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: ['evidence.workroom.current_state_public_safe'],
      id: 'turn_result.returning_operator_needs_state_summary',
      summaryNeeded: true,
      workRef: 'workroom.site_fulfillment',
    },
  }),
  fixture({
    expectedDecision: 'request_context',
    id: 'customer_site_revision_missing_assets',
    publicSafeSummaryRef: 'summary.continuation.customer_context_missing',
    sourceRefs: [
      'docs/sites/2026-06-05-ben-otec-revision-readiness.md',
      'docs/sites/2026-06-05-customer-site-revision-feedback-api.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: ['evidence.customer_feedback.open'],
      id: 'turn_result.customer_site_revision_missing_assets',
      missingContextRefs: ['context.customer_visual_assets_needed'],
      workRef: 'workroom.ben_otec_site',
    },
  }),
  fixture({
    expectedDecision: 'retry_account',
    id: 'account_fleet_capacity_retry',
    publicSafeSummaryRef: 'summary.continuation.account_capacity_retry',
    sourceRefs: [
      'docs/2026-06-05-openagents-agent-surface-gap-analysis.md',
      'docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md',
    ],
    turnResult: {
      ...baseTurn,
      accountFailureRefs: ['provider_account.capacity.redacted_low_credit'],
      evidenceRefs: ['evidence.provider_account.capacity_check_failed'],
      id: 'turn_result.account_fleet_capacity_retry',
      state: 'interrupted',
      workRef: 'workroom.account_fleet',
    },
  }),
  fixture({
    expectedDecision: 'stop',
    id: 'superseded_manual_completion_stop',
    publicSafeSummaryRef: 'summary.continuation.superseded_stop',
    sourceRefs: [
      'docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: ['evidence.foreground_operator_completed_work'],
      id: 'turn_result.superseded_manual_completion_stop',
      userRequestedStop: true,
      workRef: 'workroom.superseded_run',
    },
  }),
  fixture({
    expectedDecision: 'escalate',
    id: 'quality_gate_blocks_customer_review',
    publicSafeSummaryRef: 'summary.continuation.quality_gate_escalation',
    sourceRefs: [
      'docs/sites/2026-06-05-ben-otec-site-quality-postmortem.md',
    ],
    turnResult: {
      ...baseTurn,
      blockerRefs: ['blocker.site_quality_gate_failed'],
      evidenceRefs: ['evidence.site_quality.postmortem'],
      id: 'turn_result.quality_gate_blocks_customer_review',
      state: 'blocked',
      workRef: 'workroom.ben_otec_site',
    },
  }),
  fixture({
    expectedDecision: 'prepare_review',
    id: 'ben_otec_revision_ready_for_review',
    publicSafeSummaryRef: 'summary.continuation.revision_ready_for_review',
    sourceRefs: [
      'docs/sites/2026-06-05-ben-otec-revision-readiness.md',
      'docs/sites/2026-06-05-ben-otec-revision-2-review-email.md',
    ],
    turnResult: {
      ...baseTurn,
      evidenceRefs: ['evidence.site_revision.customer_review_ready'],
      id: 'turn_result.ben_otec_revision_ready_for_review',
      readyArtifactRefs: ['site_version.otec.review_ready'],
      receiptRefs: ['receipt.site_revision.email_queued'],
      workRef: 'workroom.ben_otec_site',
    },
  }),
]

const privateFixtureMaterialPattern =
  /(bearer\s+|cookie|mnemonic|oauth|oa_agent_|openagents_admin|password|preimage|private[_-]?key|provider[_-]?token|raw[_-]?runner|runner[_-]?log|secret|sk-[a-z0-9]|token)/i

export const blueprintContinuationDecisionFixtureHasPrivateMaterial = (
  fixture: BlueprintContinuationDecisionFixture,
): boolean => privateFixtureMaterialPattern.test(JSON.stringify(fixture))
