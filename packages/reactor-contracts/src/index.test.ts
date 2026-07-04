import { describe, expect, test } from 'bun:test'
import { Schema as S } from 'effect'

import {
  REACTOR_EXAMPLE_POLICIES,
  REACTOR_MODEL_CATALOG_SEED,
  ReactorModelCatalog,
  type ReactorModelCatalog as ReactorModelCatalogType,
  type ReactorModelProvenance,
  ReactorModelPolicyDecisionReceipt,
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
