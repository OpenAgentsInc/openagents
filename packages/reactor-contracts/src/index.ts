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

export const ReactorServingLane = S.Literals(['hydralisk', 'psionic'])
export type ReactorServingLane = typeof ReactorServingLane.Type

export const ReactorServingEngineKind = S.Literals([
  'vllm',
  'sglang',
  'tensorrt-llm',
  'llama.cpp',
  'fixture-openai-compatible',
])
export type ReactorServingEngineKind = typeof ReactorServingEngineKind.Type

export const ReactorDeploymentPlacement = S.Literals([
  'customer_premises',
  'customer_controlled_cloud',
  'dogfood',
  'fixture',
])
export type ReactorDeploymentPlacement = typeof ReactorDeploymentPlacement.Type

export const ReactorGatewayProfile = S.Struct({
  endpointRef: S.String,
  phoneHomeAllowedInServingPath: S.Literal(false),
  protocol: S.Literal('openai.chat_completions.v1'),
  servingPathNetwork: S.Literal('offline_once_provisioned'),
})
export type ReactorGatewayProfile = typeof ReactorGatewayProfile.Type

export const ReactorServingEngineProfile = S.Struct({
  engineRef: S.String,
  imageDigestRef: S.String,
  kind: ReactorServingEngineKind,
  modelArtifactRefs: S.Array(S.String),
  versionRef: S.String,
})
export type ReactorServingEngineProfile =
  typeof ReactorServingEngineProfile.Type

export const ReactorNodeModelProfile = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.node_model_profile.v1'),
  displayName: S.String,
  exactLocalMeteringRequired: S.Literal(true),
  gateway: ReactorGatewayProfile,
  modelRef: S.String,
  nodeProfileRef: S.String,
  placement: ReactorDeploymentPlacement,
  policyRef: S.String,
  policyVersion: S.String,
  servingLane: ReactorServingLane,
  servingStack: ReactorServingEngineProfile,
  sourceRefs: S.Array(S.String),
})
export type ReactorNodeModelProfile = typeof ReactorNodeModelProfile.Type

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

export const ReactorModelInstallReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.model_install_receipt.v1'),
  action: S.Literals(['install', 'upgrade']),
  artifactRefs: S.Array(S.String),
  catalogRef: S.String,
  decidedAt: S.String,
  decisionRef: S.String,
  modelRef: S.String,
  nodeProfileRef: S.String,
  policyDecisionRef: S.String,
  policyRef: S.String,
  policyVersion: S.String,
  receiptRef: S.String,
  refusal: S.NullOr(S.Struct({
    blockerRefs: S.Array(S.String),
    reason: S.Literals([
      'model_not_in_catalog',
      'policy_binding_mismatch',
      'policy_nonconforming_model',
    ]),
  })),
  servingLane: ReactorServingLane,
  sourceRefs: S.Array(S.String),
  status: S.Literals(['installed', 'refused_policy']),
  weightsPullAuthorization: S.Literals(['authorized', 'refused_before_pull']),
})
export type ReactorModelInstallReceipt =
  typeof ReactorModelInstallReceipt.Type

export const ReactorRouteDecisionReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.route_decision.v1'),
  blockerRefs: S.Array(S.String),
  catalogRef: S.String,
  decidedAt: S.String,
  decisionRef: S.String,
  gatewayProtocol: S.Literal('openai.chat_completions.v1'),
  nodeProfileRef: S.String,
  policyDecisionRef: S.String,
  policyRef: S.String,
  policyVersion: S.String,
  requestRef: S.String,
  requestedModelRef: S.String,
  routedModelRef: S.NullOr(S.String),
  servingLane: ReactorServingLane,
  servingPathNetwork: S.Literal('offline_once_provisioned'),
  sourceRefs: S.Array(S.String),
  status: S.Literals([
    'routed',
    'refused_not_installed',
    'refused_policy',
    'refused_profile_mismatch',
  ]),
})
export type ReactorRouteDecisionReceipt =
  typeof ReactorRouteDecisionReceipt.Type

export const ReactorLocalTokenMeteringReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.local_token_metering_receipt.v1'),
  blockerRefs: S.Array(S.String),
  completionTokens: S.NullOr(S.Number),
  generatedAt: S.String,
  localOnly: S.Literal(true),
  measurementState: S.Literals(['measured', 'not_measured']),
  modelRef: S.String,
  nodeProfileRef: S.String,
  policyRef: S.String,
  policyVersion: S.String,
  promptTokens: S.NullOr(S.Number),
  receiptRef: S.String,
  requestRef: S.String,
  servingLane: ReactorServingLane,
  sourceRefs: S.Array(S.String),
  totalTokens: S.NullOr(S.Number),
  usageTruth: S.Literals(['exact', 'not_measured']),
})
export type ReactorLocalTokenMeteringReceipt =
  typeof ReactorLocalTokenMeteringReceipt.Type

export type ResolveReactorModelPolicyInput = Readonly<{
  catalog: ReactorModelCatalog
  decidedAt: string
  decisionRef: string
  policy: ReactorModelPolicy
  sourceRefs?: ReadonlyArray<string>
}>

export type ProvisionReactorModelInput = Readonly<{
  action: ReactorModelInstallReceipt['action']
  artifactRefs: ReadonlyArray<string>
  catalog: ReactorModelCatalog
  decidedAt: string
  decisionRef: string
  nodeProfile: ReactorNodeModelProfile
  policy: ReactorModelPolicy
  receiptRef: string
  sourceRefs?: ReadonlyArray<string>
}>

export type RouteReactorOpenAiCompatibleRequestInput = Readonly<{
  catalog: ReactorModelCatalog
  decidedAt: string
  decisionRef: string
  installReceipt: ReactorModelInstallReceipt
  nodeProfile: ReactorNodeModelProfile
  policy: ReactorModelPolicy
  requestRef: string
  requestedModelRef: string
  sourceRefs?: ReadonlyArray<string>
}>

export type ReactorLocalMeteringUsage =
  | Readonly<{
      completionTokens: number
      promptTokens: number
      state: 'exact'
      totalTokens: number
    }>
  | Readonly<{
      reasonRef: string
      state: 'not_measured'
    }>

export type BuildReactorLocalTokenMeteringReceiptInput = Readonly<{
  generatedAt: string
  modelRef: string
  nodeProfile: ReactorNodeModelProfile
  policyRef: string
  policyVersion: string
  receiptRef: string
  requestRef: string
  sourceRefs?: ReadonlyArray<string>
  usage: ReactorLocalMeteringUsage
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

const modelDecisionForRef = (
  decision: ReactorModelPolicyDecisionReceipt,
  modelRef: string,
): ReactorPolicyModelDecision | undefined =>
  decision.modelDecisions.find(modelDecision => modelDecision.modelRef === modelRef)

const provisionRefusalReason = (
  blockerRefs: ReadonlyArray<string>,
): NonNullable<ReactorModelInstallReceipt['refusal']>['reason'] =>
  blockerRefs.includes('blocker.reactor.provision.model_not_in_catalog')
    ? 'model_not_in_catalog'
    : blockerRefs.includes('blocker.reactor.provision.policy_binding_mismatch')
      ? 'policy_binding_mismatch'
      : 'policy_nonconforming_model'

export const provisionReactorModel = (
  input: ProvisionReactorModelInput,
): ReactorModelInstallReceipt => {
  const catalog = S.decodeUnknownSync(ReactorModelCatalog)(input.catalog)
  const policy = S.decodeUnknownSync(ReactorModelPolicy)(input.policy)
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )
  const policyDecision = resolveReactorModelPolicy({
    catalog,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    policy,
    sourceRefs: [nodeProfile.nodeProfileRef, ...(input.sourceRefs ?? [])],
  })
  const modelDecision = modelDecisionForRef(policyDecision, nodeProfile.modelRef)
  const blockerRefs = unique([
    ...(nodeProfile.policyRef !== policy.policyRef ||
    nodeProfile.policyVersion !== policy.version
      ? ['blocker.reactor.provision.policy_binding_mismatch']
      : []),
    ...(modelDecision === undefined
      ? ['blocker.reactor.provision.model_not_in_catalog']
      : []),
    ...(modelDecision !== undefined && modelDecision.status !== 'conforming'
      ? [
          'blocker.reactor.provision.policy_nonconforming_model',
          ...modelDecision.reasonRefs,
        ]
      : []),
  ])
  const installed = blockerRefs.length === 0

  return S.decodeUnknownSync(ReactorModelInstallReceipt)({
    schemaVersion: 'openagents.reactor.model_install_receipt.v1',
    action: input.action,
    artifactRefs: [...input.artifactRefs],
    catalogRef: catalog.catalogRef,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    modelRef: nodeProfile.modelRef,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    policyDecisionRef: policyDecision.decisionRef,
    policyRef: policy.policyRef,
    policyVersion: policy.version,
    receiptRef: input.receiptRef,
    refusal: installed
      ? null
      : {
          blockerRefs,
          reason: provisionRefusalReason(blockerRefs),
        },
    servingLane: nodeProfile.servingLane,
    sourceRefs: unique([
      nodeProfile.nodeProfileRef,
      policy.policyRef,
      policyDecision.decisionRef,
      ...input.artifactRefs,
      ...(input.sourceRefs ?? []),
    ]),
    status: installed ? 'installed' : 'refused_policy',
    weightsPullAuthorization: installed ? 'authorized' : 'refused_before_pull',
  })
}

const routeStatusForBlockers = (
  blockerRefs: ReadonlyArray<string>,
): ReactorRouteDecisionReceipt['status'] =>
  blockerRefs.includes('blocker.reactor.router.profile_model_mismatch') ||
  blockerRefs.includes('blocker.reactor.router.policy_binding_mismatch')
    ? 'refused_profile_mismatch'
    : blockerRefs.includes('blocker.reactor.router.model_not_installed')
      ? 'refused_not_installed'
      : 'refused_policy'

export const routeReactorOpenAiCompatibleRequest = (
  input: RouteReactorOpenAiCompatibleRequestInput,
): ReactorRouteDecisionReceipt => {
  const catalog = S.decodeUnknownSync(ReactorModelCatalog)(input.catalog)
  const policy = S.decodeUnknownSync(ReactorModelPolicy)(input.policy)
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )
  const installReceipt = S.decodeUnknownSync(ReactorModelInstallReceipt)(
    input.installReceipt,
  )
  const policyDecision = resolveReactorModelPolicy({
    catalog,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    policy,
    sourceRefs: [nodeProfile.nodeProfileRef, input.requestRef],
  })
  const modelDecision = modelDecisionForRef(policyDecision, input.requestedModelRef)
  const blockerRefs = unique([
    ...(nodeProfile.modelRef !== input.requestedModelRef
      ? ['blocker.reactor.router.profile_model_mismatch']
      : []),
    ...(installReceipt.nodeProfileRef !== nodeProfile.nodeProfileRef ||
    installReceipt.modelRef !== nodeProfile.modelRef ||
    installReceipt.policyRef !== policy.policyRef ||
    installReceipt.policyVersion !== policy.version
      ? ['blocker.reactor.router.install_receipt_binding_mismatch']
      : []),
    ...(nodeProfile.policyRef !== policy.policyRef ||
    nodeProfile.policyVersion !== policy.version
      ? ['blocker.reactor.router.policy_binding_mismatch']
      : []),
    ...(installReceipt.status !== 'installed' ||
    installReceipt.weightsPullAuthorization !== 'authorized'
      ? ['blocker.reactor.router.model_not_installed']
      : []),
    ...(modelDecision === undefined
      ? ['blocker.reactor.router.model_not_in_catalog']
      : []),
    ...(modelDecision !== undefined && modelDecision.status !== 'conforming'
      ? [
          'blocker.reactor.router.policy_nonconforming_model',
          ...modelDecision.reasonRefs,
        ]
      : []),
  ])
  const routed = blockerRefs.length === 0

  return S.decodeUnknownSync(ReactorRouteDecisionReceipt)({
    schemaVersion: 'openagents.reactor.route_decision.v1',
    blockerRefs,
    catalogRef: catalog.catalogRef,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    gatewayProtocol: nodeProfile.gateway.protocol,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    policyDecisionRef: policyDecision.decisionRef,
    policyRef: policy.policyRef,
    policyVersion: policy.version,
    requestRef: input.requestRef,
    requestedModelRef: input.requestedModelRef,
    routedModelRef: routed ? input.requestedModelRef : null,
    servingLane: nodeProfile.servingLane,
    servingPathNetwork: nodeProfile.gateway.servingPathNetwork,
    sourceRefs: unique([
      nodeProfile.nodeProfileRef,
      installReceipt.receiptRef,
      policy.policyRef,
      policyDecision.decisionRef,
      ...(input.sourceRefs ?? []),
    ]),
    status: routed ? 'routed' : routeStatusForBlockers(blockerRefs),
  })
}

const exactTokenCount = (value: number): boolean =>
  Number.isInteger(value) && value >= 0

export const buildReactorLocalTokenMeteringReceipt = (
  input: BuildReactorLocalTokenMeteringReceiptInput,
): ReactorLocalTokenMeteringReceipt => {
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )

  if (input.usage.state === 'exact') {
    const countsValid =
      exactTokenCount(input.usage.promptTokens) &&
      exactTokenCount(input.usage.completionTokens) &&
      exactTokenCount(input.usage.totalTokens)
    if (
      !countsValid ||
      input.usage.promptTokens + input.usage.completionTokens !==
        input.usage.totalTokens
    ) {
      throw new Error('reactor.local_metering.exact_counts_do_not_reconcile')
    }
  }

  return S.decodeUnknownSync(ReactorLocalTokenMeteringReceipt)({
    schemaVersion: 'openagents.reactor.local_token_metering_receipt.v1',
    blockerRefs:
      input.usage.state === 'exact'
        ? []
        : ['blocker.reactor.local_metering.not_measured', input.usage.reasonRef],
    completionTokens:
      input.usage.state === 'exact' ? input.usage.completionTokens : null,
    generatedAt: input.generatedAt,
    localOnly: true,
    measurementState: input.usage.state === 'exact' ? 'measured' : 'not_measured',
    modelRef: input.modelRef,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    policyRef: input.policyRef,
    policyVersion: input.policyVersion,
    promptTokens: input.usage.state === 'exact' ? input.usage.promptTokens : null,
    receiptRef: input.receiptRef,
    requestRef: input.requestRef,
    servingLane: nodeProfile.servingLane,
    sourceRefs: unique([
      nodeProfile.nodeProfileRef,
      input.requestRef,
      ...(input.sourceRefs ?? []),
    ]),
    totalTokens: input.usage.state === 'exact' ? input.usage.totalTokens : null,
    usageTruth: input.usage.state === 'exact' ? 'exact' : 'not_measured',
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

const nodeProfile = (
  record: Omit<ReactorNodeModelProfile, 'schemaVersion'>,
): ReactorNodeModelProfile =>
  S.decodeUnknownSync(ReactorNodeModelProfile)({
    schemaVersion: 'openagents.reactor.node_model_profile.v1',
    ...record,
  })

export const REACTOR_SERVER_CLASS_HYDRALISK_PROFILE = nodeProfile({
  displayName: 'Fixture server-class Reactor Hydralisk profile',
  exactLocalMeteringRequired: true,
  gateway: {
    endpointRef: 'endpoint.reactor.fixture.openai_compatible.local',
    phoneHomeAllowedInServingPath: false,
    protocol: 'openai.chat_completions.v1',
    servingPathNetwork: 'offline_once_provisioned',
  },
  modelRef: 'model.openai.gpt_oss.open_family',
  nodeProfileRef: 'reactor.node_profile.fixture.hydralisk.server_class.v1',
  placement: 'fixture',
  policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
  policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
  servingLane: 'hydralisk',
  servingStack: {
    engineRef: 'engine.hydralisk.vllm.fixture.server_class',
    imageDigestRef: 'image.hydralisk.vllm.fixture.sha256',
    kind: 'vllm',
    modelArtifactRefs: ['artifact.fixture.gpt_oss.open_family.weights'],
    versionRef: 'vllm.fixture.server_class',
  },
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8273',
    'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#41-serving-lane-policy-hydralisk-by-default-psionic-by-exception',
  ],
})
