import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
} from '@openagentsinc/tassadar-executor'
import {
  tassadarDenseProgramFixture,
  tassadarDenseWeightModuleDigest,
} from '@openagentsinc/tassadar-executor/dense-weight-module'
import {
  TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
  TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
  tassadarLinkedDenseProgramFixture,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import { Effect, Schema as S } from 'effect'

import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  BlueprintTassadarModuleStepKind,
  type BlueprintTassadarModuleStepKind as BlueprintTassadarModuleStepKindType,
} from '../schemas/program'

export const BLUEPRINT_TASSADAR_MODULE_REGISTRY_SCHEMA_VERSION =
  'blueprint_tassadar_module_registry.v1'
export const BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF =
  'registry.tassadar_modules.seed.2026-06-18'
export const BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS =
  'compiled dense ALM module / exact replay gate'
export const BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE =
  TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE

export const BlueprintTassadarModuleRegistryEntry = S.Struct({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  capabilityRef: S.String,
  caveatRefs: S.Array(S.String),
  claimBoundary: S.String,
  claimClass: S.String,
  compileReceiptRefs: S.Array(S.String),
  fixtureRef: S.String,
  moduleDigest: S.String,
  moduleId: S.String,
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  publicSafe: S.Literal(true),
  registryVersionRef: S.String,
  traceDigest: S.String,
  trustPosture: S.String,
})
export type BlueprintTassadarModuleRegistryEntry =
  typeof BlueprintTassadarModuleRegistryEntry.Type

export const BlueprintTassadarModuleRegistryProjection = S.Struct({
  caveatRefs: S.Array(S.String),
  generatedAt: S.String,
  modules: S.Array(BlueprintTassadarModuleRegistryEntry),
  registryVersionRef: S.String,
  safeProjection: S.Literal(true),
  schemaVersion: S.Literal(BLUEPRINT_TASSADAR_MODULE_REGISTRY_SCHEMA_VERSION),
})
export type BlueprintTassadarModuleRegistryProjection =
  typeof BlueprintTassadarModuleRegistryProjection.Type

export const BlueprintTassadarModuleRegistryResolveInput = S.Struct({
  moduleRef: S.String,
  requiredClaimClass: S.optional(S.String),
  requiredModuleKind: S.optional(BlueprintTassadarModuleStepKind),
  requiredTrustPosture: S.optional(S.String),
})
export type BlueprintTassadarModuleRegistryResolveInput =
  typeof BlueprintTassadarModuleRegistryResolveInput.Type

export class BlueprintTassadarModuleRegistryResolveError extends S.TaggedErrorClass<BlueprintTassadarModuleRegistryResolveError>()(
  'BlueprintTassadarModuleRegistryResolveError',
  {
    kind: S.Literals([
      'module_not_found',
      'module_kind_refused',
      'claim_class_refused',
      'trust_posture_refused',
      'unsafe_projection',
    ]),
    moduleRef: S.String,
    reason: S.String,
  },
) {}

export type BlueprintTassadarModuleRegistryResolver = (
  input: BlueprintTassadarModuleRegistryResolveInput,
) => Effect.Effect<
  BlueprintTassadarModuleRegistryEntry,
  BlueprintTassadarModuleRegistryResolveError
>

const privateFieldPattern =
  /(^|[._-])(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|password|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_trace|raw_webhook|refresh_token|runner_log|secret|source_archive|token|wallet|xprv)([._-]|$)/i
const privateCamelFieldPattern =
  /^(accessToken|authorization|bearer|callbackUrl|callbackToken|clientSecret|customerEmail|customerName|idToken|invoice|mnemonic|oauth|password|paymentHash|paymentId|paymentPreimage|payoutAddress|payoutDestination|payoutTarget|preimage|privateKey|privateRepo|providerGrant|providerPayload|providerToken|rawEmail|rawPayload|rawPrompt|rawRunLog|rawRunner|rawSourceArchive|rawTrace|rawWebhook|refreshToken|runnerLog|secret|sourceArchive|token|wallet|xprv)$/i
const privateValuePattern =
  /\b(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_trace|raw_webhook|refresh_token|runner_log|source_archive|wallet|xprv)\b/i

const uniqueSorted = (values: ReadonlyArray<string>): Array<string> =>
  [...new Set(values)].sort()

const denseEntry = (): BlueprintTassadarModuleRegistryEntry => ({
  artifactRefs: uniqueSorted(tassadarDenseProgramFixture.runArtifactRefs),
  blockerRefs: [],
  capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
  caveatRefs: [
    'caveat.blueprint.tassadar_module.fixture_seeded_until_live_registry_storage',
    'caveat.blueprint.tassadar_module.not_a_serving_or_inference_endpoint',
  ],
  claimBoundary: tassadarDenseProgramFixture.claimBoundary,
  claimClass: BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS,
  compileReceiptRefs: uniqueSorted(tassadarDenseProgramFixture.compileReceiptRefs),
  fixtureRef: tassadarDenseProgramFixture.fixtureId,
  moduleDigest: tassadarDenseWeightModuleDigest,
  moduleId: tassadarDenseProgramFixture.denseModule.moduleId,
  moduleKind: 'dense_weight_module',
  moduleRef: tassadarDenseProgramFixture.denseModule.moduleId,
  publicSafe: true,
  registryVersionRef: BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  traceDigest: tassadarDenseProgramFixture.expectedTraceDigest,
  trustPosture: BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE,
})

const linkedEntry = (): BlueprintTassadarModuleRegistryEntry => ({
  artifactRefs: uniqueSorted(
    tassadarLinkedDenseProgramFixture.marketplaceArtifactRefs,
  ),
  blockerRefs: [],
  capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
  caveatRefs: [
    'caveat.blueprint.tassadar_module.fixture_seeded_until_live_registry_storage',
    'caveat.blueprint.tassadar_module.not_a_serving_or_inference_endpoint',
    'caveat.blueprint.tassadar_module.linked_listing_is_not_settlement',
  ],
  claimBoundary: tassadarLinkedDenseProgramFixture.claimBoundary,
  claimClass: TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
  compileReceiptRefs: uniqueSorted(
    tassadarLinkedDenseProgramFixture.compileReceiptRefs,
  ),
  fixtureRef: tassadarLinkedDenseProgramFixture.fixtureId,
  moduleDigest: TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST,
  moduleId: tassadarLinkedDenseProgramFixture.linkedModule.moduleId,
  moduleKind: 'linked_dense_module',
  moduleRef: TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
  publicSafe: true,
  registryVersionRef: BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
  traceDigest: tassadarLinkedDenseProgramFixture.composedTraceDigest,
  trustPosture: TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE,
})

export const seedBlueprintTassadarModuleRegistryEntries =
  (): ReadonlyArray<BlueprintTassadarModuleRegistryEntry> => [
    denseEntry(),
    linkedEntry(),
  ]

export const listBlueprintTassadarModuleRegistry = (
  input?: Readonly<{
    entries?: ReadonlyArray<BlueprintTassadarModuleRegistryEntry>
    generatedAt?: string
  }>,
): Effect.Effect<
  BlueprintTassadarModuleRegistryProjection,
  BlueprintTassadarModuleRegistryResolveError
> =>
  Effect.gen(function* () {
    const projection = {
      caveatRefs: [
        'caveat.blueprint.tassadar_module.registry_seeded_from_verified_fixtures',
        'caveat.blueprint.tassadar_module.registry_does_not_grant_settlement_purchase_or_serving_authority',
      ],
      generatedAt: input?.generatedAt ?? currentIsoTimestamp(),
      modules: [...(input?.entries ?? seedBlueprintTassadarModuleRegistryEntries())],
      registryVersionRef: BLUEPRINT_TASSADAR_MODULE_REGISTRY_VERSION_REF,
      safeProjection: true,
      schemaVersion: BLUEPRINT_TASSADAR_MODULE_REGISTRY_SCHEMA_VERSION,
    } satisfies BlueprintTassadarModuleRegistryProjection

    if (!blueprintTassadarModuleRegistryProjectionIsSafe(projection)) {
      return yield* new BlueprintTassadarModuleRegistryResolveError({
        kind: 'unsafe_projection',
        moduleRef: 'registry',
        reason:
          'Tassadar module registry projections must contain public-safe refs, digests, trust posture, and claim envelopes only.',
      })
    }

    return projection
  })

export const resolveBlueprintTassadarModuleRegistryEntry =
  (
    input: BlueprintTassadarModuleRegistryResolveInput,
    options?: Readonly<{
      entries?: ReadonlyArray<BlueprintTassadarModuleRegistryEntry>
    }>,
  ): Effect.Effect<
    BlueprintTassadarModuleRegistryEntry,
    BlueprintTassadarModuleRegistryResolveError
  > =>
    Effect.gen(function* () {
      const projection = yield* listBlueprintTassadarModuleRegistry(
        options?.entries === undefined
          ? undefined
          : { entries: options.entries },
      )
      const entry = projection.modules.find(
        candidate => candidate.moduleRef === input.moduleRef,
      )

      if (entry === undefined) {
        return yield* new BlueprintTassadarModuleRegistryResolveError({
          kind: 'module_not_found',
          moduleRef: input.moduleRef,
          reason: 'Tassadar module ref is not resolvable in the registry.',
        })
      }

      if (
        input.requiredModuleKind !== undefined &&
        entry.moduleKind !== input.requiredModuleKind
      ) {
        return yield* registryRefusal(
          'module_kind_refused',
          input.moduleRef,
          `Tassadar module kind ${entry.moduleKind} does not match required kind ${input.requiredModuleKind}.`,
        )
      }

      if (
        input.requiredClaimClass !== undefined &&
        entry.claimClass !== input.requiredClaimClass
      ) {
        return yield* registryRefusal(
          'claim_class_refused',
          input.moduleRef,
          `Tassadar module claim class ${entry.claimClass} does not match required claim class ${input.requiredClaimClass}.`,
        )
      }

      if (
        input.requiredTrustPosture !== undefined &&
        entry.trustPosture !== input.requiredTrustPosture
      ) {
        return yield* registryRefusal(
          'trust_posture_refused',
          input.moduleRef,
          `Tassadar module trust posture ${entry.trustPosture} does not match required trust posture ${input.requiredTrustPosture}.`,
        )
      }

      return entry
    })

const registryRefusal = (
  kind:
    | 'module_kind_refused'
    | 'claim_class_refused'
    | 'trust_posture_refused',
  moduleRef: string,
  reason: string,
): Effect.Effect<never, BlueprintTassadarModuleRegistryResolveError> =>
  Effect.fail(
    new BlueprintTassadarModuleRegistryResolveError({
      kind,
      moduleRef,
      reason,
    }),
  )

export const blueprintTassadarModuleRegistryProjectionIsSafe = (
  value: unknown,
): boolean => {
  if (value === null || value === undefined) {
    return true
  }

  if (typeof value === 'string') {
    return !privateValuePattern.test(value)
  }

  if (Array.isArray(value)) {
    return value.every(blueprintTassadarModuleRegistryProjectionIsSafe)
  }

  if (typeof value !== 'object') {
    return true
  }

  for (const [key, child] of Object.entries(value)) {
    if (
      privateFieldKey(key) ||
      !blueprintTassadarModuleRegistryProjectionIsSafe(child)
    ) {
      return false
    }
  }

  return true
}

const privateFieldKey = (key: string): boolean =>
  privateFieldPattern.test(key) || privateCamelFieldPattern.test(key)

export type BlueprintTassadarModuleRegistryModuleKind =
  BlueprintTassadarModuleStepKindType
