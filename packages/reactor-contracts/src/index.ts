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

export const REACTOR_EVAL_TASK_CLASS_REFS = [
  'drafting',
  'extraction',
  'rag_over_corpus',
  'agent_tool_use',
] as const

export const ReactorEvalTaskClassRef = S.Literals(REACTOR_EVAL_TASK_CLASS_REFS)
export type ReactorEvalTaskClassRef = typeof ReactorEvalTaskClassRef.Type

export const ReactorEvalExecutionTarget = S.Literals([
  'rx3_served_model',
  'hosted_equivalent_large_model',
  'not_measured',
])
export type ReactorEvalExecutionTarget = typeof ReactorEvalExecutionTarget.Type

export const ReactorEvalScoreUnit = S.Literals([
  'score_0_to_1',
  'pass_rate',
  'exact_match',
  'f1',
])
export type ReactorEvalScoreUnit = typeof ReactorEvalScoreUnit.Type

export const ReactorHardwareTierRef = S.Literals([
  'workstation',
  'server',
  'rack',
])
export type ReactorHardwareTierRef = typeof ReactorHardwareTierRef.Type

export const ReactorEvalHarnessProfile = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.eval_harness_profile.v1'),
  harnessRef: S.String,
  runnerOwner: S.Literal('psionic'),
  sourceRefs: S.Array(S.String),
  supportedExecutionTargets: S.Array(ReactorEvalExecutionTarget),
  taskClassRefs: S.Array(ReactorEvalTaskClassRef),
  unrunMeasurementState: S.Literal('not_measured'),
})
export type ReactorEvalHarnessProfile = typeof ReactorEvalHarnessProfile.Type

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

export const ReactorModelEvalReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.model_eval_receipt.v1'),
  blockerRefs: S.Array(S.String),
  capabilityCopyAllowed: S.Boolean,
  evalDatasetRef: S.String,
  executionTarget: ReactorEvalExecutionTarget,
  generatedAt: S.String,
  harnessRef: S.String,
  measurementState: S.Literals(['measured', 'not_measured']),
  modelRef: S.String,
  receiptRef: S.String,
  runnerOwner: S.Literal('psionic'),
  sampleCount: S.NullOr(S.Number),
  score: S.NullOr(S.Number),
  scoreUnit: S.NullOr(ReactorEvalScoreUnit),
  sourceRefs: S.Array(S.String),
  taskClassRef: ReactorEvalTaskClassRef,
})
export type ReactorModelEvalReceipt = typeof ReactorModelEvalReceipt.Type

export const ReactorEvalCoverageCell = S.Struct({
  blockerRefs: S.Array(S.String),
  capabilityCopyAllowed: S.Boolean,
  measurementState: S.Literals(['measured', 'not_measured']),
  modelRef: S.String,
  receiptRef: S.NullOr(S.String),
  score: S.NullOr(S.Number),
  taskClassRef: ReactorEvalTaskClassRef,
})
export type ReactorEvalCoverageCell = typeof ReactorEvalCoverageCell.Type

export const ReactorEvalCoverageMatrix = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.eval_coverage_matrix.v1'),
  catalogRef: S.String,
  cells: S.Array(ReactorEvalCoverageCell),
  generatedAt: S.String,
  matrixRef: S.String,
  sourceRefs: S.Array(S.String),
})
export type ReactorEvalCoverageMatrix = typeof ReactorEvalCoverageMatrix.Type

export const ReactorCapabilityCopyEvalDecision = S.Struct({
  schemaVersion: S.Literal(
    'openagents.reactor.capability_copy_eval_decision.v1',
  ),
  allowedEvalRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  decidedAt: S.String,
  decisionRef: S.String,
  modelRef: S.String,
  sourceRefs: S.Array(S.String),
  status: S.Literals(['allowed', 'blocked_not_measured']),
  taskClassRefs: S.Array(ReactorEvalTaskClassRef),
})
export type ReactorCapabilityCopyEvalDecision =
  typeof ReactorCapabilityCopyEvalDecision.Type

export const ReactorHardwareTierSpec = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.hardware_tier_spec.v1'),
  cpu: S.String,
  guidanceOnly: S.Literal(true),
  memory: S.String,
  network: S.String,
  notes: S.Array(S.String),
  storage: S.String,
  tierRef: ReactorHardwareTierRef,
})
export type ReactorHardwareTierSpec = typeof ReactorHardwareTierSpec.Type

export const ReactorAirgapUpdateBundleManifest = S.Struct({
  schemaVersion: S.Literal(
    'openagents.reactor.airgap_update_bundle_manifest.v1',
  ),
  artifactSha256: S.String,
  bundleRef: S.String,
  bundleVersion: S.String,
  callbackRequired: S.Literal(false),
  createdAt: S.String,
  modelRef: S.String,
  nodeProfileRef: S.String,
  policyRef: S.String,
  policyVersion: S.String,
  publicKeyRef: S.String,
  signatureAlg: S.Literal('ed25519'),
  signatureKid: S.String,
  signatureRef: S.String,
  sourceRefs: S.Array(S.String),
  verifierRef: S.String,
})
export type ReactorAirgapUpdateBundleManifest =
  typeof ReactorAirgapUpdateBundleManifest.Type

export const ReactorInstallOpsReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.install_ops_receipt.v1'),
  action: S.Literals(['fresh_install', 'upgrade', 'rollback']),
  blockerRefs: S.Array(S.String),
  bundleRef: S.String,
  decidedAt: S.String,
  modelInstallReceiptRef: S.String,
  modelRef: S.String,
  nodeProfileRef: S.String,
  policyDecisionRef: S.String,
  policyRef: S.String,
  policyVersion: S.String,
  receiptRef: S.String,
  rollbackFromBundleRef: S.NullOr(S.String),
  rollbackToBundleRef: S.NullOr(S.String),
  sourceRefs: S.Array(S.String),
  status: S.Literals(['succeeded', 'refused']),
  verificationRefs: S.Array(S.String),
})
export type ReactorInstallOpsReceipt = typeof ReactorInstallOpsReceipt.Type

export const ReactorDogfoodRunReceipt = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.dogfood_run_receipt.v1'),
  blockerRefs: S.Array(S.String),
  caseStudyWriteupRef: S.String,
  externalClaimFlipAllowed: S.Literal(false),
  externalPilotAuthorized: S.Literal(false),
  generatedAt: S.String,
  hardwareOwnerRef: S.Literal('owner.openagents'),
  installOpsReceiptRef: S.String,
  localMeteringReceiptRefs: S.Array(S.String),
  measuredWindowEndedAt: S.String,
  measuredWindowStartedAt: S.String,
  nodeProfileRef: S.String,
  placement: S.Literal('dogfood'),
  policyConstraintRefs: S.Array(S.String),
  policyDecisionReceiptRefs: S.Array(S.String),
  policyRef: S.String,
  policyVersion: S.String,
  publicSafe: S.Literal(true),
  receiptRef: S.String,
  refusedNonconformingInstallOpsReceiptRef: S.String,
  refusedNonconformingModelRef: S.String,
  requestRefs: S.Array(S.String),
  routeDecisionRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: S.Literals(['completed', 'refused']),
  strictPolicyRef: S.String,
  totalMeasuredTokens: S.Number,
  workloadClass: S.Literal('internal_lead_gen_case_study_seed'),
  workloadRef: S.String,
  workloadTruth: S.Literal('internal_openagents'),
})
export type ReactorDogfoodRunReceipt = typeof ReactorDogfoodRunReceipt.Type

export const ReactorNeedToKnowRoleRef = S.Literals([
  'client_admin',
  'client_member',
  'external_reviewer',
  'matter_attorney',
  'matter_analyst',
  'operator',
])
export type ReactorNeedToKnowRoleRef = typeof ReactorNeedToKnowRoleRef.Type

export const ReactorNeedToKnowOutputMode = S.Literals([
  'citation',
  'source',
  'summary',
])
export type ReactorNeedToKnowOutputMode =
  typeof ReactorNeedToKnowOutputMode.Type

export const ReactorNeedToKnowOracleVerdictKind = S.Literals([
  'need_to_know_plausible',
  'not_need_to_know',
  'not_evaluated',
])
export type ReactorNeedToKnowOracleVerdictKind =
  typeof ReactorNeedToKnowOracleVerdictKind.Type

export const ReactorNeedToKnowDecisionStatus = S.Literals([
  'allowed',
  'denied_hard_rule',
  'denied_soft_oracle',
])
export type ReactorNeedToKnowDecisionStatus =
  typeof ReactorNeedToKnowDecisionStatus.Type

export const ReactorNeedToKnowHardDecisionStatus = S.Literals([
  'failed',
  'passed',
])
export type ReactorNeedToKnowHardDecisionStatus =
  typeof ReactorNeedToKnowHardDecisionStatus.Type

export const ReactorNeedToKnowOracleDecisionStatus = S.Literals([
  'failed',
  'passed',
  'skipped_hard_denied',
])
export type ReactorNeedToKnowOracleDecisionStatus =
  typeof ReactorNeedToKnowOracleDecisionStatus.Type

export const ReactorNeedToKnowRuleSet = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.need_to_know_ruleset.v1'),
  defaultAction: S.Literal('deny'),
  hardRuleRefs: S.Array(S.String),
  oraclePolicyRef: S.String,
  ruleSetRef: S.String,
  sourceRefs: S.Array(S.String),
  version: S.String,
})
export type ReactorNeedToKnowRuleSet = typeof ReactorNeedToKnowRuleSet.Type

export const ReactorCorpusDocumentRecord = S.Struct({
  schemaVersion: S.Literal('openagents.reactor.corpus_document.v1'),
  allowedRoleRefs: S.Array(ReactorNeedToKnowRoleRef),
  allowedUserRefs: S.Array(S.String),
  citationRefs: S.Array(S.String),
  documentRef: S.String,
  matterRef: S.String,
  sourceRefs: S.Array(S.String),
  summaryRef: S.String,
  workspaceRef: S.String,
})
export type ReactorCorpusDocumentRecord =
  typeof ReactorCorpusDocumentRecord.Type

export const ReactorCorpusAccessSubject = S.Struct({
  matterRefs: S.Array(S.String),
  roleRefs: S.Array(ReactorNeedToKnowRoleRef),
  subjectUserRef: S.String,
  workspaceRefs: S.Array(S.String),
})
export type ReactorCorpusAccessSubject =
  typeof ReactorCorpusAccessSubject.Type

export const ReactorCorpusRetrievalRequest = S.Struct({
  matterRef: S.String,
  outputMode: ReactorNeedToKnowOutputMode,
  queryIntentRef: S.String,
  requestRef: S.String,
  requestedDocumentRefs: S.Array(S.String),
  subject: ReactorCorpusAccessSubject,
  workspaceRef: S.String,
})
export type ReactorCorpusRetrievalRequest =
  typeof ReactorCorpusRetrievalRequest.Type

export const ReactorNeedToKnowOracleVerdict = S.Struct({
  documentRef: S.String,
  loggedAt: S.String,
  modelRef: S.String,
  reasonRefs: S.Array(S.String),
  requestRef: S.String,
  verdict: ReactorNeedToKnowOracleVerdictKind,
  verdictRef: S.String,
})
export type ReactorNeedToKnowOracleVerdict =
  typeof ReactorNeedToKnowOracleVerdict.Type

export const ReactorNeedToKnowDocumentDecision = S.Struct({
  blockerRefs: S.Array(S.String),
  citationRefs: S.Array(S.String),
  decisionRef: S.String,
  documentRef: S.String,
  hardDecisionStatus: ReactorNeedToKnowHardDecisionStatus,
  hardRuleRefs: S.Array(S.String),
  oracleDecisionStatus: ReactorNeedToKnowOracleDecisionStatus,
  oracleVerdictRef: S.NullOr(S.String),
  reasonRefs: S.Array(S.String),
  status: ReactorNeedToKnowDecisionStatus,
})
export type ReactorNeedToKnowDocumentDecision =
  typeof ReactorNeedToKnowDocumentDecision.Type

export const ReactorCorpusAccessDecisionReceipt = S.Struct({
  schemaVersion: S.Literal(
    'openagents.reactor.corpus_access_decision_receipt.v1',
  ),
  decidedAt: S.String,
  deniedCitationRefs: S.Array(S.String),
  deniedDocumentRefs: S.Array(S.String),
  documentDecisions: S.Array(ReactorNeedToKnowDocumentDecision),
  generatedSummaryContentLogged: S.Literal(false),
  matterRef: S.String,
  oracleAppliedAfterHardRules: S.Literal(true),
  oracleVerdictRefs: S.Array(S.String),
  outputMode: ReactorNeedToKnowOutputMode,
  queryIntentRef: S.String,
  rawDocumentContentLogged: S.Literal(false),
  receiptRef: S.String,
  requestedDocumentRefs: S.Array(S.String),
  ruleSetRef: S.String,
  ruleSetVersion: S.String,
  selectedCitationRefs: S.Array(S.String),
  selectedDocumentRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subjectUserRef: S.String,
  workspaceRef: S.String,
})
export type ReactorCorpusAccessDecisionReceipt =
  typeof ReactorCorpusAccessDecisionReceipt.Type

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

export type ReactorEvalMeasurement =
  | Readonly<{
      evalDatasetRef: string
      executionTarget: Exclude<ReactorEvalExecutionTarget, 'not_measured'>
      sampleCount: number
      score: number
      scoreUnit: ReactorEvalScoreUnit
      state: 'measured'
    }>
  | Readonly<{
      evalDatasetRef?: string
      reasonRef: string
      state: 'not_measured'
    }>

export type BuildReactorModelEvalReceiptInput = Readonly<{
  catalog: ReactorModelCatalog
  generatedAt: string
  harnessProfile?: ReactorEvalHarnessProfile
  measurement: ReactorEvalMeasurement
  modelRef: string
  receiptRef: string
  sourceRefs?: ReadonlyArray<string>
  taskClassRef: ReactorEvalTaskClassRef
}>

export type BuildReactorEvalCoverageMatrixInput = Readonly<{
  catalog: ReactorModelCatalog
  evalReceipts: ReadonlyArray<ReactorModelEvalReceipt>
  generatedAt: string
  matrixRef: string
  sourceRefs?: ReadonlyArray<string>
  taskClassRefs?: ReadonlyArray<ReactorEvalTaskClassRef>
}>

export type SelectReactorCapabilityCopyEvalRefsInput = Readonly<{
  decidedAt: string
  decisionRef: string
  evalReceipts: ReadonlyArray<ReactorModelEvalReceipt>
  modelRef: string
  sourceRefs?: ReadonlyArray<string>
  taskClassRefs: ReadonlyArray<ReactorEvalTaskClassRef>
}>

export type BuildReactorAirgapUpdateBundleManifestInput = Readonly<{
  artifactSha256: string
  bundleRef: string
  bundleVersion: string
  createdAt: string
  modelRef: string
  nodeProfile: ReactorNodeModelProfile
  policyRef: string
  policyVersion: string
  publicKeyRef?: string
  signatureKid: string
  signatureRef: string
  sourceRefs?: ReadonlyArray<string>
  verifierRef?: string
}>

export type BuildReactorInstallOpsReceiptInput = Readonly<{
  action: ReactorInstallOpsReceipt['action']
  bundle: ReactorAirgapUpdateBundleManifest
  catalog: ReactorModelCatalog
  decidedAt: string
  nodeProfile: ReactorNodeModelProfile
  policy: ReactorModelPolicy
  receiptRef: string
  rollbackFromBundleRef?: string
  rollbackToBundleRef?: string
  sourceRefs?: ReadonlyArray<string>
}>

export type BuildReactorDogfoodRunReceiptInput = Readonly<{
  caseStudyWriteupRef: string
  generatedAt: string
  installOpsReceipt: ReactorInstallOpsReceipt
  measuredWindowEndedAt: string
  measuredWindowStartedAt: string
  meteringReceipts: ReadonlyArray<ReactorLocalTokenMeteringReceipt>
  nodeProfile: ReactorNodeModelProfile
  policy: ReactorModelPolicy
  receiptRef: string
  refusedNonconformingInstallOpsReceipt: ReactorInstallOpsReceipt
  routeReceipts: ReadonlyArray<ReactorRouteDecisionReceipt>
  sourceRefs?: ReadonlyArray<string>
  workloadRef: string
}>

export type EvaluateReactorNeedToKnowAccessInput = Readonly<{
  decidedAt: string
  documents: ReadonlyArray<ReactorCorpusDocumentRecord>
  oracleVerdicts?: ReadonlyArray<ReactorNeedToKnowOracleVerdict>
  receiptRef: string
  request: ReactorCorpusRetrievalRequest
  ruleSet: ReactorNeedToKnowRuleSet
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

const evalHarnessProfile = (
  record: Omit<ReactorEvalHarnessProfile, 'schemaVersion'>,
): ReactorEvalHarnessProfile =>
  S.decodeUnknownSync(ReactorEvalHarnessProfile)({
    schemaVersion: 'openagents.reactor.eval_harness_profile.v1',
    ...record,
  })

export const REACTOR_PSIONIC_EVAL_HARNESS_PROFILE = evalHarnessProfile({
  harnessRef: 'reactor.eval_harness.psionic.task_class.v1',
  runnerOwner: 'psionic',
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8274',
    'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#5-quick-win-workstreams',
  ],
  supportedExecutionTargets: [
    'rx3_served_model',
    'hosted_equivalent_large_model',
    'not_measured',
  ],
  taskClassRefs: [...REACTOR_EVAL_TASK_CLASS_REFS],
  unrunMeasurementState: 'not_measured',
})

const validEvalScore = (value: number): boolean =>
  Number.isFinite(value) && value >= 0 && value <= 1

const validEvalSampleCount = (value: number): boolean =>
  Number.isInteger(value) && value > 0

export const buildReactorModelEvalReceipt = (
  input: BuildReactorModelEvalReceiptInput,
): ReactorModelEvalReceipt => {
  const catalog = S.decodeUnknownSync(ReactorModelCatalog)(input.catalog)
  const harnessProfile = S.decodeUnknownSync(ReactorEvalHarnessProfile)(
    input.harnessProfile ?? REACTOR_PSIONIC_EVAL_HARNESS_PROFILE,
  )
  const taskClassRef = S.decodeUnknownSync(ReactorEvalTaskClassRef)(
    input.taskClassRef,
  )
  const modelExists = catalog.models.some(model => model.modelRef === input.modelRef)

  if (!modelExists) {
    throw new Error('reactor.eval.model_not_in_catalog')
  }
  if (!harnessProfile.taskClassRefs.includes(taskClassRef)) {
    throw new Error('reactor.eval.task_class_not_supported_by_harness')
  }
  if (input.measurement.state === 'measured') {
    if (
      !validEvalScore(input.measurement.score) ||
      !validEvalSampleCount(input.measurement.sampleCount)
    ) {
      throw new Error('reactor.eval.measured_receipt_invalid_score_or_samples')
    }
    if (
      !harnessProfile.supportedExecutionTargets.includes(
        input.measurement.executionTarget,
      )
    ) {
      throw new Error('reactor.eval.execution_target_not_supported_by_harness')
    }
  }

  return S.decodeUnknownSync(ReactorModelEvalReceipt)({
    schemaVersion: 'openagents.reactor.model_eval_receipt.v1',
    blockerRefs:
      input.measurement.state === 'measured'
        ? []
        : ['blocker.reactor.eval.not_measured', input.measurement.reasonRef],
    capabilityCopyAllowed: input.measurement.state === 'measured',
    evalDatasetRef:
      input.measurement.state === 'measured'
        ? input.measurement.evalDatasetRef
        : (input.measurement.evalDatasetRef ??
          'dataset.reactor.task_class_eval.not_measured'),
    executionTarget:
      input.measurement.state === 'measured'
        ? input.measurement.executionTarget
        : 'not_measured',
    generatedAt: input.generatedAt,
    harnessRef: harnessProfile.harnessRef,
    measurementState:
      input.measurement.state === 'measured' ? 'measured' : 'not_measured',
    modelRef: input.modelRef,
    receiptRef: input.receiptRef,
    runnerOwner: harnessProfile.runnerOwner,
    sampleCount:
      input.measurement.state === 'measured'
        ? input.measurement.sampleCount
        : null,
    score: input.measurement.state === 'measured' ? input.measurement.score : null,
    scoreUnit:
      input.measurement.state === 'measured'
        ? input.measurement.scoreUnit
        : null,
    sourceRefs: unique([
      catalog.catalogRef,
      harnessProfile.harnessRef,
      ...(input.sourceRefs ?? []),
    ]),
    taskClassRef,
  })
}

const evalReceiptKey = (
  modelRef: string,
  taskClassRef: ReactorEvalTaskClassRef,
): string => `${modelRef}::${taskClassRef}`

const measuredEvalReceiptByKey = (
  evalReceipts: ReadonlyArray<ReactorModelEvalReceipt>,
): ReadonlyMap<string, ReactorModelEvalReceipt> =>
  evalReceipts.reduce((map, receiptLike) => {
    const receipt = S.decodeUnknownSync(ReactorModelEvalReceipt)(receiptLike)
    if (
      receipt.measurementState !== 'measured' ||
      receipt.capabilityCopyAllowed !== true
    ) {
      return map
    }

    const key = evalReceiptKey(receipt.modelRef, receipt.taskClassRef)
    if (map.has(key)) {
      throw new Error('reactor.eval.duplicate_measured_receipt_for_model_task')
    }

    map.set(key, receipt)
    return map
  }, new Map<string, ReactorModelEvalReceipt>())

export const buildReactorEvalCoverageMatrix = (
  input: BuildReactorEvalCoverageMatrixInput,
): ReactorEvalCoverageMatrix => {
  const catalog = S.decodeUnknownSync(ReactorModelCatalog)(input.catalog)
  const taskClassRefs = unique(
    [...(input.taskClassRefs ?? REACTOR_EVAL_TASK_CLASS_REFS)].map(taskClassRef =>
      S.decodeUnknownSync(ReactorEvalTaskClassRef)(taskClassRef),
    ),
  )
  const receiptsByKey = measuredEvalReceiptByKey(input.evalReceipts)
  const cells = catalog.models.flatMap(modelRecord =>
    taskClassRefs.map(taskClassRef => {
      const receipt = receiptsByKey.get(
        evalReceiptKey(modelRecord.modelRef, taskClassRef),
      )

      if (receipt !== undefined) {
        return {
          blockerRefs: [],
          capabilityCopyAllowed: true,
          measurementState: 'measured' as const,
          modelRef: modelRecord.modelRef,
          receiptRef: receipt.receiptRef,
          score: receipt.score,
          taskClassRef,
        } satisfies ReactorEvalCoverageCell
      }

      return {
        blockerRefs: [
          'blocker.reactor.eval.not_measured',
          `model:${modelRecord.modelRef}`,
          `task_class:${taskClassRef}`,
        ],
        capabilityCopyAllowed: false,
        measurementState: 'not_measured' as const,
        modelRef: modelRecord.modelRef,
        receiptRef: null,
        score: null,
        taskClassRef,
      } satisfies ReactorEvalCoverageCell
    }),
  )

  return S.decodeUnknownSync(ReactorEvalCoverageMatrix)({
    schemaVersion: 'openagents.reactor.eval_coverage_matrix.v1',
    catalogRef: catalog.catalogRef,
    cells,
    generatedAt: input.generatedAt,
    matrixRef: input.matrixRef,
    sourceRefs: unique([
      catalog.catalogRef,
      ...input.evalReceipts.map(receipt => receipt.receiptRef),
      ...(input.sourceRefs ?? []),
    ]),
  })
}

export const selectReactorCapabilityCopyEvalRefs = (
  input: SelectReactorCapabilityCopyEvalRefsInput,
): ReactorCapabilityCopyEvalDecision => {
  const taskClassRefs = unique(
    input.taskClassRefs.map(taskClassRef =>
      S.decodeUnknownSync(ReactorEvalTaskClassRef)(taskClassRef),
    ),
  )
  const receiptsByKey = measuredEvalReceiptByKey(input.evalReceipts)
  const allowedEvalRefs = taskClassRefs.flatMap(taskClassRef => {
    const receipt = receiptsByKey.get(evalReceiptKey(input.modelRef, taskClassRef))
    return receipt === undefined ? [] : [receipt.receiptRef]
  })
  const blockerRefs = taskClassRefs.flatMap(taskClassRef => {
    const receipt = receiptsByKey.get(evalReceiptKey(input.modelRef, taskClassRef))
    return receipt === undefined
      ? [
          'blocker.reactor.capability_copy.eval_not_measured',
          `model:${input.modelRef}`,
          `task_class:${taskClassRef}`,
        ]
      : []
  })

  return S.decodeUnknownSync(ReactorCapabilityCopyEvalDecision)({
    schemaVersion: 'openagents.reactor.capability_copy_eval_decision.v1',
    allowedEvalRefs,
    blockerRefs,
    decidedAt: input.decidedAt,
    decisionRef: input.decisionRef,
    modelRef: input.modelRef,
    sourceRefs: unique([
      ...allowedEvalRefs,
      ...(input.sourceRefs ?? []),
    ]),
    status: blockerRefs.length === 0 ? 'allowed' : 'blocked_not_measured',
    taskClassRefs,
  })
}

export const REACTOR_AIRGAP_BUNDLE_VERIFIER_REF =
  'apps/oa-updates/scripts/verify-release.ts'

export const REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF =
  'apps/oa-updates/keys/release-pubkey.json'

export const buildReactorAirgapUpdateBundleManifest = (
  input: BuildReactorAirgapUpdateBundleManifestInput,
): ReactorAirgapUpdateBundleManifest => {
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )

  return S.decodeUnknownSync(ReactorAirgapUpdateBundleManifest)({
    schemaVersion: 'openagents.reactor.airgap_update_bundle_manifest.v1',
    artifactSha256: input.artifactSha256,
    bundleRef: input.bundleRef,
    bundleVersion: input.bundleVersion,
    callbackRequired: false,
    createdAt: input.createdAt,
    modelRef: input.modelRef,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    policyRef: input.policyRef,
    policyVersion: input.policyVersion,
    publicKeyRef: input.publicKeyRef ?? REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF,
    signatureAlg: 'ed25519',
    signatureKid: input.signatureKid,
    signatureRef: input.signatureRef,
    sourceRefs: unique([
      nodeProfile.nodeProfileRef,
      input.signatureRef,
      input.publicKeyRef ?? REACTOR_AIRGAP_BUNDLE_PUBLIC_KEY_REF,
      input.verifierRef ?? REACTOR_AIRGAP_BUNDLE_VERIFIER_REF,
      ...(input.sourceRefs ?? []),
    ]),
    verifierRef: input.verifierRef ?? REACTOR_AIRGAP_BUNDLE_VERIFIER_REF,
  })
}

const installOpsProvisionAction = (
  action: ReactorInstallOpsReceipt['action'],
): ReactorModelInstallReceipt['action'] =>
  action === 'fresh_install' ? 'install' : 'upgrade'

export const buildReactorInstallOpsReceipt = (
  input: BuildReactorInstallOpsReceiptInput,
): ReactorInstallOpsReceipt => {
  const bundle = S.decodeUnknownSync(ReactorAirgapUpdateBundleManifest)(
    input.bundle,
  )
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )
  const policy = S.decodeUnknownSync(ReactorModelPolicy)(input.policy)
  const installReceipt = provisionReactorModel({
    action: installOpsProvisionAction(input.action),
    artifactRefs: [bundle.bundleRef, `sha256:${bundle.artifactSha256}`],
    catalog: input.catalog,
    decidedAt: input.decidedAt,
    decisionRef: `${input.receiptRef}.policy_decision`,
    nodeProfile,
    policy,
    receiptRef: `${input.receiptRef}.model_install`,
    sourceRefs: [bundle.bundleRef, ...(input.sourceRefs ?? [])],
  })
  const blockerRefs = unique([
    ...(bundle.callbackRequired === false
      ? []
      : ['blocker.reactor.airgap_update.callback_required']),
    ...(bundle.nodeProfileRef === nodeProfile.nodeProfileRef &&
    bundle.modelRef === nodeProfile.modelRef
      ? []
      : ['blocker.reactor.airgap_update.profile_binding_mismatch']),
    ...(bundle.policyRef === policy.policyRef &&
    bundle.policyVersion === policy.version
      ? []
      : ['blocker.reactor.airgap_update.policy_binding_mismatch']),
    ...(installReceipt.status === 'installed'
      ? []
      : [
          'blocker.reactor.install_ops.policy_revalidation_failed',
          ...(installReceipt.refusal?.blockerRefs ?? []),
        ]),
  ])

  return S.decodeUnknownSync(ReactorInstallOpsReceipt)({
    schemaVersion: 'openagents.reactor.install_ops_receipt.v1',
    action: input.action,
    blockerRefs,
    bundleRef: bundle.bundleRef,
    decidedAt: input.decidedAt,
    modelInstallReceiptRef: installReceipt.receiptRef,
    modelRef: nodeProfile.modelRef,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    policyDecisionRef: installReceipt.policyDecisionRef,
    policyRef: policy.policyRef,
    policyVersion: policy.version,
    receiptRef: input.receiptRef,
    rollbackFromBundleRef:
      input.action === 'rollback' ? (input.rollbackFromBundleRef ?? null) : null,
    rollbackToBundleRef:
      input.action === 'rollback'
        ? (input.rollbackToBundleRef ?? bundle.bundleRef)
        : null,
    sourceRefs: unique([
      bundle.bundleRef,
      bundle.signatureRef,
      installReceipt.receiptRef,
      installReceipt.policyDecisionRef,
      ...(input.sourceRefs ?? []),
    ]),
    status: blockerRefs.length === 0 ? 'succeeded' : 'refused',
    verificationRefs: unique([
      bundle.verifierRef,
      bundle.publicKeyRef,
      bundle.signatureRef,
      installReceipt.policyDecisionRef,
    ]),
  })
}

const strictPolicyConstraintRefs = (
  policy: ReactorModelPolicy,
): ReadonlyArray<string> =>
  unique([
    ...(policy.constraints.allowOriginJurisdictions ?? []).map(
      jurisdiction => `constraint.origin_jurisdiction:${jurisdiction}`,
    ),
    policy.constraints.enforceDistillationLineageJurisdiction
      ? 'constraint.distillation_lineage_jurisdiction:enforced'
      : 'constraint.distillation_lineage_jurisdiction:not_enforced',
  ])

const policyIsUsOnly = (policy: ReactorModelPolicy): boolean =>
  policy.constraints.enforceDistillationLineageJurisdiction &&
  policy.constraints.allowOriginJurisdictions?.length === 1 &&
  policy.constraints.allowOriginJurisdictions[0] === 'us'

export const buildReactorDogfoodRunReceipt = (
  input: BuildReactorDogfoodRunReceiptInput,
): ReactorDogfoodRunReceipt => {
  const nodeProfile = S.decodeUnknownSync(ReactorNodeModelProfile)(
    input.nodeProfile,
  )
  const policy = S.decodeUnknownSync(ReactorModelPolicy)(input.policy)
  const installOpsReceipt = S.decodeUnknownSync(ReactorInstallOpsReceipt)(
    input.installOpsReceipt,
  )
  const refusedNonconformingInstallOpsReceipt = S.decodeUnknownSync(
    ReactorInstallOpsReceipt,
  )(input.refusedNonconformingInstallOpsReceipt)
  const routeReceipts = input.routeReceipts.map(receipt =>
    S.decodeUnknownSync(ReactorRouteDecisionReceipt)(receipt),
  )
  const meteringReceipts = input.meteringReceipts.map(receipt =>
    S.decodeUnknownSync(ReactorLocalTokenMeteringReceipt)(receipt),
  )
  const routeRequestRefs = unique(routeReceipts.map(receipt => receipt.requestRef))
  const meteringRequestRefs = new Set(
    meteringReceipts.map(receipt => receipt.requestRef),
  )
  const missingMeteringRequestRefs = routeRequestRefs.filter(
    requestRef => !meteringRequestRefs.has(requestRef),
  )
  const totalMeasuredTokens = meteringReceipts.reduce(
    (total, receipt) => total + (receipt.totalTokens ?? 0),
    0,
  )
  const blockerRefs = unique([
    ...(nodeProfile.placement === 'dogfood'
      ? []
      : ['blocker.reactor.dogfood.node_not_dogfood_placement']),
    ...(nodeProfile.policyRef === policy.policyRef &&
    nodeProfile.policyVersion === policy.version
      ? []
      : ['blocker.reactor.dogfood.profile_policy_binding_mismatch']),
    ...(policyIsUsOnly(policy)
      ? []
      : ['blocker.reactor.dogfood.strict_us_only_policy_missing']),
    ...(installOpsReceipt.status === 'succeeded'
      ? []
      : ['blocker.reactor.dogfood.install_ops_not_succeeded']),
    ...(installOpsReceipt.nodeProfileRef === nodeProfile.nodeProfileRef &&
    installOpsReceipt.modelRef === nodeProfile.modelRef &&
    installOpsReceipt.policyRef === policy.policyRef &&
    installOpsReceipt.policyVersion === policy.version
      ? []
      : ['blocker.reactor.dogfood.install_ops_binding_mismatch']),
    ...(refusedNonconformingInstallOpsReceipt.status === 'refused'
      ? []
      : ['blocker.reactor.dogfood.nonconforming_pull_not_refused']),
    ...(refusedNonconformingInstallOpsReceipt.blockerRefs.includes(
      'blocker.reactor.install_ops.policy_revalidation_failed',
    )
      ? []
      : ['blocker.reactor.dogfood.nonconforming_pull_not_policy_refused']),
    ...(refusedNonconformingInstallOpsReceipt.modelRef !== nodeProfile.modelRef
      ? []
      : ['blocker.reactor.dogfood.refused_model_matches_served_model']),
    ...routeReceipts.flatMap(receipt => [
      ...(receipt.status === 'routed'
        ? []
        : [
            'blocker.reactor.dogfood.route_not_routed',
            ...receipt.blockerRefs,
          ]),
      ...(receipt.nodeProfileRef === nodeProfile.nodeProfileRef &&
      receipt.policyRef === policy.policyRef &&
      receipt.policyVersion === policy.version
        ? []
        : ['blocker.reactor.dogfood.route_binding_mismatch']),
    ]),
    ...meteringReceipts.flatMap(receipt => [
      ...(receipt.nodeProfileRef === nodeProfile.nodeProfileRef &&
      receipt.policyRef === policy.policyRef &&
      receipt.policyVersion === policy.version
        ? []
        : ['blocker.reactor.dogfood.metering_binding_mismatch']),
      ...(receipt.measurementState === 'measured' &&
      receipt.usageTruth === 'exact' &&
      receipt.totalTokens !== null
        ? []
        : ['blocker.reactor.dogfood.local_metering_not_exact']),
    ]),
    ...missingMeteringRequestRefs.map(
      requestRef => `blocker.reactor.dogfood.missing_metering:${requestRef}`,
    ),
  ])

  return S.decodeUnknownSync(ReactorDogfoodRunReceipt)({
    schemaVersion: 'openagents.reactor.dogfood_run_receipt.v1',
    blockerRefs,
    caseStudyWriteupRef: input.caseStudyWriteupRef,
    externalClaimFlipAllowed: false,
    externalPilotAuthorized: false,
    generatedAt: input.generatedAt,
    hardwareOwnerRef: 'owner.openagents',
    installOpsReceiptRef: installOpsReceipt.receiptRef,
    localMeteringReceiptRefs: meteringReceipts.map(receipt => receipt.receiptRef),
    measuredWindowEndedAt: input.measuredWindowEndedAt,
    measuredWindowStartedAt: input.measuredWindowStartedAt,
    nodeProfileRef: nodeProfile.nodeProfileRef,
    placement: nodeProfile.placement,
    policyConstraintRefs: strictPolicyConstraintRefs(policy),
    policyDecisionReceiptRefs: unique([
      installOpsReceipt.policyDecisionRef,
      refusedNonconformingInstallOpsReceipt.policyDecisionRef,
      ...routeReceipts.map(receipt => receipt.policyDecisionRef),
    ]),
    policyRef: policy.policyRef,
    policyVersion: policy.version,
    publicSafe: true,
    receiptRef: input.receiptRef,
    refusedNonconformingInstallOpsReceiptRef:
      refusedNonconformingInstallOpsReceipt.receiptRef,
    refusedNonconformingModelRef: refusedNonconformingInstallOpsReceipt.modelRef,
    requestRefs: routeRequestRefs,
    routeDecisionRefs: routeReceipts.map(receipt => receipt.decisionRef),
    sourceRefs: unique([
      nodeProfile.nodeProfileRef,
      policy.policyRef,
      installOpsReceipt.receiptRef,
      refusedNonconformingInstallOpsReceipt.receiptRef,
      input.caseStudyWriteupRef,
      ...routeReceipts.map(receipt => receipt.decisionRef),
      ...meteringReceipts.map(receipt => receipt.receiptRef),
      ...(input.sourceRefs ?? []),
    ]),
    status: blockerRefs.length === 0 ? 'completed' : 'refused',
    strictPolicyRef: policy.policyRef,
    totalMeasuredTokens,
    workloadClass: 'internal_lead_gen_case_study_seed',
    workloadRef: input.workloadRef,
    workloadTruth: 'internal_openagents',
  })
}

const intersects = <T extends string>(
  left: ReadonlyArray<T>,
  right: ReadonlyArray<T>,
): boolean => left.some(value => right.includes(value))

const hardAccessBlockerRefs = (
  request: ReactorCorpusRetrievalRequest,
  document: ReactorCorpusDocumentRecord,
): ReadonlyArray<string> =>
  unique([
    ...(request.subject.workspaceRefs.includes(document.workspaceRef) &&
    request.workspaceRef === document.workspaceRef
      ? []
      : ['blocker.reactor.need_to_know.workspace_scope_mismatch']),
    ...(request.subject.matterRefs.includes(document.matterRef) &&
    request.matterRef === document.matterRef
      ? []
      : ['blocker.reactor.need_to_know.matter_scope_mismatch']),
    ...(intersects(request.subject.roleRefs, document.allowedRoleRefs) ||
    document.allowedUserRefs.includes(request.subject.subjectUserRef)
      ? []
      : ['blocker.reactor.need_to_know.role_or_user_scope_missing']),
  ])

const oracleBlockerRefs = (
  verdict: ReactorNeedToKnowOracleVerdict | undefined,
): ReadonlyArray<string> => {
  if (verdict === undefined) {
    return ['blocker.reactor.need_to_know.oracle_verdict_missing']
  }

  return verdict.verdict === 'need_to_know_plausible'
    ? []
    : [
        'blocker.reactor.need_to_know.oracle_not_plausible',
        ...verdict.reasonRefs,
      ]
}

export const evaluateReactorNeedToKnowAccess = (
  input: EvaluateReactorNeedToKnowAccessInput,
): ReactorCorpusAccessDecisionReceipt => {
  const ruleSet = S.decodeUnknownSync(ReactorNeedToKnowRuleSet)(input.ruleSet)
  const request = S.decodeUnknownSync(ReactorCorpusRetrievalRequest)(
    input.request,
  )
  const documents = input.documents.map(document =>
    S.decodeUnknownSync(ReactorCorpusDocumentRecord)(document),
  )
  const oracleVerdicts = new Map(
    (input.oracleVerdicts ?? []).map(verdictLike => {
      const verdict = S.decodeUnknownSync(ReactorNeedToKnowOracleVerdict)(
        verdictLike,
      )
      return [verdict.documentRef, verdict] as const
    }),
  )
  const documentsByRef = new Map(
    documents.map(document => [document.documentRef, document] as const),
  )
  const documentDecisions = request.requestedDocumentRefs.map(documentRef => {
    const document = documentsByRef.get(documentRef)

    if (document === undefined) {
      return {
        blockerRefs: ['blocker.reactor.need_to_know.document_not_found'],
        citationRefs: [],
        decisionRef: `${input.receiptRef}.decision.${documentRef}`,
        documentRef,
        hardDecisionStatus: 'failed' as const,
        hardRuleRefs: ruleSet.hardRuleRefs,
        oracleDecisionStatus: 'skipped_hard_denied' as const,
        oracleVerdictRef: null,
        reasonRefs: ['reason.reactor.need_to_know.document_not_found'],
        status: 'denied_hard_rule' as const,
      } satisfies ReactorNeedToKnowDocumentDecision
    }

    const hardBlockers = hardAccessBlockerRefs(request, document)
    const hardPassed = hardBlockers.length === 0
    const oracleVerdict = oracleVerdicts.get(document.documentRef)

    if (!hardPassed) {
      return {
        blockerRefs: hardBlockers,
        citationRefs: [],
        decisionRef: `${input.receiptRef}.decision.${document.documentRef}`,
        documentRef: document.documentRef,
        hardDecisionStatus: 'failed' as const,
        hardRuleRefs: ruleSet.hardRuleRefs,
        oracleDecisionStatus: 'skipped_hard_denied' as const,
        oracleVerdictRef: oracleVerdict?.verdictRef ?? null,
        reasonRefs: hardBlockers,
        status: 'denied_hard_rule' as const,
      } satisfies ReactorNeedToKnowDocumentDecision
    }

    const softBlockers = oracleBlockerRefs(oracleVerdict)
    const softPassed = softBlockers.length === 0

    return {
      blockerRefs: softBlockers,
      citationRefs: softPassed ? document.citationRefs : [],
      decisionRef: `${input.receiptRef}.decision.${document.documentRef}`,
      documentRef: document.documentRef,
      hardDecisionStatus: 'passed' as const,
      hardRuleRefs: ruleSet.hardRuleRefs,
      oracleDecisionStatus: softPassed ? ('passed' as const) : ('failed' as const),
      oracleVerdictRef: oracleVerdict?.verdictRef ?? null,
      reasonRefs: softPassed ? ['reason.reactor.need_to_know.allowed'] : softBlockers,
      status: softPassed ? ('allowed' as const) : ('denied_soft_oracle' as const),
    } satisfies ReactorNeedToKnowDocumentDecision
  })
  const selectedDocumentRefs = documentDecisions
    .filter(decision => decision.status === 'allowed')
    .map(decision => decision.documentRef)
  const deniedDocumentRefs = documentDecisions
    .filter(decision => decision.status !== 'allowed')
    .map(decision => decision.documentRef)
  const selectedCitationRefs = unique(
    documentDecisions.flatMap(decision => decision.citationRefs),
  )
  const deniedCitationRefs = unique(
    deniedDocumentRefs.flatMap(documentRef => {
      const document = documentsByRef.get(documentRef)
      return document === undefined ? [] : document.citationRefs
    }),
  )

  return S.decodeUnknownSync(ReactorCorpusAccessDecisionReceipt)({
    schemaVersion: 'openagents.reactor.corpus_access_decision_receipt.v1',
    decidedAt: input.decidedAt,
    deniedCitationRefs,
    deniedDocumentRefs,
    documentDecisions,
    generatedSummaryContentLogged: false,
    matterRef: request.matterRef,
    oracleAppliedAfterHardRules: true,
    oracleVerdictRefs: unique(
      [...oracleVerdicts.values()].map(verdict => verdict.verdictRef),
    ),
    outputMode: request.outputMode,
    queryIntentRef: request.queryIntentRef,
    rawDocumentContentLogged: false,
    receiptRef: input.receiptRef,
    requestedDocumentRefs: request.requestedDocumentRefs,
    ruleSetRef: ruleSet.ruleSetRef,
    ruleSetVersion: ruleSet.version,
    selectedCitationRefs,
    selectedDocumentRefs,
    sourceRefs: unique([
      ruleSet.ruleSetRef,
      request.requestRef,
      ...documentDecisions.map(decision => decision.decisionRef),
      ...(input.sourceRefs ?? []),
    ]),
    subjectUserRef: request.subject.subjectUserRef,
    workspaceRef: request.workspaceRef,
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
    'github:OpenAgentsInc/openagents#8274',
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
      evalRefs: [
        'reactor.eval_receipt.llama.drafting.fixture.20260704',
        'reactor.eval_receipt.llama.extraction.fixture.20260704',
      ],
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
      evalRefs: [
        'reactor.eval_receipt.gpt_oss.drafting.fixture.20260704',
        'reactor.eval_receipt.gpt_oss.extraction.fixture.20260704',
      ],
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

export const REACTOR_MODEL_EVAL_RECEIPT_SEED = [
  buildReactorModelEvalReceipt({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    generatedAt: '2026-07-04T12:15:00.000Z',
    measurement: {
      evalDatasetRef: 'dataset.reactor.fixture.drafting.v1',
      executionTarget: 'rx3_served_model',
      sampleCount: 12,
      score: 0.82,
      scoreUnit: 'score_0_to_1',
      state: 'measured',
    },
    modelRef: 'model.openai.gpt_oss.open_family',
    receiptRef: 'reactor.eval_receipt.gpt_oss.drafting.fixture.20260704',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8274',
      'reactor.node_profile.fixture.hydralisk.server_class.v1',
    ],
    taskClassRef: 'drafting',
  }),
  buildReactorModelEvalReceipt({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    generatedAt: '2026-07-04T12:16:00.000Z',
    measurement: {
      evalDatasetRef: 'dataset.reactor.fixture.extraction.v1',
      executionTarget: 'rx3_served_model',
      sampleCount: 12,
      score: 0.88,
      scoreUnit: 'score_0_to_1',
      state: 'measured',
    },
    modelRef: 'model.openai.gpt_oss.open_family',
    receiptRef: 'reactor.eval_receipt.gpt_oss.extraction.fixture.20260704',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8274',
      'reactor.node_profile.fixture.hydralisk.server_class.v1',
    ],
    taskClassRef: 'extraction',
  }),
  buildReactorModelEvalReceipt({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    generatedAt: '2026-07-04T12:17:00.000Z',
    measurement: {
      evalDatasetRef: 'dataset.reactor.fixture.drafting.v1',
      executionTarget: 'hosted_equivalent_large_model',
      sampleCount: 12,
      score: 0.79,
      scoreUnit: 'score_0_to_1',
      state: 'measured',
    },
    modelRef: 'model.meta.llama.open_family',
    receiptRef: 'reactor.eval_receipt.llama.drafting.fixture.20260704',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8274',
      'label:hosted_equivalent_large_model',
    ],
    taskClassRef: 'drafting',
  }),
  buildReactorModelEvalReceipt({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    generatedAt: '2026-07-04T12:18:00.000Z',
    measurement: {
      evalDatasetRef: 'dataset.reactor.fixture.extraction.v1',
      executionTarget: 'hosted_equivalent_large_model',
      sampleCount: 12,
      score: 0.84,
      scoreUnit: 'score_0_to_1',
      state: 'measured',
    },
    modelRef: 'model.meta.llama.open_family',
    receiptRef: 'reactor.eval_receipt.llama.extraction.fixture.20260704',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8274',
      'label:hosted_equivalent_large_model',
    ],
    taskClassRef: 'extraction',
  }),
] as const satisfies ReadonlyArray<ReactorModelEvalReceipt>

export const REACTOR_EVAL_COVERAGE_MATRIX_SEED =
  buildReactorEvalCoverageMatrix({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    evalReceipts: REACTOR_MODEL_EVAL_RECEIPT_SEED,
    generatedAt: '2026-07-04T12:20:00.000Z',
    matrixRef: 'reactor.eval_coverage_matrix.seed.20260704.v1',
    sourceRefs: ['github:OpenAgentsInc/openagents#8274'],
  })

const hardwareTierSpec = (
  record: Omit<ReactorHardwareTierSpec, 'schemaVersion'>,
): ReactorHardwareTierSpec =>
  S.decodeUnknownSync(ReactorHardwareTierSpec)({
    schemaVersion: 'openagents.reactor.hardware_tier_spec.v1',
    ...record,
  })

export const REACTOR_HARDWARE_TIER_SPECS = [
  hardwareTierSpec({
    cpu: '16+ modern x86_64 or arm64 cores',
    guidanceOnly: true,
    memory: '128 GB system memory minimum for workstation-class pilots',
    network: '1 GbE management; 10 GbE preferred for corpus ingest',
    notes: [
      'Guidance only: no purchase commitment and no availability claim.',
      'Use for one-operator dogfood or small customer pilot planning.',
    ],
    storage: '2 TB NVMe for weights, bundle cache, receipts, and rollback slot',
    tierRef: 'workstation',
  }),
  hardwareTierSpec({
    cpu: '32+ server cores with ECC memory',
    guidanceOnly: true,
    memory: '256-512 GB ECC system memory depending on served model family',
    network: '10 GbE minimum for controlled corpus ingest and local clients',
    notes: [
      'Guidance only: final sizing depends on model, quantization, context, and concurrency.',
      'Prefer redundant OS and receipt storage before customer pilots.',
    ],
    storage: '4-8 TB NVMe with separate rollback bundle retention',
    tierRef: 'server',
  }),
  hardwareTierSpec({
    cpu: 'rack server platform sized by GPU count and thermal envelope',
    guidanceOnly: true,
    memory: '512 GB+ ECC system memory for multi-GPU serving and larger corpora',
    network: '25 GbE+ east/west or customer-controlled fabric',
    notes: [
      'Guidance only: requires site power, cooling, remote hands, and customer change-control review.',
      'Do not treat this as a quoted bill of materials without owner approval.',
    ],
    storage: '8 TB+ NVMe plus offline signed-bundle archive and rollback retention',
    tierRef: 'rack',
  }),
] as const satisfies ReadonlyArray<ReactorHardwareTierSpec>

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

const DOGFOOD_DECIDED_AT = '2026-07-04T14:10:00.000Z'

export const REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE = nodeProfile({
  displayName: 'OpenAgents dogfood Reactor Hydralisk profile',
  exactLocalMeteringRequired: true,
  gateway: {
    endpointRef: 'endpoint.reactor.openagents.dogfood.openai_compatible.local',
    phoneHomeAllowedInServingPath: false,
    protocol: 'openai.chat_completions.v1',
    servingPathNetwork: 'offline_once_provisioned',
  },
  modelRef: 'model.openai.gpt_oss.open_family',
  nodeProfileRef: 'reactor.node_profile.openagents.dogfood.hydralisk.v1',
  placement: 'dogfood',
  policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
  policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
  servingLane: 'hydralisk',
  servingStack: {
    engineRef: 'engine.hydralisk.vllm.openagents.dogfood',
    imageDigestRef: 'image.hydralisk.vllm.openagents.dogfood.sha256',
    kind: 'vllm',
    modelArtifactRefs: ['artifact.openagents.dogfood.gpt_oss.open_family.weights'],
    versionRef: 'vllm.openagents.dogfood.20260704',
  },
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8276',
    'docs/fable/2026-07-04-rx-5-reactor-install-airgap-runbook.md',
  ],
})

export const REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE =
  buildReactorAirgapUpdateBundleManifest({
    artifactSha256:
      'd0a6f9e2a4fcd1b91f4a2a6726d0c36922cfb6e6b6d312d7bfb28f0d1a0f8276',
    bundleRef: 'reactor.airgap_bundle.openagents.dogfood.gpt_oss.20260704',
    bundleVersion: '2026-07-04.rx6.dogfood',
    createdAt: DOGFOOD_DECIDED_AT,
    modelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
    policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
    signatureKid: 'openagents-release-ed25519-20260704',
    signatureRef: 'reactor.airgap_bundle.openagents.dogfood.gpt_oss.20260704.sig',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8276',
      'docs/fable/2026-07-04-rx-5-reactor-install-airgap-runbook.md',
    ],
  })

export const REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT =
  buildReactorInstallOpsReceipt({
    action: 'fresh_install',
    bundle: REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE,
    catalog: REACTOR_MODEL_CATALOG_SEED,
    decidedAt: DOGFOOD_DECIDED_AT,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    receiptRef: 'reactor.install_ops.openagents.dogfood.fresh.gpt_oss.20260704',
    sourceRefs: ['github:OpenAgentsInc/openagents#8276'],
  })

export const REACTOR_OPENAGENTS_DOGFOOD_MODEL_INSTALL_RECEIPT =
  provisionReactorModel({
    action: 'install',
    artifactRefs: [
      REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE.bundleRef,
      `sha256:${REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE.artifactSha256}`,
    ],
    catalog: REACTOR_MODEL_CATALOG_SEED,
    decidedAt: DOGFOOD_DECIDED_AT,
    decisionRef:
      'reactor.install_ops.openagents.dogfood.fresh.gpt_oss.20260704.policy_decision',
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    receiptRef:
      'reactor.install_ops.openagents.dogfood.fresh.gpt_oss.20260704.model_install',
    sourceRefs: [REACTOR_OPENAGENTS_DOGFOOD_AIRGAP_BUNDLE.bundleRef],
  })

export const REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED = [
  routeReactorOpenAiCompatibleRequest({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    decidedAt: '2026-07-04T14:12:00.000Z',
    decisionRef: 'reactor.route_decision.openagents.dogfood.discovery.20260704',
    installReceipt: REACTOR_OPENAGENTS_DOGFOOD_MODEL_INSTALL_RECEIPT,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    requestRef: 'reactor.request.openagents.dogfood.lead_gen.discovery.20260704',
    requestedModelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
    sourceRefs: ['workload.openagents.lead_gen_reactor.case_study_seed.20260704'],
  }),
  routeReactorOpenAiCompatibleRequest({
    catalog: REACTOR_MODEL_CATALOG_SEED,
    decidedAt: '2026-07-04T14:14:00.000Z',
    decisionRef: 'reactor.route_decision.openagents.dogfood.sequence.20260704',
    installReceipt: REACTOR_OPENAGENTS_DOGFOOD_MODEL_INSTALL_RECEIPT,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    requestRef: 'reactor.request.openagents.dogfood.lead_gen.sequence.20260704',
    requestedModelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
    sourceRefs: ['workload.openagents.lead_gen_reactor.case_study_seed.20260704'],
  }),
] as const satisfies ReadonlyArray<ReactorRouteDecisionReceipt>

export const REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED = [
  buildReactorLocalTokenMeteringReceipt({
    generatedAt: '2026-07-04T14:12:30.000Z',
    modelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
    policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
    receiptRef: 'reactor.local_metering.openagents.dogfood.discovery.20260704',
    requestRef: 'reactor.request.openagents.dogfood.lead_gen.discovery.20260704',
    sourceRefs: ['reactor.route_decision.openagents.dogfood.discovery.20260704'],
    usage: {
      completionTokens: 69,
      promptTokens: 243,
      state: 'exact',
      totalTokens: 312,
    },
  }),
  buildReactorLocalTokenMeteringReceipt({
    generatedAt: '2026-07-04T14:14:30.000Z',
    modelRef: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE.modelRef,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
    policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
    receiptRef: 'reactor.local_metering.openagents.dogfood.sequence.20260704',
    requestRef: 'reactor.request.openagents.dogfood.lead_gen.sequence.20260704',
    sourceRefs: ['reactor.route_decision.openagents.dogfood.sequence.20260704'],
    usage: {
      completionTokens: 112,
      promptTokens: 319,
      state: 'exact',
      totalTokens: 431,
    },
  }),
] as const satisfies ReadonlyArray<ReactorLocalTokenMeteringReceipt>

const REACTOR_OPENAGENTS_DOGFOOD_QWEN_PROFILE = nodeProfile({
  displayName: 'Refused OpenAgents dogfood Reactor Qwen refresh profile',
  exactLocalMeteringRequired: true,
  gateway: {
    endpointRef: 'endpoint.reactor.openagents.dogfood.openai_compatible.local',
    phoneHomeAllowedInServingPath: false,
    protocol: 'openai.chat_completions.v1',
    servingPathNetwork: 'offline_once_provisioned',
  },
  modelRef: 'model.alibaba.qwen.open_family',
  nodeProfileRef: 'reactor.node_profile.openagents.dogfood.hydralisk.qwen_refused.v1',
  placement: 'dogfood',
  policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
  policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
  servingLane: 'hydralisk',
  servingStack: {
    engineRef: 'engine.hydralisk.vllm.openagents.dogfood.qwen_refused',
    imageDigestRef: 'image.hydralisk.vllm.openagents.dogfood.qwen_refused.sha256',
    kind: 'vllm',
    modelArtifactRefs: ['artifact.openagents.dogfood.qwen.open_family.weights'],
    versionRef: 'vllm.openagents.dogfood.qwen_refused.20260704',
  },
  sourceRefs: ['github:OpenAgentsInc/openagents#8276'],
})

export const REACTOR_OPENAGENTS_DOGFOOD_QWEN_REFUSED_BUNDLE =
  buildReactorAirgapUpdateBundleManifest({
    artifactSha256:
      'b0a6f9e2a4fcd1b91f4a2a6726d0c36922cfb6e6b6d312d7bfb28f0d1a0f8276',
    bundleRef: 'reactor.airgap_bundle.openagents.dogfood.qwen.refused.20260704',
    bundleVersion: '2026-07-04.rx6.qwen-refused',
    createdAt: '2026-07-04T14:16:00.000Z',
    modelRef: REACTOR_OPENAGENTS_DOGFOOD_QWEN_PROFILE.modelRef,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_QWEN_PROFILE,
    policyRef: REACTOR_EXAMPLE_POLICIES.usOnly.policyRef,
    policyVersion: REACTOR_EXAMPLE_POLICIES.usOnly.version,
    signatureKid: 'openagents-release-ed25519-20260704',
    signatureRef: 'reactor.airgap_bundle.openagents.dogfood.qwen.refused.20260704.sig',
    sourceRefs: ['github:OpenAgentsInc/openagents#8276'],
  })

export const REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT =
  buildReactorInstallOpsReceipt({
    action: 'upgrade',
    bundle: REACTOR_OPENAGENTS_DOGFOOD_QWEN_REFUSED_BUNDLE,
    catalog: REACTOR_MODEL_CATALOG_SEED,
    decidedAt: '2026-07-04T14:16:30.000Z',
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_QWEN_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    receiptRef: 'reactor.install_ops.openagents.dogfood.qwen.refused.20260704',
    sourceRefs: ['github:OpenAgentsInc/openagents#8276'],
  })

export const REACTOR_OPENAGENTS_DOGFOOD_RUN_RECEIPT =
  buildReactorDogfoodRunReceipt({
    caseStudyWriteupRef:
      'docs/fable/2026-07-04-rx-6-reactor-dogfood-run.md',
    generatedAt: '2026-07-04T14:20:00.000Z',
    installOpsReceipt: REACTOR_OPENAGENTS_DOGFOOD_INSTALL_OPS_RECEIPT,
    measuredWindowEndedAt: '2026-07-04T14:15:00.000Z',
    measuredWindowStartedAt: '2026-07-04T14:10:00.000Z',
    meteringReceipts: REACTOR_OPENAGENTS_DOGFOOD_LOCAL_METERING_RECEIPT_SEED,
    nodeProfile: REACTOR_OPENAGENTS_DOGFOOD_HYDRALISK_PROFILE,
    policy: REACTOR_EXAMPLE_POLICIES.usOnly,
    receiptRef: 'reactor.dogfood_run.openagents.lead_gen_case_study_seed.20260704',
    refusedNonconformingInstallOpsReceipt:
      REACTOR_OPENAGENTS_DOGFOOD_REFUSED_INSTALL_OPS_RECEIPT,
    routeReceipts: REACTOR_OPENAGENTS_DOGFOOD_ROUTE_DECISION_SEED,
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8276',
      'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#9-workstream-map-rx--filed-2026-07-04-under-epic-8261',
    ],
    workloadRef: 'workload.openagents.lead_gen_reactor.case_study_seed.20260704',
  })

const corpusDocument = (
  record: Omit<ReactorCorpusDocumentRecord, 'schemaVersion'>,
): ReactorCorpusDocumentRecord =>
  S.decodeUnknownSync(ReactorCorpusDocumentRecord)({
    schemaVersion: 'openagents.reactor.corpus_document.v1',
    ...record,
  })

const corpusSubject = (
  record: ReactorCorpusAccessSubject,
): ReactorCorpusAccessSubject =>
  S.decodeUnknownSync(ReactorCorpusAccessSubject)(record)

const corpusRequest = (
  record: ReactorCorpusRetrievalRequest,
): ReactorCorpusRetrievalRequest =>
  S.decodeUnknownSync(ReactorCorpusRetrievalRequest)(record)

const oracleVerdict = (
  record: ReactorNeedToKnowOracleVerdict,
): ReactorNeedToKnowOracleVerdict =>
  S.decodeUnknownSync(ReactorNeedToKnowOracleVerdict)(record)

export const REACTOR_NEED_TO_KNOW_RULESET_V1 =
  S.decodeUnknownSync(ReactorNeedToKnowRuleSet)({
    schemaVersion: 'openagents.reactor.need_to_know_ruleset.v1',
    defaultAction: 'deny',
    hardRuleRefs: [
      'hard_rule.reactor.need_to_know.workspace_scope.v1',
      'hard_rule.reactor.need_to_know.matter_scope.v1',
      'hard_rule.reactor.need_to_know.role_or_user_scope.v1',
    ],
    oraclePolicyRef: 'soft_oracle.reactor.need_to_know.plausibility.v1',
    ruleSetRef: 'reactor.need_to_know.ruleset.v1',
    sourceRefs: [
      'github:OpenAgentsInc/openagents#8277',
      'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md#9-workstream-map-rx--filed-2026-07-04-under-epic-8261',
    ],
    version: '2026-07-04.rx9',
  })

export const REACTOR_NEED_TO_KNOW_BROKEN_ALLOW_ALL_RULESET_FIXTURE = {
  schemaVersion: 'openagents.reactor.need_to_know_ruleset.v1',
  defaultAction: 'allow',
  hardRuleRefs: [],
  oraclePolicyRef: 'soft_oracle.reactor.need_to_know.disabled',
  ruleSetRef: 'reactor.need_to_know.ruleset.broken_allow_all',
  sourceRefs: ['github:OpenAgentsInc/openagents#8277'],
  version: '2026-07-04.broken',
} as const

export const REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS = {
  aliceStrategyMemo: corpusDocument({
    allowedRoleRefs: ['matter_attorney'],
    allowedUserRefs: ['user.alice'],
    citationRefs: ['citation.reactor.fixture.alice.strategy_memo'],
    documentRef: 'document.reactor.fixture.alice.strategy_memo',
    matterRef: 'matter.reactor.fixture.alice',
    sourceRefs: ['source.reactor.fixture.alice.strategy_memo'],
    summaryRef: 'summary.reactor.fixture.alice.strategy_memo',
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
  bobIntakeNote: corpusDocument({
    allowedRoleRefs: ['matter_analyst'],
    allowedUserRefs: ['user.bob'],
    citationRefs: ['citation.reactor.fixture.bob.intake_note'],
    documentRef: 'document.reactor.fixture.bob.intake_note',
    matterRef: 'matter.reactor.fixture.bob',
    sourceRefs: ['source.reactor.fixture.bob.intake_note'],
    summaryRef: 'summary.reactor.fixture.bob.intake_note',
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
} as const

export const REACTOR_NEED_TO_KNOW_ADVERSARIAL_SUBJECTS = {
  alice: corpusSubject({
    matterRefs: ['matter.reactor.fixture.alice'],
    roleRefs: ['matter_attorney'],
    subjectUserRef: 'user.alice',
    workspaceRefs: ['workspace.reactor.fixture.customer_one'],
  }),
  bob: corpusSubject({
    matterRefs: ['matter.reactor.fixture.bob'],
    roleRefs: ['matter_analyst'],
    subjectUserRef: 'user.bob',
    workspaceRefs: ['workspace.reactor.fixture.customer_one'],
  }),
} as const

export const REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS = {
  aliceDirect: corpusRequest({
    matterRef: 'matter.reactor.fixture.alice',
    outputMode: 'source',
    queryIntentRef: 'query_intent.reactor.fixture.alice.direct',
    requestRef: 'request.reactor.fixture.alice.direct',
    requestedDocumentRefs: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ],
    subject: REACTOR_NEED_TO_KNOW_ADVERSARIAL_SUBJECTS.alice,
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
  aliceSoftDenied: corpusRequest({
    matterRef: 'matter.reactor.fixture.alice',
    outputMode: 'summary',
    queryIntentRef: 'query_intent.reactor.fixture.alice.soft_denied',
    requestRef: 'request.reactor.fixture.alice.soft_denied',
    requestedDocumentRefs: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ],
    subject: REACTOR_NEED_TO_KNOW_ADVERSARIAL_SUBJECTS.alice,
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
  bobAliceCitation: corpusRequest({
    matterRef: 'matter.reactor.fixture.bob',
    outputMode: 'citation',
    queryIntentRef: 'query_intent.reactor.fixture.bob.alice_citation',
    requestRef: 'request.reactor.fixture.bob.alice_citation',
    requestedDocumentRefs: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ],
    subject: REACTOR_NEED_TO_KNOW_ADVERSARIAL_SUBJECTS.bob,
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
  bobAliceSummary: corpusRequest({
    matterRef: 'matter.reactor.fixture.bob',
    outputMode: 'summary',
    queryIntentRef: 'query_intent.reactor.fixture.bob.alice_summary',
    requestRef: 'request.reactor.fixture.bob.alice_summary',
    requestedDocumentRefs: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo.documentRef,
    ],
    subject: REACTOR_NEED_TO_KNOW_ADVERSARIAL_SUBJECTS.bob,
    workspaceRef: 'workspace.reactor.fixture.customer_one',
  }),
} as const

export const REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES = {
  alicePlausible: oracleVerdict({
    documentRef: REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo
      .documentRef,
    loggedAt: '2026-07-04T15:05:00.000Z',
    modelRef: 'model.openagents.need_to_know.oracle.fixture',
    reasonRefs: ['reason.reactor.need_to_know.alice_direct_plausible'],
    requestRef: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceDirect.requestRef,
    verdict: 'need_to_know_plausible',
    verdictRef: 'oracle.reactor.fixture.alice.direct.plausible',
  }),
  aliceNotNeeded: oracleVerdict({
    documentRef: REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo
      .documentRef,
    loggedAt: '2026-07-04T15:06:00.000Z',
    modelRef: 'model.openagents.need_to_know.oracle.fixture',
    reasonRefs: ['reason.reactor.need_to_know.alice_summary_not_needed'],
    requestRef:
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceSoftDenied.requestRef,
    verdict: 'not_need_to_know',
    verdictRef: 'oracle.reactor.fixture.alice.soft_denied.not_needed',
  }),
  bobAlicePlausibleButHardDenied: oracleVerdict({
    documentRef: REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo
      .documentRef,
    loggedAt: '2026-07-04T15:07:00.000Z',
    modelRef: 'model.openagents.need_to_know.oracle.fixture',
    reasonRefs: ['reason.reactor.need_to_know.bob_prompt_claimed_relevance'],
    requestRef:
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.bobAliceSummary.requestRef,
    verdict: 'need_to_know_plausible',
    verdictRef: 'oracle.reactor.fixture.bob.alice.plausible_but_hard_denied',
  }),
} as const

export const REACTOR_NEED_TO_KNOW_ADVERSARIAL_RECEIPTS = {
  aliceAllowed: evaluateReactorNeedToKnowAccess({
    decidedAt: '2026-07-04T15:05:30.000Z',
    documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
    oracleVerdicts: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.alicePlausible,
    ],
    receiptRef: 'reactor.corpus_access.alice.direct.allowed.20260704',
    request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceDirect,
    ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
    sourceRefs: ['github:OpenAgentsInc/openagents#8277'],
  }),
  aliceSoftDenied: evaluateReactorNeedToKnowAccess({
    decidedAt: '2026-07-04T15:06:30.000Z',
    documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
    oracleVerdicts: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES.aliceNotNeeded,
    ],
    receiptRef: 'reactor.corpus_access.alice.summary.soft_denied.20260704',
    request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.aliceSoftDenied,
    ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
    sourceRefs: ['github:OpenAgentsInc/openagents#8277'],
  }),
  bobAliceCitationDenied: evaluateReactorNeedToKnowAccess({
    decidedAt: '2026-07-04T15:08:30.000Z',
    documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
    receiptRef: 'reactor.corpus_access.bob.alice_citation.denied.20260704',
    request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.bobAliceCitation,
    ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
    sourceRefs: ['github:OpenAgentsInc/openagents#8277'],
  }),
  bobAliceSummaryDenied: evaluateReactorNeedToKnowAccess({
    decidedAt: '2026-07-04T15:07:30.000Z',
    documents: [REACTOR_NEED_TO_KNOW_ADVERSARIAL_DOCUMENTS.aliceStrategyMemo],
    oracleVerdicts: [
      REACTOR_NEED_TO_KNOW_ADVERSARIAL_ORACLES
        .bobAlicePlausibleButHardDenied,
    ],
    receiptRef: 'reactor.corpus_access.bob.alice_summary.denied.20260704',
    request: REACTOR_NEED_TO_KNOW_ADVERSARIAL_REQUESTS.bobAliceSummary,
    ruleSet: REACTOR_NEED_TO_KNOW_RULESET_V1,
    sourceRefs: ['github:OpenAgentsInc/openagents#8277'],
  }),
} as const
