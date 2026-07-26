import { Effect, Redacted, Schema } from 'effect'

import {
  ForgeOwnedCanonicalMirrorError,
  type ForgeOwnedCanonicalMirrorDescriptor,
  type ForgeOwnedCanonicalMirrorService,
} from './forge-github-mirror-worker'
import type { ForgeGitHubMirrorIntent, ForgeGitHubMirrorObservedState } from '@openagentsinc/forge-protocol'
import { decodeForgeGitHubMirrorObservedState } from '@openagentsinc/forge-protocol'

const RegistryEntry = Schema.Struct({
  authorityGeneration: Schema.Number,
  authorityMode: Schema.Literals([
    'github_authoritative',
    'openagents_git_authoritative',
  ]),
  destinationGithubRef: Schema.String,
  destinationGithubRepository: Schema.String,
  repositoryRef: Schema.String,
  sourceRefs: Schema.Array(Schema.String),
  tenantRef: Schema.String,
})
type RegistryEntry = Schema.Schema.Type<typeof RegistryEntry>

const Registry = Schema.Array(RegistryEntry)

type SourceResponse = Readonly<{
  objectId: string
  repositoryRef: string
  sourceRef: string
  tenantRef: string
}>

const sourceResponse = Schema.Struct({
  objectId: Schema.String,
  repositoryRef: Schema.String,
  sourceRef: Schema.String,
  tenantRef: Schema.String,
})

const error = (operation: string, reason: string, retryable = false) =>
  new ForgeOwnedCanonicalMirrorError({ operation, reason, retryable })

const decodeRegistry = (raw: string): ReadonlyArray<RegistryEntry> =>
  Schema.decodeUnknownSync(Registry)(JSON.parse(raw))

/**
 * The Worker supplies destination policy only from its static admitted registry.
 * The Forge Git service resolves the canonical ref and executes the GitHub
 * operation. Neither side accepts destination URLs or GitHub headers from a
 * caller.
 */
export const makeForgeOwnedCanonicalMirrorHttpService = (input: Readonly<{
  baseUrl: string
  registryJson: string
  serviceAuthToken: string
  fetch?: typeof fetch
}>): ForgeOwnedCanonicalMirrorService => {
  let registry: ReadonlyArray<RegistryEntry>
  try {
    registry = decodeRegistry(input.registryJson)
  } catch {
    return {
      describe: () => Effect.fail(error('ForgeOwnedCanonicalMirror.describe', 'forge_github_mirror_registry_invalid')),
      observe: () => Effect.fail(error('ForgeOwnedCanonicalMirror.observe', 'forge_github_mirror_registry_invalid')),
      project: () => Effect.fail(error('ForgeOwnedCanonicalMirror.project', 'forge_github_mirror_registry_invalid')),
    }
  }
  const base = input.baseUrl.replace(/\/$/u, '')
  const request = input.fetch ?? fetch
  const entryFor = (tenantRef: string, repositoryRef: string) =>
    registry.find(entry => entry.tenantRef === tenantRef && entry.repositoryRef === repositoryRef)

  const source = (entry: RegistryEntry, sourceRef: string) =>
    Effect.tryPromise({
      try: async (): Promise<SourceResponse> => {
        const url = new URL(`${base}/internal/v1/github-mirror/source`)
        url.searchParams.set('tenantRef', entry.tenantRef)
        url.searchParams.set('repositoryRef', entry.repositoryRef)
        url.searchParams.set('sourceRef', sourceRef)
        const response = await request(url, {
          headers: { authorization: `Bearer ${input.serviceAuthToken}` },
          method: 'GET',
        })
        if (!response.ok) throw new Error(`http_${response.status}`)
        return Schema.decodeUnknownSync(sourceResponse)(await response.json())
      },
      catch: cause =>
        error(
          'ForgeOwnedCanonicalMirror.describe',
          cause instanceof Error && cause.message.startsWith('http_')
            ? `forge_github_mirror_source_${cause.message}`
            : 'forge_github_mirror_source_unavailable',
          true,
        ),
    })

  const describe = (requestInput: Readonly<{ tenantRef: string; repositoryRef: string; sourceRef: string }>) =>
    Effect.gen(function* () {
      const entry = entryFor(requestInput.tenantRef, requestInput.repositoryRef)
      if (entry === undefined) {
        return yield* error('ForgeOwnedCanonicalMirror.describe', 'forge_github_mirror_repository_not_admitted')
      }
      const resolved = yield* source(entry, requestInput.sourceRef)
      if (
        resolved.tenantRef !== entry.tenantRef ||
        resolved.repositoryRef !== entry.repositoryRef ||
        resolved.sourceRef !== requestInput.sourceRef
      ) {
        return yield* error('ForgeOwnedCanonicalMirror.describe', 'forge_github_mirror_source_response_mismatch')
      }
      return {
        authorityGeneration: entry.authorityGeneration,
        authorityMode: entry.authorityMode,
        destinationGithubRef: entry.destinationGithubRef,
        destinationGithubRepository: entry.destinationGithubRepository,
        repositoryRef: entry.repositoryRef,
        sourceObjectId: resolved.objectId.toLowerCase(),
        sourceRef: requestInput.sourceRef,
        sourceRefs: entry.sourceRefs,
        tenantRef: entry.tenantRef,
      } satisfies ForgeOwnedCanonicalMirrorDescriptor
    })

  const invoke = (operation: 'observe' | 'project', intent: ForgeGitHubMirrorIntent) =>
    Effect.tryPromise({
      try: async (): Promise<ForgeGitHubMirrorObservedState> => {
        const response = await request(`${base}/internal/v1/github-mirror/${operation}`, {
          body: JSON.stringify(intent),
          headers: {
            authorization: `Bearer ${input.serviceAuthToken}`,
            'content-type': 'application/json',
          },
          method: 'POST',
        })
        if (!response.ok) throw new Error(`http_${response.status}`)
        return decodeForgeGitHubMirrorObservedState(await response.json())
      },
      catch: cause =>
        error(
          `ForgeOwnedCanonicalMirror.${operation}`,
          cause instanceof Error && cause.message.startsWith('http_')
            ? `forge_github_mirror_${operation}_${cause.message}`
            : `forge_github_mirror_${operation}_unavailable`,
          true,
        ),
    })

  return { describe, observe: intent => invoke('observe', intent), project: intent => invoke('project', intent) }
}
