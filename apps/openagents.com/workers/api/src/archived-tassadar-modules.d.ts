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
