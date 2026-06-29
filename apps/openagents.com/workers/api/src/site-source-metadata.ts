import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'
import type {
  AutopilotSiteAccessMode,
  AutopilotSiteDeployment,
  AutopilotSiteProject,
  AutopilotSiteRuntimeKind,
  AutopilotSiteVersion,
  AutopilotSiteVisibility,
} from './sites'
import {
  AutopilotSiteAccessMode as AutopilotSiteAccessModeSchema,
  AutopilotSiteRuntimeKind as AutopilotSiteRuntimeKindSchema,
  AutopilotSiteVisibility as AutopilotSiteVisibilitySchema,
} from './sites'

export const OPENAGENTS_SITE_METADATA_PATH = '.openagents/site.json'
export const OPENAGENTS_SITE_METADATA_SCHEMA_VERSION = 'openagents.site.v1'

export const OpenAgentsSiteMetadataSource = S.Struct({
  provider: S.Literal('github'),
  owner: S.String,
  name: S.String,
  ref: S.String,
  path: S.optionalKey(S.NullOr(S.String)),
})
export type OpenAgentsSiteMetadataSource =
  typeof OpenAgentsSiteMetadataSource.Type

export const OpenAgentsSiteMetadataBindings = S.Struct({
  d1: S.NullOr(S.String),
  r2: S.NullOr(S.String),
})
export type OpenAgentsSiteMetadataBindings =
  typeof OpenAgentsSiteMetadataBindings.Type

export const OpenAgentsSiteMetadataTarget = S.Struct({
  runtimeKind: AutopilotSiteRuntimeKindSchema,
  slug: S.String,
  url: S.optionalKey(S.NullOr(S.String)),
})
export type OpenAgentsSiteMetadataTarget =
  typeof OpenAgentsSiteMetadataTarget.Type

export const OpenAgentsSiteAgentSurfacePreset = S.Literals([
  'none',
  'basic',
  'public_proof',
  'public_challenge',
  'proof_and_challenge',
  'customer_site_safe',
  'openagents_network',
])
export type OpenAgentsSiteAgentSurfacePreset =
  typeof OpenAgentsSiteAgentSurfacePreset.Type

export const OpenAgentsSiteMetadataAgentSurface = S.Struct({
  preset: OpenAgentsSiteAgentSurfacePreset,
  publicSourceRef: S.optionalKey(S.NullOr(S.String)),
  capabilityManifestUrl: S.optionalKey(S.NullOr(S.String)),
  proofUrl: S.optionalKey(S.NullOr(S.String)),
  challengeUrl: S.optionalKey(S.NullOr(S.String)),
  openAgentsJoinUrl: S.optionalKey(S.NullOr(S.String)),
  referralJoinUrl: S.optionalKey(S.NullOr(S.String)),
  agentReferralJoinUrl: S.optionalKey(S.NullOr(S.String)),
})
export type OpenAgentsSiteMetadataAgentSurface =
  typeof OpenAgentsSiteMetadataAgentSurface.Type

export const OpenAgentsSiteMetadata = S.Struct({
  schemaVersion: S.Literal(OPENAGENTS_SITE_METADATA_SCHEMA_VERSION),
  siteId: S.NullOr(S.String),
  hostedProjectId: S.optionalKey(S.NullOr(S.String)),
  softwareOrderId: S.optionalKey(S.NullOr(S.String)),
  accessMode: AutopilotSiteAccessModeSchema,
  visibility: AutopilotSiteVisibilitySchema,
  source: S.optionalKey(S.NullOr(OpenAgentsSiteMetadataSource)),
  target: OpenAgentsSiteMetadataTarget,
  bindings: OpenAgentsSiteMetadataBindings,
  lastSavedVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
  agentSurface: S.optionalKey(OpenAgentsSiteMetadataAgentSurface),
  updatedAt: S.optionalKey(S.String),
})
export type OpenAgentsSiteMetadata = typeof OpenAgentsSiteMetadata.Type

export class OpenAgentsSiteMetadataUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMetadataUnsafe>()(
  'OpenAgentsSiteMetadataUnsafe',
  {
    reason: S.String,
  },
) {}

export class OpenAgentsSiteMetadataParseError extends S.TaggedErrorClass<OpenAgentsSiteMetadataParseError>()(
  'OpenAgentsSiteMetadataParseError',
  {
    reason: S.String,
  },
) {}

export class OpenAgentsSiteMetadataValidationError extends S.TaggedErrorClass<OpenAgentsSiteMetadataValidationError>()(
  'OpenAgentsSiteMetadataValidationError',
  {
    reason: S.String,
  },
) {}

const secretKeyPattern =
  /(^|_)(api_?key|secret|token|password|cookie|bearer|oauth|auth_?grant)(_|$)/i

const metadataContainsForbiddenKey = (value: unknown): boolean => {
  if (Array.isArray(value)) {
    return value.some(metadataContainsForbiddenKey)
  }

  if (value === null || typeof value !== 'object') {
    return false
  }

  return Object.entries(value).some(
    ([key, nested]) =>
      secretKeyPattern.test(key) || metadataContainsForbiddenKey(nested),
  )
}

export const assertOpenAgentsSiteMetadataSafe = (
  metadata: OpenAgentsSiteMetadata,
): Effect.Effect<void, OpenAgentsSiteMetadataUnsafe> =>
  Effect.sync(() => JSON.stringify(metadata)).pipe(
    Effect.flatMap(json =>
      containsProviderSecretMaterial(json) ||
      metadataContainsForbiddenKey(metadata)
        ? Effect.fail(
            new OpenAgentsSiteMetadataUnsafe({
              reason:
                '.openagents/site.json metadata contains secret-shaped material.',
            }),
          )
        : Effect.void,
    ),
  )

export const parseOpenAgentsSiteMetadata = (
  text: string,
): Effect.Effect<
  OpenAgentsSiteMetadata,
  | OpenAgentsSiteMetadataParseError
  | OpenAgentsSiteMetadataUnsafe
  | OpenAgentsSiteMetadataValidationError
> =>
  Effect.gen(function* () {
    const parsed = yield* Effect.try({
      try: () => parseJsonUnknown(text),
      catch: error =>
        new OpenAgentsSiteMetadataParseError({
          reason:
            error instanceof Error
              ? error.message
              : 'Unable to parse .openagents/site.json.',
        }),
    })
    const parsedJson = yield* Effect.sync(() => JSON.stringify(parsed))

    if (
      containsProviderSecretMaterial(parsedJson) ||
      metadataContainsForbiddenKey(parsed)
    ) {
      return yield* new OpenAgentsSiteMetadataUnsafe({
        reason:
          '.openagents/site.json metadata contains secret-shaped material.',
      })
    }

    const metadata = yield* S.decodeUnknownEffect(OpenAgentsSiteMetadata)(
      parsed,
    ).pipe(
      Effect.mapError(
        error =>
          new OpenAgentsSiteMetadataValidationError({
            reason: String(error),
          }),
      ),
    )

    yield* assertOpenAgentsSiteMetadataSafe(metadata)

    return metadata
  })

export const serializeOpenAgentsSiteMetadata = (
  metadata: OpenAgentsSiteMetadata,
): Effect.Effect<
  string,
  OpenAgentsSiteMetadataUnsafe | OpenAgentsSiteMetadataValidationError
> =>
  Effect.gen(function* () {
    const validated = yield* S.decodeUnknownEffect(OpenAgentsSiteMetadata)(
      metadata,
    ).pipe(
      Effect.mapError(
        error =>
          new OpenAgentsSiteMetadataValidationError({
            reason: String(error),
          }),
      ),
    )

    yield* assertOpenAgentsSiteMetadataSafe(validated)

    return `${JSON.stringify(validated, null, 2)}\n`
  })

const defaultRuntimeKind = (
  deployment: AutopilotSiteDeployment | null,
): AutopilotSiteRuntimeKind =>
  deployment?.runtimeKind ?? 'omega_static_r2'

const savedVersionId = (
  version: AutopilotSiteVersion | null,
): string | null =>
  version?.buildStatus === 'saved' ? version.id : null

const bindingsFromVersion = (
  version: AutopilotSiteVersion | null,
): OpenAgentsSiteMetadataBindings => ({
  d1: version?.d1BindingName ?? null,
  r2: version?.r2BindingName ?? null,
})

const sourceFromProject = (
  project: AutopilotSiteProject,
): OpenAgentsSiteMetadataSource | null =>
  project.sourceRepository === null
    ? null
    : {
        provider: 'github',
        owner: project.sourceRepository.owner,
        name: project.sourceRepository.name,
        ref: project.sourceRepository.ref,
      }

export const openAgentsSiteMetadataFromProject = (input: {
  activeDeployment?: AutopilotSiteDeployment | null | undefined
  agentSurface?: OpenAgentsSiteMetadataAgentSurface | undefined
  hostedProjectId?: string | null | undefined
  project: AutopilotSiteProject
  updatedAt?: string | undefined
  version?: AutopilotSiteVersion | null | undefined
}): OpenAgentsSiteMetadata => ({
  schemaVersion: OPENAGENTS_SITE_METADATA_SCHEMA_VERSION,
  siteId: input.project.id,
  ...(input.hostedProjectId === undefined
    ? {}
    : { hostedProjectId: input.hostedProjectId }),
  ...(input.project.softwareOrderId === null
    ? {}
    : { softwareOrderId: input.project.softwareOrderId }),
  accessMode: input.project.accessMode as AutopilotSiteAccessMode,
  visibility: input.project.visibility as AutopilotSiteVisibility,
  source: sourceFromProject(input.project),
  target: {
    runtimeKind: defaultRuntimeKind(input.activeDeployment ?? null),
    slug: input.project.slug,
    url: input.activeDeployment?.url ?? null,
  },
  bindings: bindingsFromVersion(input.version ?? null),
  lastSavedVersionId: savedVersionId(input.version ?? null),
  activeDeploymentId: input.activeDeployment?.id ?? input.project.activeDeploymentId,
  ...(input.agentSurface === undefined
    ? {}
    : { agentSurface: input.agentSurface }),
  ...(input.updatedAt === undefined ? {} : { updatedAt: input.updatedAt }),
})
