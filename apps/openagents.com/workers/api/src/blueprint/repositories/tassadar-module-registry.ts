import { Effect, Schema as S } from 'effect'

export const TASSADAR_MODULE_REGISTRY_VERSION_REF =
  'registry.blueprint.tassadar_modules.archived_to_backroom'
export const BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF =
  TASSADAR_MODULE_REGISTRY_VERSION_REF
export const BLUEPRINT_TASSADAR_MODULE_REGISTRY_SCHEMA_VERSION =
  'blueprint.tassadar_module_registry.archived.v1'
export const BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE =
  'archived_to_backroom'
export const BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS =
  'claim.blueprint.tassadar_dense_module.archived'

export type BlueprintTassadarModuleRegistryModuleKind =
  | 'dense_weight_module'
  | 'linked_dense_module'
export type BlueprintTassadarModuleRegistryEntry = Readonly<{
  claimClass: string
  moduleDigest: string
  moduleKind: BlueprintTassadarModuleRegistryModuleKind
  moduleRef: string
  publicSafe: boolean
  registryVersionRef: string
  sourceRefs: ReadonlyArray<string>
  traceDigest: string
  trustPosture: string
}>
export type BlueprintTassadarModuleRegistryResolveInput = Readonly<{
  moduleRef: string
  requiredClaimClass?: string | undefined
  requiredModuleKind?: BlueprintTassadarModuleRegistryModuleKind | undefined
  requiredTrustPosture?: string | undefined
}>
export type BlueprintTassadarModuleRegistryResolveError = Readonly<{
  kind:
    | 'claim_class_refused'
    | 'module_kind_refused'
    | 'module_not_found'
    | 'trust_posture_refused'
    | 'unsafe_registry'
  moduleRef: string
  reason: string
}>
export type BlueprintTassadarModuleRegistryResolver = (
  input: BlueprintTassadarModuleRegistryResolveInput,
) => Effect.Effect<
  BlueprintTassadarModuleRegistryEntry,
  BlueprintTassadarModuleRegistryResolveError
>

export const seedBlueprintTassadarModuleRegistryEntries =
  [] as ReadonlyArray<BlueprintTassadarModuleRegistryEntry>

export const BlueprintTassadarModuleRegistryEntry = S.Struct({
  claimClass: S.String,
  moduleDigest: S.String,
  moduleKind: S.Literals(['dense_weight_module', 'linked_dense_module']),
  moduleRef: S.String,
  publicSafe: S.Boolean,
  registryVersionRef: S.String,
  sourceRefs: S.Array(S.String),
  traceDigest: S.String,
  trustPosture: S.String,
})
export const BlueprintTassadarModuleRegistryResolveError = S.Struct({
  kind: S.Literals([
    'claim_class_refused',
    'module_kind_refused',
    'module_not_found',
    'trust_posture_refused',
    'unsafe_registry',
  ]),
  moduleRef: S.String,
  reason: S.String,
})
export const BlueprintTassadarModuleRegistryResolveInput = S.Struct({
  moduleRef: S.String,
  requiredClaimClass: S.optional(S.String),
  requiredModuleKind: S.optional(S.Literals(['dense_weight_module', 'linked_dense_module'])),
  requiredTrustPosture: S.optional(S.String),
})
export const BlueprintTassadarModuleRegistryProjection = S.Struct({
  modules: S.Array(BlueprintTassadarModuleRegistryEntry),
  registryVersionRef: S.String,
  safeProjection: S.Boolean,
  schemaVersion: S.String,
})

export const blueprintTassadarModuleRegistryProjectionIsSafe = (
  projection: unknown,
): boolean =>
  typeof projection === 'object' &&
  projection !== null &&
  !/access_token|callback_token|private_key|provider_payload|raw_prompt|sk-[a-z0-9]/i.test(
    JSON.stringify(projection),
  )

const archivedProjection = {
  modules: seedBlueprintTassadarModuleRegistryEntries,
  registryVersionRef: BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  safeProjection: true,
  schemaVersion: BLUEPRINT_TASSADAR_MODULE_REGISTRY_SCHEMA_VERSION,
} as const

export const listBlueprintTassadarModuleRegistry = (): Effect.Effect<
  typeof archivedProjection,
  never
> => Effect.succeed(archivedProjection)

export const resolveBlueprintTassadarModuleRegistryEntry = (
  input: BlueprintTassadarModuleRegistryResolveInput,
) =>
  Effect.fail({
    kind: 'module_not_found' as const,
    moduleRef: input.moduleRef,
    reason: 'Tassadar module registry was retired and archived to backroom.',
  }) as Effect.Effect<
    BlueprintTassadarModuleRegistryEntry,
    BlueprintTassadarModuleRegistryResolveError
  >

export class TassadarModuleRegistryError extends Error {
  readonly reason: string

  constructor(reason = 'Tassadar module registry archived to backroom') {
    super(reason)
    this.reason = reason
  }
}

export const createTassadarModuleRegistry = () => ({
  list: async () => [],
  lookup: async () => {
    throw new TassadarModuleRegistryError()
  },
})

export const makeInMemoryTassadarModuleRegistry = createTassadarModuleRegistry
