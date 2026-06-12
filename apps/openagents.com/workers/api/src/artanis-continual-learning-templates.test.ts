import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'

import {
  ARTANIS_CONTINUAL_LEARNING_NO_EXECUTION_AUTHORITY,
  ARTANIS_CONTINUAL_LEARNING_TEMPLATE_KINDS,
  ArtanisContinualLearningTemplateLedgerProjection,
  ArtanisContinualLearningTemplateLedgerRecord,
  ArtanisContinualLearningTemplateRecord,
  ArtanisContinualLearningTemplateUnsafe,
  artanisContinualLearningTemplateProjectionHasPrivateMaterial,
  exampleArtanisContinualLearningTemplateLedger,
  projectArtanisContinualLearningTemplates,
  pylonMarketplaceIntakeRequestFromTemplate,
  pylonMarketplaceTriageRequestFromTemplate,
} from './artanis-continual-learning-templates'

const nowIso = '2026-06-07T06:10:00.000Z'

const ledgerRecord = (
  overrides: Partial<ArtanisContinualLearningTemplateLedgerRecord> = {},
): ArtanisContinualLearningTemplateLedgerRecord =>
  S.decodeUnknownSync(ArtanisContinualLearningTemplateLedgerRecord)({
    ...exampleArtanisContinualLearningTemplateLedger(),
    ...overrides,
  })

const templateRecord = (
  overrides: Partial<ArtanisContinualLearningTemplateRecord> = {},
): ArtanisContinualLearningTemplateRecord =>
  S.decodeUnknownSync(ArtanisContinualLearningTemplateRecord)({
    ...exampleArtanisContinualLearningTemplateLedger().templates[0]!,
    ...overrides,
  })

describe('Artanis continual-learning templates', () => {
  test('projects all template kinds publicly and privately without execution authority', () => {
    const publicProjection = projectArtanisContinualLearningTemplates(
      exampleArtanisContinualLearningTemplateLedger(),
      'public_forum',
      nowIso,
    )
    const operatorProjection = projectArtanisContinualLearningTemplates(
      exampleArtanisContinualLearningTemplateLedger(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisContinualLearningTemplateLedgerProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      acceptedCount: 0,
      agentId: 'agent_artanis',
      authority: ARTANIS_CONTINUAL_LEARNING_NO_EXECUTION_AUTHORITY,
      audience: 'public_forum',
      blockedCount: 0,
      operatorReadyProposalCount: 7,
      proposedCount: 7,
      rejectedCount: 0,
      runningCount: 0,
      templateCount: 7,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(publicProjection.templates.map(template => template.kind)).toEqual(
      ARTANIS_CONTINUAL_LEARNING_TEMPLATE_KINDS,
    )
    expect(publicProjection.templates.every(
      template => template.operatorDetailRefs.length === 0,
    )).toBe(true)
    expect(operatorProjection.templates.every(
      template => template.operatorDetailRefs.length === 1,
    )).toBe(true)
    expect(artanisContinualLearningTemplateProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
  })

  test('requires every template to carry target, acceptance, evidence, cost, rollback, and approval refs', () => {
    const base = exampleArtanisContinualLearningTemplateLedger()

    for (const template of base.templates) {
      expect(template.benchmarkTargetRefs).not.toEqual([])
      expect(template.dispatchPayloadSchemaRefs).not.toEqual([])
      expect(template.acceptanceCriteriaRefs).not.toEqual([])
      expect(template.evidenceRefs).not.toEqual([])
      expect(template.requiredCapabilityRefs).not.toEqual([])
      expect(template.costCaveatRefs).not.toEqual([])
      expect(template.rollbackPostureRefs).not.toEqual([])
      expect(template.approvalRequirementRefs).not.toEqual([])
      expect(template.spendLimitRefs).not.toEqual([])
      expect(template.retainedFailureRefs).not.toEqual([])
      expect(template.modelArtifactRefs).not.toEqual([])
      expect(template.trainingRunRefs).not.toEqual([])
      expect(template.benchmarkCloudRefs).not.toEqual([])
      expect(template.promotionDecisionRefs).not.toEqual([])
      expect(template.publicReportRefs).not.toEqual([])
      expect(template.workloadRefs).not.toEqual([])
    }

    expect(() =>
      projectArtanisContinualLearningTemplates(
        ledgerRecord({
          templates: [templateRecord({ benchmarkTargetRefs: [] })],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisContinualLearningTemplateUnsafe)
  })

  test('blocks high-risk running templates without operator approval and executor authority', () => {
    const loraTemplate = templateRecord({
      kind: 'lora_finetuning_training',
      state: 'running',
      templateRef: 'template.public.artanis.continual_learning.lora_test',
    })

    expect(() =>
      projectArtanisContinualLearningTemplates(
        ledgerRecord({ templates: [loraTemplate] }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisContinualLearningTemplateUnsafe)

    const approved = projectArtanisContinualLearningTemplates(
      ledgerRecord({
        templates: [
          {
            ...loraTemplate,
            downstreamExecutorAuthorityRefs: [
              'authority.public.executor.model_lab_training_run',
            ],
            operatorApprovalRefs: [
              'approval.public.operator.model_lab_training_run',
            ],
          },
        ],
      }),
      'operator',
      nowIso,
    )

    expect(approved.templates[0]).toMatchObject({
      kind: 'lora_finetuning_training',
      state: 'running',
      templateExecutionAllowed: true,
    })
    expect(approved.authority).toEqual(
      ARTANIS_CONTINUAL_LEARNING_NO_EXECUTION_AUTHORITY,
    )
  })

  test('describes blocked, proposed, running, accepted, and rejected states for Forum summaries', () => {
    const templates = [
      templateRecord({
        blockerRefs: ['blocker.public.retained_failure_missing'],
        state: 'blocked',
        templateRef: 'template.public.artanis.continual_learning.blocked',
      }),
      templateRecord({
        state: 'proposed',
        templateRef: 'template.public.artanis.continual_learning.proposed',
      }),
      templateRecord({
        downstreamExecutorAuthorityRefs: ['authority.public.executor.eval_rerun'],
        operatorApprovalRefs: ['approval.public.operator.eval_rerun'],
        state: 'running',
        templateRef: 'template.public.artanis.continual_learning.running',
      }),
      templateRecord({
        downstreamExecutorAuthorityRefs: ['authority.public.executor.eval_rerun'],
        operatorApprovalRefs: ['approval.public.operator.eval_rerun'],
        state: 'accepted',
        templateRef: 'template.public.artanis.continual_learning.accepted',
      }),
      templateRecord({
        blockerRefs: ['blocker.public.regression_not_reproduced'],
        state: 'rejected',
        templateRef: 'template.public.artanis.continual_learning.rejected',
      }),
    ]
    const projection = projectArtanisContinualLearningTemplates(
      ledgerRecord({ templates }),
      'public_forum',
      nowIso,
    )

    expect(projection.templates.map(template => template.forumSummaryState))
      .toEqual(['blocked', 'proposed', 'running', 'accepted', 'rejected'])
    expect(JSON.stringify(projection)).not.toContain('private')
    expect(JSON.stringify(projection)).not.toContain('raw_prompt')
    expect(JSON.stringify(projection)).not.toContain('weights')
    expect(JSON.stringify(projection)).not.toContain('provider_payload')
    expect(JSON.stringify(projection)).not.toContain('provider.private')
    expect(JSON.stringify(projection)).not.toContain('provider.secret')
    expect(artanisContinualLearningTemplateProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('turns templates into Pylon marketplace intake and triage proposal payloads', () => {
    const templates = exampleArtanisContinualLearningTemplateLedger().templates
    const intakeKinds = templates.map(template =>
      pylonMarketplaceIntakeRequestFromTemplate(template, 'overnight_full')
        .jobKind
    )
    const triageRequest = pylonMarketplaceTriageRequestFromTemplate(
      templates[1]!,
      'balanced',
    )

    expect(intakeKinds).toEqual([
      'benchmark_evaluation',
      'gepa_dspy_optimization',
      'embedding_data_prep',
      'validation',
      'validation',
      'lora_finetuning',
      'artifact_review',
    ])
    expect(triageRequest).toMatchObject({
      assignment: {
        providerEligibilityRefs: expect.arrayContaining([
          'eligibility.public.provider.capability_snapshot_ok',
          'eligibility.public.model_lab_artifact_policy_ok',
        ]),
        resourceMode: 'balanced',
      },
      outcome: 'proposed_assignment',
    })
  })

  test('binds executor-trace replay to Tassadar capability, payload schemas, workload refs, and zero-spend caps', () => {
    const executorTemplate = exampleArtanisContinualLearningTemplateLedger()
      .templates.find(template => template.kind === 'executor_trace_replay')
    const intake = pylonMarketplaceIntakeRequestFromTemplate(
      executorTemplate!,
      'background_20',
    )
    const triage = pylonMarketplaceTriageRequestFromTemplate(
      executorTemplate!,
      'background_20',
    )

    expect(executorTemplate).toMatchObject({
      dispatchPayloadSchemaRefs: [
        'openagents.tassadar_executor_trace_request.v1',
        'openagents.tassadar_executor_trace_output.v1',
      ],
      requiredCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
      spendLimitRefs: [
        'spend_limit.public.tassadar_executor_trace.zero_sats_default',
      ],
      workloadRefs: ['workload.public.tassadar_poc.loop_sum_fixture'],
    })
    expect(intake).toMatchObject({
      dataRefs: expect.arrayContaining([
        'workload.public.tassadar_poc.loop_sum_fixture',
      ]),
      jobKind: 'validation',
      spendCaveatRefs: expect.arrayContaining([
        'spend_limit.public.tassadar_executor_trace.zero_sats_default',
      ]),
    })
    expect(triage.assignment!.providerEligibilityRefs).toContain(
      'eligibility.public.capability_tassadar_poc_numeric_model_executor',
    )
  })

  test('rejects raw prompts, private datasets, weights, provider payloads, private repos, customer data, and raw timestamps', () => {
    for (const unsafeTemplate of [
      templateRecord({ evidenceRefs: ['raw_prompt.customer_order'] }),
      templateRecord({ modelLabEvidenceRefs: ['dataset.private.training'] }),
      templateRecord({ modelArtifactRefs: ['weights.safetensors'] }),
      templateRecord({ sourceRefs: ['provider_payload.openai'] }),
      templateRecord({ sourceRefs: ['https://github.com/org/private-repo'] }),
      templateRecord({ retainedFailureRefs: ['customer_record.private'] }),
      templateRecord({ sourceRefs: ['source.public.2026-06-07T06:00:00Z'] }),
    ]) {
      expect(() =>
        projectArtanisContinualLearningTemplates(
          ledgerRecord({ templates: [unsafeTemplate] }),
          'operator',
          nowIso,
        ),
      ).toThrow(ArtanisContinualLearningTemplateUnsafe)
    }
  })
})
