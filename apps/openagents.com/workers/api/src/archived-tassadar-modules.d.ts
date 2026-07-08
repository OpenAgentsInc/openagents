declare module '@openagentsinc/proof-replay' {
  export const LAUNCH_RECOGNITION_REPLAY_SLUG: string
  export const FIRST_REAL_SETTLEMENT_REPLAY_SLUG: string
  export const TASSADAR_FIRST_REAL_SETTLEMENT_REPLAY_ENDPOINT: string
  export type ProofReplayBundle = any
  export type ProofReplayCatalogEntry = any
  export type ProofReplayCatalogSlug = string
  export const proofReplayCatalog: (...args: any[]) => any
  export const proofReplayCatalogEntryForSlug: (...args: any[]) => any
  export const proofReplayBundleEndpointForSlug: (...args: any[]) => any
  export const assertProofReplayBundleShipmentGate: (...args: any[]) => any
  export const buildReplayRenderPlan: (...args: any[]) => any
}

declare module '@openagentsinc/tassadar-executor' {
  export const TASSADAR_EXECUTOR_CAPABILITY_REF: string
  export const TASSADAR_EXECUTOR_LEG_REFS: readonly string[]
  export const TASSADAR_EXECUTOR_WINDOW_VERSION_REF: string
  export const TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND: string
  export const TASSADAR_EXECUTOR_TRACE_JOB_KIND: string
  export const KERNEL_OPTIMIZATION_PARITY_CLASS_ID: string
  export const TASSADAR_TS_REPLAY_CLASS_ID: string
  export type KernelOptimizationVerdict = any
  export type TassadarReplayVerdict = any
  export type TassadarAlmNumericModel = any
  export class TassadarNumericExecutionError extends Error {}
  export const collectInterpreterOutputs: (...args: any[]) => any
  export const executeTassadarNumericModel: (...args: any[]) => Promise<any>
  export const verifyKernelOptimizationParity: (...args: any[]) => any
  export const hasReceiptedTassadarExecutorCapability: (...args: any[]) => boolean
  export const isTassadarExecutorSelfTestReceiptRef: (...args: any[]) => boolean
}

declare module '@openagentsinc/tassadar-executor/dense-weight-module' {
  export const TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND: string
  export const tassadarDenseProgramFixture: any
  export const tassadarDenseWeightModuleDigest: string
  export const tassadarDenseWeightModuleTraceDigest: (...args: any[]) => any
  export const executeTassadarDenseWeightModule: (...args: any[]) => any
  export class TassadarDenseModuleError extends Error {}
  export type TassadarDenseWeightModule = any
}

declare module '@openagentsinc/tassadar-executor/linked-dense-module' {
  export const TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST: string
  export const TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS: string
  export const TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST: string
  export const TASSADAR_ALM_LINKED_DENSE_MODULE_KIND: string
  export const TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF: string
  export const tassadarLinkedDenseProgramFixture: any
  export const tassadarLinkedDenseModuleTraceDigest: (...args: any[]) => any
  export const executeTassadarLinkedDenseModule: (...args: any[]) => any
  export const projectTassadarCompiledWeightModuleListing: (...args: any[]) => Promise<any>
  export const verifyTassadarLinkedDenseComposition: (...args: any[]) => any
  export type TassadarCompiledWeightModuleListing = any
  export type TassadarLinkedDenseModule = any
  export type TassadarLinkedDenseProgramFixture = any
  export type TassadarLinkedDenseReplayVerification = any
}

declare module '@openagentsinc/tassadar-executor/compiled-program-corpus' {
  export const loadTassadarCompiledProgramCorpusFixture: (...args: any[]) => any
  export const selectTassadarCompiledProgramFixture: (...args: any[]) => any
  export const tassadarCompiledProgramCorpus: any
  export const tassadarCompiledProgramCorpusSize: number
}

declare module './public-proof-replay-routes' {
  export const handlePublicProofReplayBundleRequest: (...args: any[]) => any
}

declare module './blueprint/repositories/tassadar-module-registry' {
  export const createTassadarModuleRegistry: (...args: any[]) => any
  export const makeInMemoryTassadarModuleRegistry: (...args: any[]) => any
  export const TASSADAR_MODULE_REGISTRY_VERSION_REF: string
}

declare module './blueprint/services/tassadar-module-step' {
  export const makeTassadarModuleStepRuntime: (...args: any[]) => any
}

declare module './blueprint/services/replay-module' {
  export const makePublicProofReplayModuleRuntime: (...args: any[]) => any
}

declare module './blueprint/fixtures/replay-signatures' {
  export const blueprintReplaySignatureFixtures: any
}

declare module './tassadar-module-library' {
  export const listTassadarCompiledModules: (...args: any[]) => any
}
