import { Schema as S } from 'effect'

export const ReactorOriginJurisdiction = S.Literals([
  'us',
  'eu',
  'fr',
  'cn',
  'jp',
  'kr',
  'mixed',
  'unknown',
])
export type ReactorOriginJurisdiction = typeof ReactorOriginJurisdiction.Type

export const ReactorModelLicense = S.Literals([
  'apache-2.0',
  'mit',
  'llama-community',
  'gemma-terms',
  'mistral-research',
  'nvidia-open-model',
  'qwen',
  'deepseek',
  'moonshot',
  'zai',
  'unknown',
])
export type ReactorModelLicense = typeof ReactorModelLicense.Type

export const ReactorLicenseClass = S.Literals([
  'permissive',
  'community_restricted',
  'research_restricted',
  'unknown',
])
export type ReactorLicenseClass = typeof ReactorLicenseClass.Type

export const ReactorWeightsOpenness = S.Literals([
  'open-weights',
  'open-weights-restricted',
  'unknown',
])
export type ReactorWeightsOpenness = typeof ReactorWeightsOpenness.Type

export const ReactorTrainingDataDisclosure = S.Literals([
  'disclosed',
  'partial',
  'undisclosed',
  'unknown',
])
export type ReactorTrainingDataDisclosure =
  typeof ReactorTrainingDataDisclosure.Type

export const ReactorRoutingPreference = S.Literals([
  'quality_first',
  'cost_first',
  'balanced',
])
export type ReactorRoutingPreference = typeof ReactorRoutingPreference.Type

export const ReactorDistillationLineageEntry = S.Struct({
  developer: S.String,
  disclosure: S.Literals(['documented', 'reported', 'suspected', 'unknown']),
  modelRef: S.String,
  originJurisdiction: ReactorOriginJurisdiction,
  sourceRefs: S.Array(S.String),
})
export type ReactorDistillationLineageEntry =
  typeof ReactorDistillationLineageEntry.Type

export const ReactorModelProvenance = S.Struct({
  schemaVersion: S.Literal('openagents.model_provenance.v1'),
  developer: S.String,
  displayName: S.String,
  distillationLineage: S.Array(ReactorDistillationLineageEntry),
  evalRefs: S.Array(S.String),
  family: S.String,
  license: ReactorModelLicense,
  licenseClass: ReactorLicenseClass,
  modelRef: S.String,
  originJurisdiction: ReactorOriginJurisdiction,
  sourceRefs: S.Array(S.String),
  trainingDataDisclosure: ReactorTrainingDataDisclosure,
  weightsOpenness: ReactorWeightsOpenness,
})
export type ReactorModelProvenance = typeof ReactorModelProvenance.Type

export const ReactorModelCatalog = S.Struct({
  schemaVersion: S.Literal('openagents.reactor_model_catalog.v1'),
  catalogRef: S.String,
  generatedAt: S.String,
  models: S.Array(ReactorModelProvenance),
  sourceRefs: S.Array(S.String),
})
export type ReactorModelCatalog = typeof ReactorModelCatalog.Type

export const ReactorModelPolicyConstraints = S.Struct({
  allowDevelopers: S.optional(S.Array(S.String)),
  allowLicenseClasses: S.optional(S.Array(ReactorLicenseClass)),
  allowLicenses: S.optional(S.Array(ReactorModelLicense)),
  allowOriginJurisdictions: S.optional(S.Array(ReactorOriginJurisdiction)),
  allowWeightsOpenness: S.optional(S.Array(ReactorWeightsOpenness)),
  blockDevelopers: S.optional(S.Array(S.String)),
  blockOriginJurisdictions: S.optional(S.Array(ReactorOriginJurisdiction)),
  enforceDistillationLineageJurisdiction: S.Boolean,
  requireTrainingDataDisclosure: S.optional(
    S.Array(ReactorTrainingDataDisclosure),
  ),
})
export type ReactorModelPolicyConstraints =
  typeof ReactorModelPolicyConstraints.Type

export const ReactorTaskRoutingPreference = S.Struct({
  preference: ReactorRoutingPreference,
  taskClassRef: S.String,
})
export type ReactorTaskRoutingPreference =
  typeof ReactorTaskRoutingPreference.Type

export const ReactorModelPolicy = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.model_policy.v1'),
  constraints: ReactorModelPolicyConstraints,
  ownerRef: S.String,
  policyRef: S.String,
  routingPreferences: S.Struct({
    defaultPreference: ReactorRoutingPreference,
    taskPreferences: S.Array(ReactorTaskRoutingPreference),
  }),
  sourceRefs: S.Array(S.String),
  version: S.String,
})
export type ReactorModelPolicy = typeof ReactorModelPolicy.Type

export const ReactorPolicyModelDecision = S.Struct({
  modelRef: S.String,
  reasonRefs: S.Array(S.String),
  status: S.Literals(['conforming', 'refused']),
})
export type ReactorPolicyModelDecision =
  typeof ReactorPolicyModelDecision.Type

export const ReactorModelPolicyDecisionReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.model_policy_decision.v1'),
  catalogRef: S.String,
  conformingModelRefs: S.Array(S.String),
  decidedAt: S.String,
  decisionRef: S.String,
  modelDecisions: S.Array(ReactorPolicyModelDecision),
  policyRef: S.String,
  policyVersion: S.String,
  refusal: S.NullOr(S.Struct({
    blockerRefs: S.Array(S.String),
    reason: S.Literal('no_conforming_models'),
  })),
  sourceRefs: S.Array(S.String),
  status: S.Literals(['conforming_models', 'refused_empty_set']),
})
export type ReactorModelPolicyDecisionReceipt =
  typeof ReactorModelPolicyDecisionReceipt.Type

export type ResolveReactorModelPolicyInput = Readonly<{
  catalog: ReactorModelCatalog
  decidedAt: string
  decisionRef: string
  policy: ReactorModelPolicy
  sourceRefs?: ReadonlyArray<string>
}>

const unique = <T extends string>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  [...new Set(values)]

const normalized = (value: string): string => value.trim().toLowerCase()

const optionalIncludes = <T extends string>(
  values: ReadonlyArray<T> | undefined,
  value: T,
): boolean => values === undefined || values.length === 0 || values.includes(value)

const optionalBlocks = <T extends string>(
  values: ReadonlyArray<T> | undefined,
  value: T,
): boolean => values !== undefined && values.includes(value)

const optionalStringIncludes = (
  values: ReadonlyArray<string> | undefined,
  value: string,
): boolean =>
  values === undefined ||
  values.length === 0 ||
  values.map(normalized).includes(normalized(value))

const optionalStringBlocks = (
  values: ReadonlyArray<string> | undefined,
  value: string,
): boolean =>
  values !== undefined && values.map(normalized).includes(normalized(value))

const refusalReasonsForModel = (
  policy: ReactorModelPolicy,
  model: ReactorModelProvenance,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []
  const constraints = policy.constraints

  if (
    !optionalIncludes(
      constraints.allowOriginJurisdictions,
      model.originJurisdiction,
    )
  ) {
    reasons.push('reactor.policy.origin_not_allowed')
  }
  if (
    optionalBlocks(
      constraints.blockOriginJurisdictions,
      model.originJurisdiction,
    )
  ) {
    reasons.push('reactor.policy.origin_blocked')
  }
  if (!optionalStringIncludes(constraints.allowDevelopers, model.developer)) {
    reasons.push('reactor.policy.developer_not_allowed')
  }
  if (optionalStringBlocks(constraints.blockDevelopers, model.developer)) {
    reasons.push('reactor.policy.developer_blocked')
  }
  if (!optionalIncludes(constraints.allowLicenses, model.license)) {
    reasons.push('reactor.policy.license_not_allowed')
  }
  if (!optionalIncludes(constraints.allowLicenseClasses, model.licenseClass)) {
    reasons.push('reactor.policy.license_class_not_allowed')
  }
  if (!optionalIncludes(constraints.allowWeightsOpenness, model.weightsOpenness)) {
    reasons.push('reactor.policy.weights_openness_not_allowed')
  }
  if (
    !optionalIncludes(
      constraints.requireTrainingDataDisclosure,
      model.trainingDataDisclosure,
    )
  ) {
    reasons.push('reactor.policy.training_data_disclosure_not_allowed')
  }

  if (constraints.enforceDistillationLineageJurisdiction) {
    for (const lineage of model.distillationLineage) {
      if (
        !optionalIncludes(
          constraints.allowOriginJurisdictions,
          lineage.originJurisdiction,
        )
      ) {
        reasons.push('reactor.policy.lineage_origin_not_allowed')
      }
      if (
        optionalBlocks(
          constraints.blockOriginJurisdictions,
          lineage.originJurisdiction,
        )
      ) {
        reasons.push('reactor.policy.lineage_origin_blocked')
      }
    }
  }

  return unique(reasons)
}

export const resolveReactorModelPolicy = (
  input: ResolveReactorModelPolicyInput,
): ReactorModelPolicyDecisionReceipt => {
  const catalog = S.decodeUnknownSync(ReactorModelCatalog)(input.catalog)
  const policy = S.decodeUnknownSync(ReactorModelPolicy)(input.policy)
  const modelDecisions = catalog.models.map(model => {
    const reasonRefs = refusalReasonsForModel(policy, model)
    return {
      modelRef: model.modelRef,
      reasonRefs,
      status: reasonRefs.length === 0 ? 'conforming' : 'refused',
    } satisfies ReactorPolicyModelDecision
  })
  const conformingModelRefs = modelDecisions
    .filter(decision => decision.status === 'conforming')
    .map(decision => decision.modelRef)
  const status =
    conformingModelRefs.length === 0
      ? ('refused_empty_set' as const)
      : ('conforming_models' as const)

  return S.decodeUnknownSync(ReactorModelPolicyDecisionReceipt)({
    schemaVersion: 'openagents.reactor.model_policy_decision.v1',
    catalogRef: catalog.catalogRef,
    conformingModelRefs,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    modelDecisions,
    policyRef: policy.policyRef,
    policyVersion: policy.version,
    refusal:
      status === 'refused_empty_set'
        ? {
            blockerRefs: [
              'blocker.reactor.model_policy.no_conforming_models',
              `policy:${policy.policyRef}`,
              `policy_version:${policy.version}`,
            ],
            reason: 'no_conforming_models',
          }
        : null,
    sourceRefs: unique([
      catalog.catalogRef,
      policy.policyRef,
      ...(input.sourceRefs ?? []),
    ]),
    status,
  })
}

const model = (
  record: Omit<ReactorModelProvenance, 'schemaVersion'>,
): ReactorModelProvenance =>
  S.decodeUnknownSync(ReactorModelProvenance)({
    schemaVersion: 'openagents.model_provenance.v1',
    ...record,
  })

export const REACTOR_MODEL_CATALOG_SEED = S.decodeUnknownSync(
  ReactorModelCatalog,
)({
  schemaVersion: 'openagents.reactor_model_catalog.v1',
  catalogRef: 'reactor.model_catalog.seed.20260704.v1',
  generatedAt: '2026-07-04T00:00:00.000Z',
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8272',
    'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#3-the-model-catalog-and-the-provenance-policy-the-differentiator',
  ],
  models: [
    model({
      developer: 'nvidia',
      displayName: 'NVIDIA Nemotron family',
      distillationLineage: [],
      evalRefs: [],
      family: 'nemotron',
      license: 'nvidia-open-model',
      licenseClass: 'community_restricted',
      modelRef: 'model.nvidia.nemotron.open_family',
      originJurisdiction: 'us',
      sourceRefs: [
        'source:vendor.nvidia.nemotron_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights-restricted',
    }),
    model({
      developer: 'meta',
      displayName: 'Meta Llama family',
      distillationLineage: [],
      evalRefs: [],
      family: 'llama',
      license: 'llama-community',
      licenseClass: 'community_restricted',
      modelRef: 'model.meta.llama.open_family',
      originJurisdiction: 'us',
      sourceRefs: [
        'source:vendor.meta.llama_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights-restricted',
    }),
    model({
      developer: 'openai',
      displayName: 'OpenAI GPT-OSS family',
      distillationLineage: [],
      evalRefs: [],
      family: 'gpt-oss',
      license: 'apache-2.0',
      licenseClass: 'permissive',
      modelRef: 'model.openai.gpt_oss.open_family',
      originJurisdiction: 'us',
      sourceRefs: [
        'source:vendor.openai.gpt_oss_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'unknown',
      weightsOpenness: 'open-weights',
    }),
    model({
      developer: 'google',
      displayName: 'Google Gemma family',
      distillationLineage: [],
      evalRefs: [],
      family: 'gemma',
      license: 'gemma-terms',
      licenseClass: 'community_restricted',
      modelRef: 'model.google.gemma.open_family',
      originJurisdiction: 'us',
      sourceRefs: [
        'source:vendor.google.gemma_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights-restricted',
    }),
    model({
      developer: 'mistral',
      displayName: 'Mistral / Magistral family',
      distillationLineage: [],
      evalRefs: [],
      family: 'mistral',
      license: 'mistral-research',
      licenseClass: 'research_restricted',
      modelRef: 'model.mistral.magistral_open_family',
      originJurisdiction: 'fr',
      sourceRefs: [
        'source:vendor.mistral.magistral_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights-restricted',
    }),
    model({
      developer: 'alibaba',
      displayName: 'Alibaba Qwen family',
      distillationLineage: [],
      evalRefs: [],
      family: 'qwen',
      license: 'qwen',
      licenseClass: 'community_restricted',
      modelRef: 'model.alibaba.qwen.open_family',
      originJurisdiction: 'cn',
      sourceRefs: [
        'source:vendor.alibaba.qwen_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights',
    }),
    model({
      developer: 'deepseek',
      displayName: 'DeepSeek family',
      distillationLineage: [
        {
          developer: 'alibaba',
          disclosure: 'documented',
          modelRef: 'model.alibaba.qwen.open_family',
          originJurisdiction: 'cn',
          sourceRefs: ['source:vendor.deepseek.distill_qwen_lineage'],
        },
      ],
      evalRefs: [],
      family: 'deepseek',
      license: 'deepseek',
      licenseClass: 'community_restricted',
      modelRef: 'model.deepseek.open_family',
      originJurisdiction: 'cn',
      sourceRefs: [
        'source:vendor.deepseek.open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'partial',
      weightsOpenness: 'open-weights',
    }),
    model({
      developer: 'moonshot',
      displayName: 'Kimi family',
      distillationLineage: [],
      evalRefs: [],
      family: 'kimi',
      license: 'moonshot',
      licenseClass: 'community_restricted',
      modelRef: 'model.moonshot.kimi.open_family',
      originJurisdiction: 'cn',
      sourceRefs: [
        'source:vendor.moonshot.kimi_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'unknown',
      weightsOpenness: 'open-weights',
    }),
    model({
      developer: 'zai',
      displayName: 'GLM family',
      distillationLineage: [],
      evalRefs: [],
      family: 'glm',
      license: 'zai',
      licenseClass: 'community_restricted',
      modelRef: 'model.zai.glm.open_family',
      originJurisdiction: 'cn',
      sourceRefs: [
        'source:vendor.zai.glm_open_family',
        'source:reactor.seed.20260704',
      ],
      trainingDataDisclosure: 'unknown',
      weightsOpenness: 'open-weights',
    }),
  ],
})

const policy = (
  record: Omit<ReactorModelPolicy, 'schemaVersion'>,
): ReactorModelPolicy =>
  S.decodeUnknownSync(ReactorModelPolicy)({
    schemaVersion: 'openagents.reactor.model_policy.v1',
    ...record,
  })

const basePolicy = {
  ownerRef: 'owner.openagents.reactor.fixture',
  routingPreferences: {
    defaultPreference: 'balanced' as const,
    taskPreferences: [],
  },
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8272',
    'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#3-the-model-catalog-and-the-provenance-policy-the-differentiator',
  ],
}

export const REACTOR_EXAMPLE_POLICIES = {
  noCn: policy({
    ...basePolicy,
    constraints: {
      blockOriginJurisdictions: ['cn'],
      enforceDistillationLineageJurisdiction: true,
    },
    policyRef: 'reactor.model_policy.example.no_cn.v1',
    version: '2026-07-04.no-cn',
  }),
  permissiveLicenseOnly: policy({
    ...basePolicy,
    constraints: {
      allowLicenseClasses: ['permissive'],
      enforceDistillationLineageJurisdiction: true,
    },
    policyRef: 'reactor.model_policy.example.permissive_license_only.v1',
    version: '2026-07-04.permissive-license-only',
  }),
  unconstrained: policy({
    ...basePolicy,
    constraints: {
      enforceDistillationLineageJurisdiction: false,
    },
    policyRef: 'reactor.model_policy.example.unconstrained.v1',
    version: '2026-07-04.unconstrained',
  }),
  usOnly: policy({
    ...basePolicy,
    constraints: {
      allowOriginJurisdictions: ['us'],
      enforceDistillationLineageJurisdiction: true,
    },
    policyRef: 'reactor.model_policy.example.us_only.v1',
    version: '2026-07-04.us-only',
  }),
} satisfies Record<string, ReactorModelPolicy>
