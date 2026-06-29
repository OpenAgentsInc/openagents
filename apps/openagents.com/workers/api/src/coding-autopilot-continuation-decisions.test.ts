import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BlueprintContinuationDecision } from './blueprint/schemas/continuation-decision'
import {
  CodingAutopilotContinuationDecisionProjection,
  CodingAutopilotContinuationDecisionRecord,
  CodingAutopilotContinuationDecisionUnsafe,
  codingAutopilotContinuationDecisionProjectionHasPrivateMaterial,
  codingAutopilotContinuationDecisionRecordFromBlueprint,
  exampleCodingAutopilotContinuationDecisionRecord,
  projectCodingAutopilotContinuationDecisionRecord,
} from './coding-autopilot-continuation-decisions'

const nowIso = '2026-06-06T21:05:00.000Z'

const blueprintDecision = (
  action: BlueprintContinuationDecision['action'],
): BlueprintContinuationDecision => ({
  action,
  actionSubmissionRequiredForDirectEffects: true,
  authorityBoundary: 'evidence_only',
  confidence: 0.75,
  constraintRefs: ['constraint.test'],
  decisionRef: `continuation_decision.${action}`,
  directMutationDisabled: true,
  evidenceRefs: ['evidence.test'],
  forbiddenDirectEffects: [
    'create_pull_request',
    'deploy',
    'mutate_source_fact',
    'send_email',
    'spend_money',
    'upgrade_public_claim',
  ],
  moduleVersionId: `module_version.autopilot.${action}.candidate_1`,
  noDeploy: true,
  noEmail: true,
  noPublicClaimUpgrade: true,
  noSourceMutation: true,
  noSpend: true,
  programSignatureId: `program_signature.autopilot.${action}.v1`,
  programTypeId: 'program_type.autopilot.continuation.v1',
  reason: 'Test decision.',
  receiptRefs: ['receipt.test'],
  sourceAuthorityRefs: ['source_authority.test'],
  turnResultRef: `turn_result.${action}`,
  workRef: 'mission.test',
})

describe('Coding on Autopilot continuation decision records', () => {
  test('projects an evidence-only between-turn decision across audiences', () => {
    const record = exampleCodingAutopilotContinuationDecisionRecord()
    const publicProjection = projectCodingAutopilotContinuationDecisionRecord(
      record,
      'public',
      nowIso,
    )
    const customerProjection = projectCodingAutopilotContinuationDecisionRecord(
      record,
      'customer',
      nowIso,
    )
    const operatorProjection = projectCodingAutopilotContinuationDecisionRecord(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotContinuationDecisionRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(CodingAutopilotContinuationDecisionProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      actionSubmissionRequiredForDirectEffects: true,
      audience: 'public',
      confidence: 0.82,
      confidenceBucket: 'medium',
      directEffectPermitted: false,
      evidenceOnly: true,
      guardrailState: 'needs_action_submission',
      queuedActionKind: 'retry_account',
      selectedContinuationAction: 'retry_account',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(publicProjection.programRunRef).toBe(null)
    expect(publicProjection.workroomRefs).toEqual([])
    expect(publicProjection.sourceAuthorityRefs).toEqual([])
    expect(customerProjection.programRunRef).toBe(
      'program_run.continuation.otec_revision_4',
    )
    expect(customerProjection.workroomRefs).toEqual([
      'workroom.otec_site_revision_4',
    ])
    expect(customerProjection.sourceAuthorityRefs).toEqual([])
    expect(operatorProjection.sourceAuthorityRefs).toEqual([
      'source_authority.account_fleet_health',
    ])
  })

  test('maps Blueprint continuation actions to queue action kinds', () => {
    const actions = [
      ['continue', 'continue'],
      ['escalate', 'request_customer_input'],
      ['fix', 'steer'],
      ['prepare_review', 'approve_pr_draft'],
      ['request_context', 'provide_context'],
      ['retry_account', 'retry_account'],
      ['stop', 'stop'],
      ['summarize', 'continue'],
      ['test', 'rerun_tests'],
    ] as const
    const queued = actions.map(([action]) =>
      codingAutopilotContinuationDecisionRecordFromBlueprint({
        customerExplanationRef: `explanation.${action}`,
        decision: blueprintDecision(action),
        guardrailState: 'passed',
        id: `continuation_decision_record_${action}`,
        missionRef: 'mission.test',
        programRunRef: 'program_run.test',
        updatedAtIso: '2026-06-06T21:00:00.000Z',
      }).queuedActionKind,
    )

    expect(queued).toEqual(actions.map(([, queuedAction]) => queuedAction))
  })

  test('rejects non-evidence-only Blueprint continuation decisions', () => {
    expect(() =>
      codingAutopilotContinuationDecisionRecordFromBlueprint({
        customerExplanationRef: 'explanation.bad',
        decision: {
          ...blueprintDecision('continue'),
          actionSubmissionRequiredForDirectEffects: false,
          directMutationDisabled: false,
        },
        guardrailState: 'failed',
        id: 'continuation_decision_record_bad',
        missionRef: 'mission.bad',
        programRunRef: 'program_run.bad',
        updatedAtIso: '2026-06-06T21:00:00.000Z',
      }),
    ).toThrow(CodingAutopilotContinuationDecisionUnsafe)
  })

  test('does not expose raw timestamps and rejects unsafe refs', () => {
    const projection = projectCodingAutopilotContinuationDecisionRecord(
      exampleCodingAutopilotContinuationDecisionRecord(),
      'customer',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(codingAutopilotContinuationDecisionProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(() =>
      projectCodingAutopilotContinuationDecisionRecord({
        ...exampleCodingAutopilotContinuationDecisionRecord(),
        evidenceRefs: ['raw_runner_payload:mission'],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotContinuationDecisionUnsafe)
    expect(() =>
      projectCodingAutopilotContinuationDecisionRecord({
        ...exampleCodingAutopilotContinuationDecisionRecord(),
        sourceAuthorityRefs: ['provider_token:abc'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotContinuationDecisionUnsafe)
    expect(() =>
      projectCodingAutopilotContinuationDecisionRecord({
        ...exampleCodingAutopilotContinuationDecisionRecord(),
        rejectedAlternativeRefs: ['private_repo:https://github.com/customer/private-repo'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotContinuationDecisionUnsafe)
    expect(() =>
      projectCodingAutopilotContinuationDecisionRecord({
        ...exampleCodingAutopilotContinuationDecisionRecord(),
        riskRefs: ['ben@example.com'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotContinuationDecisionUnsafe)
  })
})
