import { describe, expect, test } from 'bun:test'
import { Schema as S } from 'effect'

import {
  REACTOR_EXAMPLE_POLICIES,
  REACTOR_MODEL_CATALOG_SEED,
  REACTOR_SERVER_CLASS_HYDRALISK_PROFILE,
  ReactorLocalTokenMeteringReceipt,
  ReactorModelInstallReceipt,
  ReactorModelCatalog,
  type ReactorModelCatalog as ReactorModelCatalogType,
  type ReactorModelProvenance,
  ReactorModelPolicyDecisionReceipt,
  ReactorNodeModelProfile,
  buildReactorLocalTokenMeteringReceipt,
  provisionReactorModel,
  routeReactorOpenAiCompatibleRequest,
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
    expect(catalog.models.every(model => model.evalRefs.length === 0)).toBe(true)
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
