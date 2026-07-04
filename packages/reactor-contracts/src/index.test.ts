import { describe, expect, test } from 'bun:test'
import { Schema as S } from 'effect'

import {
  REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF,
  REACTOR_AIRGAP_BUNDLE_VERIFIER_REF,
  REACTOR_EVAL_COVERAGE_MATRIX_SEED,
  REACTOR_EVAL_TASK_CLASS_REFS,
  REACTOR_EXAMPLE_POLICIES,
  REACTOR_HARDWARE_TIER_SPECS,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS,
  REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS,
  REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE,
  REACTOR_NEED_TO_KNOW_RULESET_V1,
  REACTOR_MODEL_EVAL_RECEIPT_SEED,
  REACTOR_MODEL_CATALOG_SEED,
  REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
  REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT,
  REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED,
  REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
  REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED,
  REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT,
  REACTOR_PSIONIC_EVAL_HARNESS_PROFILE,
  REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
  ReactorCapabilityCopyEvalDecision,
  ReactorAirgapUpdateBundleManifest,
  ReactorCorpusAccessDecisionReceipt,
  ReactorDogfoodRunReceipt,
  ReactorEvalCoverageMatrix,
  ReactorEvalHarnessProfile,
  ReactorInstallOpsReceipt,
  ReactorLocalTokenMeteringReceipt,
  ReactorModelInstallReceipt,
  ReactorModelCatalog,
  ReactorModelEvalReceipt,
  type ReactorModelCatalog as ReactorModelCatalogType,
  type ReactorModelProvenance,
  ReactorModelPolicyDecisionReceipt,
  ReactorNeedToKnowRuleSet,
  ReactorNodeModelProfile,
  buildReactorAirgapUpdateBundleManifest,
  buildReactorDogfoodRunReceipt,
  buildReactorEvalCoverageMatrix,
  buildReactorInstallOpsReceipt,
  buildReactorLocalTokenMeteringReceipt,
  buildReactorModelEvalReceipt,
  evaluateReactorNeedToKnowAccess,
  provisionReactorModel,
  routeReactorOpenAiCompatibleRequest,
  selectReactorCapabilityCopyEvalRefs,
  resolveReactorModelPolicy,
} from './index'

const DECIDED_AT = '2026-07-04T12:00:00.000Z'

const decision = (
  policy: keyof typeof REACTOR_EXAMPLE_POLICIES,
  catalog: ReactorModelCatalogType = REACTOR_MODEL_CATALOG_SEED,
) =>
  resolveReactorModelPolicy({
    catalog,
    decidedAt: DECIDED_AT,
    decisionRef: `reactor.policy_decision.${policy}.001`,
    policy: REACTOR_EXAMPLE_POLICIES[policy],
    sourceRefs: ['test:reactor-contracts'],
  })

const byRef = new Map(
  REACTOR_MODEL_CATALOG_SEED.models.map(model => [model.modelRef, model]),
)

describe('Reactor model provenance catalog', () => {
  test('decodes the curated seed with honest partial and unknown disclosures', () => {
    const catalog = S.decodeUnknownSync(ReactorModelCatalog)(
      REACTOR_MODEL_CATALOG_SEED,
    )

    expect(catalog.models.map(model => model.family)).toEqual([
      'nemotron',
      'llama',
      'gpt-oss',
      'gemma',
      'mistral',
      'qwen',
      'deepseek',
      'kimi',
      'glm',
    ])
    expect(catalog.models.some(model => model.trainingDataDisclosure === 'unknown')).toBe(true)
    expect(catalog.models.some(model => model.trainingDataDisclosure === 'partial')).toBe(true)
    expect(
      catalog.models.find(
        model => model.modelRef === 'model.openai.gpt_oss.open_family',
      )?.evalRefs,
    ).toEqual([
      'reactor.eval_receipt.gpt_oss.drafting.fixture.20260704',
      'reactor.eval_receipt.gpt_oss.extraction.fixture.20260704',
    ])
    expect(
      catalog.models.find(
        model => model.modelRef === 'model.meta.llama.open_family',
      )?.evalRefs,
    ).toEqual([
      'reactor.eval_receipt.llama.drafting.fixture.20260704',
      'reactor.eval_receipt.llama.extraction.fixture.20260704',
    ])
    expect(
      catalog.models
        .filter(
          model =>
            model.modelRef !== 'model.openai.gpt_oss.open_family' &&
            model.modelRef !== 'model.meta.llama.open_family',
        )
        .every(model => model.evalRefs.length === 0),
    ).toBe(true)
  })

  test('policy decisions are receipt-shaped and name the policy version', () => {
    const receipt = decision('unconstrained')
    const decoded = S.decodeUnknownSync(ReactorModelPolicyDecisionReceipt)(
      receipt,
    )

    expect(decoded.schemaVersion).toBe(
      'openagents.reactor.model_policy_decision.v1',
    )
    expect(decoded.policyRef).toBe(
      REACTOR_EXAMPLE_POLICIES.unconstrained.policyRef,
    )
    expect(decoded.policyVersion).toBe(
      REACTOR_EXAMPLE_POLICIES.unconstrained.version,
    )
    expect(decoded.decidedAt).toBe(DECIDED_AT)
    expect(decoded.sourceRefs).toContain(REACTOR_MODEL_CATALOG_SEED.catalogRef)
  })
})

describe('Reactor task-class eval receipts', () => {
  test('declares a Psionic-owned harness profile for the four Reactor task classes', () => {
    const profile = S.decodeUnknownSync(ReactorEvalHarnessProfile)(
      REACTOR_PSIONIC_EVAL_HARNESS_PROFILE,
    )

    expect(profile.runnerOwner).toBe('psionic')
    expect(profile.taskClassRefs).toEqual([...REACTOR_EVAL_TASK_CLASS_REFS])
    expect(profile.supportedExecutionTargets).toEqual([
      'rx3_served_model',
      'hosted_equivalent_large_model',
      'not_measured',
    ])
    expect(profile.unrunMeasurementState).toBe('not_measured')
  })

  test('ships measured receipts for two models across two task classes', () => {
    const receipts = REACTOR_MODEL_EVAL_RECEIPT_SEED.map(receipt =>
      S.decodeUnknownSync(ReactorModelEvalReceipt)(receipt),
    )
    const byModel = new Map(
      REACTOR_MODEL_CATALOG_SEED.models.map(model => [
        model.modelRef,
        model.evalRefs,
      ]),
    )

    expect(receipts).toHaveLength(4)
    expect(
      receipts.map(receipt => [
        receipt.modelRef,
        receipt.taskClassRef,
        receipt.executionTarget,
      ]),
    ).toEqual([
      [
        'model.openai.gpt_oss.open_family',
        'drafting',
        'rx3_served_model',
      ],
      [
        'model.openai.gpt_oss.open_family',
        'extraction',
        'rx3_served_model',
      ],
      [
        'model.meta.llama.open_family',
        'drafting',
        'hosted_equivalent_large_model',
      ],
      [
        'model.meta.llama.open_family',
        'extraction',
        'hosted_equivalent_large_model',
      ],
    ])
    expect(receipts.every(receipt => receipt.measurementState === 'measured')).toBe(true)
    expect(receipts.every(receipt => receipt.capabilityCopyAllowed)).toBe(true)
    for (const receipt of receipts) {
      expect(receipt.score).not.toBeNull()
      expect(receipt.sampleCount).toBeGreaterThan(0)
      expect(byModel.get(receipt.modelRef)).toContain(receipt.receiptRef)
    }
  })

  test('coverage matrix marks unrun combinations not_measured rather than zero', () => {
    const matrix = S.decodeUnknownSync(ReactorEvalCoverageMatrix)(
      REACTOR_EVAL_COVERAGE_MATRIX_SEED,
    )
    const measuredCells = matrix.cells.filter(
      cell => cell.measurementState === 'measured',
    )
    const notMeasuredCells = matrix.cells.filter(
      cell => cell.measurementState === 'not_measured',
    )
    const qwenAgentToolUseCell = matrix.cells.find(
      cell =>
        cell.modelRef === 'model.alibaba.qwen.open_family' &&
        cell.taskClassRef === 'agent_tool_use',
    )

    expect(matrix.cells).toHaveLength(
      REACTOR_MODEL_CATALOG_SEED.models.length *
        REACTOR_EVAL_TASK_CLASS_REFS.length,
    )
    expect(measuredCells).toHaveLength(4)
    expect(notMeasuredCells.length).toBe(matrix.cells.length - 4)
    expect(qwenAgentToolUseCell).toMatchObject({
      capabilityCopyAllowed: false,
      measurementState: 'not_measured',
      receiptRef: null,
      score: null,
    })
    expect(qwenAgentToolUseCell?.blockerRefs).toContain(
      'blocker.reactor.eval.not_measured',
    )
    expect(notMeasuredCells.every(cell => cell.score === null)).toBe(true)
  })

  test('capability copy decisions only return measured eval receipt refs', () => {
    const allowed = selectReactorCapabilityCopyEvalRefs({
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.capability_copy.gpt_oss.draft_extract.001',
      evalReceipts: REACTOR_MODEL_EVAL_RECEIPT_SEED,
      modelRef: 'model.openai.gpt_oss.open_family',
      taskClassRefs: ['drafting', 'extraction'],
    })
    const blocked = selectReactorCapabilityCopyEvalRefs({
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.capability_copy.gpt_oss.agent_tool_use.001',
      evalReceipts: REACTOR_MODEL_EVAL_RECEIPT_SEED,
      modelRef: 'model.openai.gpt_oss.open_family',
      taskClassRefs: ['drafting', 'agent_tool_use'],
    })

    expect(
      S.decodeUnknownSync(ReactorCapabilityCopyEvalDecision)(allowed),
    ).toMatchObject({
      allowedEvalRefs: [
        'reactor.eval_receipt.gpt_oss.drafting.fixture.20260704',
        'reactor.eval_receipt.gpt_oss.extraction.fixture.20260704',
      ],
      blockerRefs: [],
      status: 'allowed',
    })
    expect(blocked.status).toBe('blocked_not_measured')
    expect(blocked.allowedEvalRefs).toEqual([
      'reactor.eval_receipt.gpt_oss.drafting.fixture.20260704',
    ])
    expect(blocked.blockerRefs).toContain(
      'blocker.reactor.capability_copy.eval_not_measured',
    )
    expect(blocked.blockerRefs).toContain('task_class:agent_tool_use')
  })

  test('eval builders reject invalid measured scores and duplicate matrix cells', () => {
    expect(() =>
      buildReactorModelEvalReceipt({
        catalog: REACTOR_MODEL_CATALOG_SEED,
        generatedAt: DECIDED_AT,
        measurement: {
          evalDatasetRef: 'dataset.reactor.fixture.drafting.v1',
          executionTarget: 'rx3_served_model',
          sampleCount: 0,
          score: 0.7,
          scoreUnit: 'score_0_to_1',
          state: 'measured',
        },
        modelRef: 'model.openai.gpt_oss.open_family',
        receiptRef: 'reactor.eval_receipt.invalid.001',
        taskClassRef: 'drafting',
      }),
    ).toThrow('reactor.eval.measured_receipt_invalid_score_or_samples')

    expect(() =>
      buildReactorEvalCoverageMatrix({
        catalog: REACTOR_MODEL_CATALOG_SEED,
        evalReceipts: [
          REACTOR_MODEL_EVAL_RECEIPT_SEED[0],
          {
            ...REACTOR_MODEL_EVAL_RECEIPT_SEED[0],
            receiptRef: 'reactor.eval_receipt.duplicate.001',
          },
        ],
        generatedAt: DECIDED_AT,
        matrixRef: 'reactor.eval_matrix.duplicate.001',
      }),
    ).toThrow('reactor.eval.duplicate_measured_receipt_for_model_task')
  })
})

describe('Reactor policy resolver examples', () => {
  test('US-only policy admits only US-origin models and checks lineage', () => {
    const receipt = decision('usOnly')

    expect(receipt.status).toBe('conforming_models')
    expect(receipt.conformingModelRefs).toEqual([
      'model.nvidia.nemotron.open_family',
      'model.meta.llama.open_family',
      'model.openai.gpt_oss.open_family',
      'model.google.gemma.open_family',
    ])
    for (const modelRef of receipt.conformingModelRefs) {
      const model = byRef.get(modelRef)
      expect(model?.originJurisdiction).toBe('us')
      expect(
        model?.distillationLineage.every(
          lineage => lineage.originJurisdiction === 'us',
        ),
      ).toBe(true)
    }
  })

  test('no-cn policy excludes Chinese-origin models and Chinese-origin lineage', () => {
    const receipt = decision('noCn')

    expect(receipt.conformingModelRefs).toEqual([
      'model.nvidia.nemotron.open_family',
      'model.meta.llama.open_family',
      'model.openai.gpt_oss.open_family',
      'model.google.gemma.open_family',
      'model.mistral.magistral_open_family',
    ])
    for (const modelRef of receipt.conformingModelRefs) {
      const model = byRef.get(modelRef)
      expect(model?.originJurisdiction).not.toBe('cn')
      expect(
        model?.distillationLineage.some(
          lineage => lineage.originJurisdiction === 'cn',
        ),
      ).toBe(false)
    }
  })

  test('permissive-license policy admits only permissive license-class records', () => {
    const receipt = decision('permissiveLicenseOnly')

    expect(receipt.conformingModelRefs).toEqual([
      'model.openai.gpt_oss.open_family',
    ])
    for (const modelRef of receipt.conformingModelRefs) {
      expect(byRef.get(modelRef)?.licenseClass).toBe('permissive')
    }
  })

  test('unconstrained policy admits the entire seed catalog', () => {
    const receipt = decision('unconstrained')

    expect(receipt.status).toBe('conforming_models')
    expect(receipt.conformingModelRefs).toEqual(
      REACTOR_MODEL_CATALOG_SEED.models.map(model => model.modelRef),
    )
    expect(receipt.refusal).toBeNull()
  })
})

describe('Reactor lineage and refusal behavior', () => {
  test('a US-label model with restricted-origin lineage fails strict US-only policy', () => {
    const disguisedModel: ReactorModelProvenance = {
      schemaVersion: 'openagents.model_provenance.v1',
      developer: 'fixture-lab',
      displayName: 'Fixture US-label CN-distilled model',
      distillationLineage: [
        {
          developer: 'alibaba',
          disclosure: 'documented',
          modelRef: 'model.alibaba.qwen.open_family',
          originJurisdiction: 'cn',
          sourceRefs: ['test:cn_lineage_fixture'],
        },
      ],
      evalRefs: [],
      family: 'fixture',
      license: 'apache-2.0',
      licenseClass: 'permissive',
      modelRef: 'model.fixture.us_label_cn_lineage',
      originJurisdiction: 'us',
      sourceRefs: ['test:reactor_lineage_fixture'],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights',
    }
    const catalog: ReactorModelCatalogType = {
      ...REACTOR_MODEL_CATALOG_SEED,
      models: [...REACTOR_MODEL_CATALOG_SEED.models, disguisedModel],
    }
    const receipt = decision('usOnly', catalog)
    const refused = receipt.modelDecisions.find(
      model => model.modelRef === disguisedModel.modelRef,
    )

    expect(receipt.conformingModelRefs).not.toContain(disguisedModel.modelRef)
    expect(refused).toMatchObject({
      status: 'refused',
      reasonRefs: ['reactor.policy.lineage_origin_not_allowed'],
    })
  })

  test('empty conforming set emits a refusal object naming the policy version', () => {
    const receipt = resolveReactorModelPolicy({
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.policy_decision.empty.001',
      policy: {
        ...REACTOR_EXAMPLE_POLICIES.usOnly,
        constraints: {
          allowDevelopers: ['developer.none'],
          allowOriginJurisdictions: ['us'],
          enforceDistillationLineageJurisdiction: true,
        },
        policyRef: 'reactor.model_policy.example.empty.v1',
        version: '2026-07-04.empty',
      },
    })

    expect(receipt.status).toBe('refused_empty_set')
    expect(receipt.conformingModelRefs).toEqual([])
    expect(receipt.refusal).toMatchObject({
      blockerRefs: expect.arrayContaining([
        'blocker.reactor.model_policy.no_conforming_models',
        'policy:reactor.model_policy.example.empty.v1',
        'policy_version:2026-07-04.empty',
      ]),
      reason: 'no_conforming_models',
    })
  })
})

describe('Reactor serving skeleton', () => {
  test('declares one server-class Hydralisk profile behind an offline OpenAI-compatible gateway', () => {
    const profile = S.decodeUnknownSync(ReactorNodeModelProfile)(
      REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
    )

    expect(profile.servingLane).toBe('hydralisk')
    expect(profile.servingStack.kind).toBe('vllm')
    expect(profile.gateway.protocol).toBe('openai.chat_completions.v1')
    expect(profile.gateway.servingPathNetwork).toBe('offline_once_provisioned')
    expect(profile.gateway.phoneHomeAllowedInServingPath).toBe(false)
    expect(profile.exactLocalMeteringRequired).toBe(true)
    expect(profile.policyVersion).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.version)
  })

  test('provisions a conforming model with a receipt naming the policy version', () => {
    const receipt = provisionReactorModel({
      action: 'install',
      artifactRefs: ['artifact.fixture.gpt_oss.open_family.weights'],
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.policy_decision.install.gpt_oss.001',
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_receipt.gpt_oss.001',
      sourceRefs: ['test:reactor-serving-skeleton'],
    })
    const decoded = S.decodeUnknownSync(ReactorModelInstallReceipt)(receipt)

    expect(decoded.status).toBe('installed')
    expect(decoded.weightsPullAuthorization).toBe('authorized')
    expect(decoded.policyRef).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.policyRef)
    expect(decoded.policyVersion).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.version)
    expect(decoded.refusal).toBeNull()
  })

  test('provisioning refuses nonconforming weights before pull even with a bypass-like env var set', () => {
    const previous = process.env.REACTOR_MODEL_POLICY_BYPASS
    process.env.REACTOR_MODEL_POLICY_BYPASS = '1'
    const qwenProfile = S.decodeUnknownSync(ReactorNodeModelProfile)({
      ...REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      modelRef: 'model.alibaba.qwen.open_family',
      nodeProfileRef: 'reactor.node_profile.fixture.hydralisk.qwen.v1',
      servingStack: {
        ...REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.servingStack,
        modelArtifactRefs: ['artifact.fixture.qwen.open_family.weights'],
      },
    })

    try {
      const receipt = provisionReactorModel({
        action: 'install',
        artifactRefs: ['artifact.fixture.qwen.open_family.weights'],
        catalog: REACTOR_MODEL_CATALOG_SEED,
        decidedAt: DECIDED_AT,
        decisionRef: 'reactor.policy_decision.install.qwen.001',
        nodeProfile: qwenProfile,
        policy: REACTOR_EXAMPLE_POLICIES.usOnly,
        receiptRef: 'reactor.install_receipt.qwen.001',
      })

      expect(receipt.status).toBe('refused_policy')
      expect(receipt.weightsPullAuthorization).toBe('refused_before_pull')
      expect(receipt.refusal?.blockerRefs).toContain(
        'blocker.reactor.provision.policy_nonconforming_model',
      )
      expect(receipt.refusal?.blockerRefs).toContain(
        'reactor.policy.origin_not_allowed',
      )
    } finally {
      if (previous === undefined) {
        delete process.env.REACTOR_MODEL_POLICY_BYPASS
      } else {
        process.env.REACTOR_MODEL_POLICY_BYPASS = previous
      }
    }
  })

  test('router routes only an installed conforming model through the OpenAI-compatible gateway', () => {
    const installReceipt = provisionReactorModel({
      action: 'install',
      artifactRefs: ['artifact.fixture.gpt_oss.open_family.weights'],
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.policy_decision.install.route.001',
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_receipt.route.001',
    })
    const routeReceipt = routeReactorOpenAiCompatibleRequest({
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.route_decision.gpt_oss.001',
      installReceipt,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      requestRef: 'reactor.request.openai_chat.001',
      requestedModelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
    })

    expect(routeReceipt.status).toBe('routed')
    expect(routeReceipt.routedModelRef).toBe(
      REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
    )
    expect(routeReceipt.gatewayProtocol).toBe('openai.chat_completions.v1')
    expect(routeReceipt.servingPathNetwork).toBe('offline_once_provisioned')
    expect(routeReceipt.blockerRefs).toEqual([])
  })

  test('router recomputes policy and refuses a forged installed receipt for a nonconforming model', () => {
    const qwenProfile = S.decodeUnknownSync(ReactorNodeModelProfile)({
      ...REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      modelRef: 'model.alibaba.qwen.open_family',
      nodeProfileRef: 'reactor.node_profile.fixture.hydralisk.qwen.forged.v1',
    })
    const forgedInstalledReceipt = S.decodeUnknownSync(ReactorModelInstallReceipt)({
      schemaVersion: 'openagents.reactor.model_install_receipt.v1',
      action: 'install',
      artifactRefs: ['artifact.fixture.qwen.open_family.weights'],
      catalogRef: REACTOR_MODEL_CATALOG_SEED.catalogRef,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.policy_decision.forged.qwen.001',
      modelRef: qwenProfile.modelRef,
      nodeProfileRef: qwenProfile.nodeProfileRef,
      policyDecisionRef: 'reactor.policy_decision.forged.qwen.001',
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      receiptRef: 'reactor.install_receipt.forged.qwen.001',
      refusal: null,
      servingLane: 'hydralisk',
      sourceRefs: ['test:forged-install-receipt'],
      status: 'installed',
      weightsPullAuthorization: 'authorized',
    })

    const routeReceipt = routeReactorOpenAiCompatibleRequest({
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      decisionRef: 'reactor.route_decision.forged.qwen.001',
      installReceipt: forgedInstalledReceipt,
      nodeProfile: qwenProfile,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      requestRef: 'reactor.request.openai_chat.forged.qwen.001',
      requestedModelRef: qwenProfile.modelRef,
    })

    expect(routeReceipt.status).toBe('refused_policy')
    expect(routeReceipt.routedModelRef).toBeNull()
    expect(routeReceipt.blockerRefs).toContain(
      'blocker.reactor.router.policy_nonconforming_model',
    )
    expect(routeReceipt.blockerRefs).toContain('reactor.policy.origin_not_allowed')
  })

  test('local token metering records exact reconciled rows and marks unknowns not_measured', () => {
    const exactReceipt = buildReactorLocalTokenMeteringReceipt({
      generatedAt: DECIDED_AT,
      modelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      receiptRef: 'reactor.local_metering.exact.001',
      requestRef: 'reactor.request.openai_chat.metered.001',
      usage: {
        completionTokens: 8,
        promptTokens: 13,
        state: 'exact',
        totalTokens: 21,
      },
    })

    expect(exactReceipt.measurementState).toBe('measured')
    expect(exactReceipt.usageTruth).toBe('exact')
    expect(exactReceipt.totalTokens).toBe(21)
    expect(exactReceipt.localOnly).toBe(true)

    expect(() =>
      buildReactorLocalTokenMeteringReceipt({
        generatedAt: DECIDED_AT,
        modelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
        nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
        policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
        policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
        receiptRef: 'reactor.local_metering.bad_total.001',
        requestRef: 'reactor.request.openai_chat.bad_total.001',
        usage: {
          completionTokens: 8,
          promptTokens: 13,
          state: 'exact',
          totalTokens: 20,
        },
      }),
    ).toThrow('reactor.local_metering.exact_counts_do_not_reconcile')

    const notMeasuredReceipt = buildReactorLocalTokenMeteringReceipt({
      generatedAt: DECIDED_AT,
      modelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      receiptRef: 'reactor.local_metering.not_measured.001',
      requestRef: 'reactor.request.openai_chat.not_measured.001',
      usage: {
        reasonRef: 'reason.fixture.counter_unavailable',
        state: 'not_measured',
      },
    })

    expect(notMeasuredReceipt.measurementState).toBe('not_measured')
    expect(notMeasuredReceipt.usageTruth).toBe('not_measured')
    expect(notMeasuredReceipt.totalTokens).toBeNull()
    expect(notMeasuredReceipt.blockerRefs).toContain(
      'blocker.reactor.local_metering.not_measured',
    )
  })

  test('metering schema rejects estimated token usage labels', () => {
    expect(() =>
      S.decodeUnknownSync(ReactorLocalTokenMeteringReceipt)({
        schemaVersion: 'openagents.reactor.local_token_metering_receipt.v1',
        blockerRefs: [],
        completionTokens: 8,
        generatedAt: DECIDED_AT,
        localOnly: true,
        measurementState: 'measured',
        modelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
        nodeProfileRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.nodeProfileRef,
        policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
        policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
        promptTokens: 13,
        receiptRef: 'reactor.local_metering.estimated.001',
        requestRef: 'reactor.request.openai_chat.estimated.001',
        servingLane: 'hydralisk',
        sourceRefs: [],
        totalTokens: 21,
        usageTruth: 'estimated',
      }),
    ).toThrow()
  })
})

describe('Reactor install and air-gap update receipts', () => {
  const bundle = () =>
    buildReactorAirgapUpdateBundleManifest({
      artifactSha256:
        '8f6d8d6c7c2f0f4f0e2a111111111111111111111111111111111111111111111',
      bundleRef: 'reactor.airgap_bundle.fixture.gpt_oss.20260704',
      bundleVersion: '2026-07-04.rx5.fixture',
      createdAt: DECIDED_AT,
      modelRef: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE.modelRef,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      signatureKid: '2dbe811d19f67528',
      signatureRef: 'reactor.airgap_bundle.fixture.gpt_oss.20260704.sig',
      sourceRefs: ['test:reactor-install-airgap'],
    })

  test('air-gap bundle manifests reuse the OpenAgents release verifier and require no callbacks', () => {
    const manifest = S.decodeUnknownSync(ReactorAirgapUpdateBundleManifest)(
      bundle(),
    )

    expect(manifest.callbackRequired).toBe(false)
    expect(manifest.signatureAlg).toBe('ed25519')
    expect(manifest.verifierRef).toBe(REACTOR_AIRGAP_BUNDLE_VERIFIER_REF)
    expect(manifest.publicKeyRef).toBe(REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF)
    expect(manifest.policyRef).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.policyRef)
    expect(manifest.policyVersion).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.version)
  })

  test('fresh install, upgrade, and rollback receipts revalidate model policy', () => {
    const updateBundle = bundle()
    const freshInstall = buildReactorInstallOpsReceipt({
      action: 'fresh_install',
      bundle: updateBundle,
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_ops.fresh.fixture.001',
    })
    const upgrade = buildReactorInstallOpsReceipt({
      action: 'upgrade',
      bundle: updateBundle,
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_ops.upgrade.fixture.001',
    })
    const rollback = buildReactorInstallOpsReceipt({
      action: 'rollback',
      bundle: updateBundle,
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      nodeProfile: REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_ops.rollback.fixture.001',
      rollbackFromBundleRef: 'reactor.airgap_bundle.fixture.gpt_oss.20260704.bad',
    })

    for (const receipt of [freshInstall, upgrade, rollback]) {
      const decoded = S.decodeUnknownSync(ReactorInstallOpsReceipt)(receipt)
      expect(decoded.status).toBe('succeeded')
      expect(decoded.blockerRefs).toEqual([])
      expect(decoded.policyRef).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.policyRef)
      expect(decoded.policyVersion).toBe(REACTOR_EXAMPLE_POLICIES.usOnly.version)
      expect(decoded.verificationRefs).toContain(REACTOR_AIRGAP_BUNDLE_VERIFIER_REF)
      expect(decoded.verificationRefs).toContain(
        REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF,
      )
    }
    expect(rollback.rollbackFromBundleRef).toBe(
      'reactor.airgap_bundle.fixture.gpt_oss.20260704.bad',
    )
    expect(rollback.rollbackToBundleRef).toBe(updateBundle.bundleRef)
  })

  test('model refresh refuses when policy revalidation fails', () => {
    const qwenProfile = S.decodeUnknownSync(ReactorNodeModelProfile)({
      ...REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
      modelRef: 'model.alibaba.qwen.open_family',
      nodeProfileRef: 'reactor.node_profile.fixture.hydralisk.qwen.rx5.v1',
    })
    const qwenBundle = buildReactorAirgapUpdateBundleManifest({
      artifactSha256:
        '1f6d8d6c7c2f0f4f0e2a111111111111111111111111111111111111111111111',
      bundleRef: 'reactor.airgap_bundle.fixture.qwen.20260704',
      bundleVersion: '2026-07-04.rx5.qwen',
      createdAt: DECIDED_AT,
      modelRef: qwenProfile.modelRef,
      nodeProfile: qwenProfile,
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      signatureKid: '2dbe811d19f67528',
      signatureRef: 'reactor.airgap_bundle.fixture.qwen.20260704.sig',
    })
    const receipt = buildReactorInstallOpsReceipt({
      action: 'upgrade',
      bundle: qwenBundle,
      catalog: REACTOR_MODEL_CATALOG_SEED,
      decidedAt: DECIDED_AT,
      nodeProfile: qwenProfile,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.install_ops.upgrade.qwen.refused.001',
    })

    expect(receipt.status).toBe('refused')
    expect(receipt.blockerRefs).toContain(
      'blocker.reactor.install_ops.policy_revalidation_failed',
    )
    expect(receipt.blockerRefs).toContain(
      'blocker.reactor.provision.policy_nonconforming_model',
    )
    expect(receipt.blockerRefs).toContain('reactor.policy.origin_not_allowed')
  })

  test('hardware tier specs are guidance only and not purchase commitments', () => {
    expect(REACTOR_HARDWARE_TIER_SPECS.map(spec => spec.tierRef)).toEqual([
      'workstation',
      'server',
      'rack',
    ])
    expect(REACTOR_HARDWARE_TIER_SPECS.every(spec => spec.guidanceOnly)).toBe(true)
    expect(
      REACTOR_HARDWARE_TIER_SPECS.flatMap(spec => spec.notes).join('\n'),
    ).toContain('no purchase commitment')
  })
})

describe('Reactor dogfood run receipts', () => {
  test('records OpenAgents as customer number one under a strict US-only policy', () => {
    const receipt = S.decodeUnknownSync(ReactorDogfoodRunReceipt)(
      REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT,
    )

    expect(receipt.status).toBe('completed')
    expect(receipt.blockerRefs).toEqual([])
    expect(receipt.hardwareOwnerRef).toBe('owner.openagents')
    expect(receipt.placement).toBe('dogfood')
    expect(receipt.workloadTruth).toBe('internal_openagents')
    expect(receipt.workloadClass).toBe('internal_lead_gen_case_study_seed')
    expect(receipt.caseStudyWriteupRef).toBe(
      'docs/fable/2026-07-04-rx-6-reactor-dogfood-run.md',
    )
    expect(receipt.policyConstraintRefs).toContain(
      'constraint.origin_jurisdiction:us',
    )
    expect(receipt.policyConstraintRefs).toContain(
      'constraint.distillation_lineage_jurisdiction:enforced',
    )
    expect(receipt.installOpsReceiptRef).toBe(
      REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT.receiptRef,
    )
    expect(receipt.routeDecisionRefs).toEqual(
      REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED.map(
        route => route.decisionRef,
      ),
    )
    expect(receipt.localMeteringReceiptRefs).toEqual(
      REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED.map(
        metering => metering.receiptRef,
      ),
    )
    expect(receipt.totalMeasuredTokens).toBe(743)
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.externalPilotAuthorized).toBe(false)
    expect(receipt.externalClaimFlipAllowed).toBe(false)
  })

  test('ties the deliberate nonconforming pull attempt to a policy refusal', () => {
    const refused = S.decodeUnknownSync(ReactorInstallOpsReceipt)(
      REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
    )
    const receipt = REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT

    expect(refused.status).toBe('refused')
    expect(refused.modelRef).toBe('model.alibaba.qwen.open_family')
    expect(refused.blockerRefs).toContain(
      'blocker.reactor.install_ops.policy_revalidation_failed',
    )
    expect(refused.blockerRefs).toContain('reactor.policy.origin_not_allowed')
    expect(receipt.refusedNonconformingInstallOpsReceiptRef).toBe(
      refused.receiptRef,
    )
    expect(receipt.refusedNonconformingModelRef).toBe(refused.modelRef)
  })

  test('refuses dogfood completion when exact local metering is missing', () => {
    const unmeasured = buildReactorLocalTokenMeteringReceipt({
      generatedAt: DECIDED_AT,
      modelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
      nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
      policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
      policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
      receiptRef: 'reactor.local_metering.openagents.dogfood.not_measured.001',
      requestRef:
        REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED[0]?.requestRef ??
        'reactor.request.openagents.dogfood.lead_gen.discovery.20260704',
      usage: {
        reasonRef: 'reason.test.counter_unavailable',
        state: 'not_measured',
      },
    })

    const receipt = buildReactorDogfoodRunReceipt({
      caseStudyWriteupRef:
        'docs/fable/2026-07-04-rx-6-reactor-dogfood-run.md',
      generatedAt: DECIDED_AT,
      installOpsReceipt: REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT,
      measuredWindowEndedAt: '2026-07-04T14:15:00.000Z',
      measuredWindowStartedAt: '2026-07-04T14:10:00.000Z',
      meteringReceipts: [unmeasured],
      nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
      policy: REACTOR_EXAMPLE_POLICIES.usOnly,
      receiptRef: 'reactor.dogfood_run.openagents.not_measured.001',
      refusedNonconformingInstallOpsReceipt:
        REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
      routeReceipts: [REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED[0]],
      workloadRef: 'workload.openagents.lead_gen_reactor.case_study_seed.20260704',
    })

    expect(receipt.status).toBe('refused')
    expect(receipt.blockerRefs).toContain(
      'blocker.reactor.dogfood.local_metering_not_exact',
    )
  })
})

describe('Reactor need-to-know corpus access', () => {
  test('allows Alice direct source retrieval only after hard rules and oracle pass', () => {
    const receipt = S.decodeUnknownSync(ReactorCorpusAccessDecisionReceipt)(
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.aliceAllowed,
    )

    expect(receipt).toMatchObject({
      schemaVersion: 'openagents.reactor.corpus_access_decision_receipt.v1',
      matterRef: 'matter.reactor.fixture.alice',
      oracleAppliedAfterHardRules: true,
      outputMode: 'source',
      rawDocumentContentLogged: false,
      generatedSummaryContentLogged: false,
      ruleSetRef: REACTOR_NEED_TO_KNOW_RULESET_V1.ruleSetRef,
      ruleSetVersion: REACTOR_NEED_TO_KNOW_RULESET_V1.version,
      subjectUserRef: 'user.alice',
      workspaceRef: 'workspace.reactor.fixture.customer_one',
    })
    expect(receipt.selectedDocumentRefs).toEqual([
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ])
    expect(receipt.selectedCitationRefs).toEqual([
      'citation.reactor.fixture.alice.strategy_memo',
    ])
    expect(receipt.deniedDocumentRefs).toEqual([])
    expect(receipt.documentDecisions[0]).toMatchObject({
      blockerRefs: [],
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'passed',
      oracleVerdictRef:
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.alicePlausible.verdictRef,
      status: 'allowed',
    })
  })

  test('denies Bob access to Alice citations and summaries at the hard layer', () => {
    const citationDenied =
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.bobAliceCitationDenied
    const summaryDenied =
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.bobAliceSummaryDenied

    for (const receipt of [citationDenied, summaryDenied]) {
      expect(receipt.selectedDocumentRefs).toEqual([])
      expect(receipt.selectedCitationRefs).toEqual([])
      expect(receipt.deniedDocumentRefs).toEqual([
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
      ])
      expect(receipt.deniedCitationRefs).toEqual([
        'citation.reactor.fixture.alice.strategy_memo',
      ])
      expect(receipt.rawDocumentContentLogged).toBe(false)
      expect(receipt.generatedSummaryContentLogged).toBe(false)
      expect(receipt.documentDecisions[0]).toMatchObject({
        hardDecisionStatus: 'failed',
        oracleDecisionStatus: 'skipped_hard_denied',
        status: 'denied_hard_rule',
      })
      expect(receipt.documentDecisions[0]?.blockerRefs).toEqual(
        expect.arrayContaining([
          'blocker.reactor.need_to_know.matter_scope_mismatch',
          'blocker.reactor.need_to_know.role_or_user_scope_missing',
        ]),
      )
      expect(JSON.stringify(receipt)).not.toContain('Alice private strategy text')
    }

    expect(summaryDenied.documentDecisions[0]?.oracleVerdictRef).toBe(
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.bobAlicePlausibleButHardDenied
        .verdictRef,
    )
  })

  test('soft oracle can deny after hard rules pass', () => {
    const receipt = REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS.aliceSoftDenied

    expect(receipt.selectedDocumentRefs).toEqual([])
    expect(receipt.selectedCitationRefs).toEqual([])
    expect(receipt.deniedDocumentRefs).toEqual([
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ])
    expect(receipt.documentDecisions[0]).toMatchObject({
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'failed',
      oracleVerdictRef:
        REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.aliceNotNeeded.verdictRef,
      status: 'denied_soft_oracle',
    })
    expect(receipt.documentDecisions[0]?.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.reactor.need_to_know.oracle_not_plausible',
        'reason.reactor.need_to_know.alice_summary_not_needed',
      ]),
    )
  })

  test('fails closed when a hard-allowed request lacks an oracle verdict', () => {
    const receipt = evaluateReactorNeedToKnowAccess({
      decidedAt: DECIDED_AT,
      documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
      receiptRef: 'reactor.corpus_access.alice.missing_oracle.denied.001',
      request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceDirect,
      ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
    })

    expect(receipt.selectedDocumentRefs).toEqual([])
    expect(receipt.documentDecisions[0]).toMatchObject({
      blockerRefs: ['blocker.reactor.need_to_know.oracle_verdict_missing'],
      hardDecisionStatus: 'passed',
      oracleDecisionStatus: 'failed',
      status: 'denied_soft_oracle',
    })
  })

  test('rejects broken allow-all rule fixtures before evaluation', () => {
    expect(() =>
      S.decodeUnknownSync(ReactorNeedToKnowRuleSet)(
        REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE,
      ),
    ).toThrow()
    expect(() =>
      evaluateReactorNeedToKnowAccess({
        decidedAt: DECIDED_AT,
        documents: [
          REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo,
        ],
        oracleVerdicts: [
          REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.bobAlicePlausibleButHardDenied,
        ],
        receiptRef: 'reactor.corpus_access.broken_allow_all.denied.001',
        request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.bobAliceSummary,
        ruleSet: REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE as unknown as typeof REACTOR_NEED_TO_KNOW_RULESET_V1,
      }),
    ).toThrow()
  })
})
