import {
  buildReplayRenderPlan,
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  OPENAGENTS_PUBLIC_ORIGIN,
  proofReplayBundleEndpointForSlug,
  proofReplayCatalogEntryForSlug,
  type ProofReplayBundle as SharedProofReplayBundle,
} from '@openagentsinc/proof-replay'
import { Effect, Schema as S } from 'effect'

import {
  buildPublicProofReplayBundleForRequest,
  type ProofReplayBundle,
} from '../../public-proof-replay-routes'
import type { BlueprintReplayModuleBinding } from '../schemas/program'

export const BlueprintReplayModuleViewSpec = S.Struct({
  bundleEndpoint: S.String,
  bundleRef: S.String,
  replaySlug: S.String,
  socialPath: S.optional(S.String),
  websitePath: S.String,
})
export type BlueprintReplayModuleViewSpec =
  typeof BlueprintReplayModuleViewSpec.Type

export const BlueprintReplayModuleEvidence = S.Struct({
  authorityBoundary: S.Literal('evidence_only'),
  bundle: S.Record(S.String, S.Unknown),
  bundleEndpoint: S.String,
  bundleRef: S.String,
  contentRedacted: S.Literal(true),
  directMutationDisabled: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  intentRef: S.String,
  kind: S.Literal('blueprint_replay_module_evidence'),
  moduleRef: S.String,
  noDeploy: S.Literal(true),
  noEmail: S.Literal(true),
  noSourceMutation: S.Literal(true),
  noSpend: S.Literal(true),
  observedAt: S.String,
  receiptRefs: S.Array(S.String),
  renderPlan: S.Record(S.String, S.Unknown),
  replaySlug: S.String,
  replayViewSpec: BlueprintReplayModuleViewSpec,
  sourceAuthority: S.String,
  sourceRefs: S.Array(S.String),
  stepRef: S.String,
  summary: S.String,
  targetRef: S.String,
  title: S.String,
  toolRef: S.String,
})
export type BlueprintReplayModuleEvidence =
  typeof BlueprintReplayModuleEvidence.Type

export class BlueprintReplayModuleError extends S.TaggedErrorClass<BlueprintReplayModuleError>()(
  'BlueprintReplayModuleError',
  {
    moduleRef: S.String,
    reason: S.String,
    replaySlug: S.String,
  },
) {}

export type BlueprintReplayModuleRuntimeInput = Readonly<{
  observedAt: string
  origin: string
  replaySlug: string
}>

export type BlueprintReplayModuleRuntime = Readonly<{
  buildProofReplayBundle: (
    input: BlueprintReplayModuleRuntimeInput,
  ) => Effect.Effect<ProofReplayBundle, BlueprintReplayModuleError>
}>

export const makePublicProofReplayModuleRuntime = (
  env: Parameters<typeof buildPublicProofReplayBundleForRequest>[1],
): BlueprintReplayModuleRuntime => ({
  buildProofReplayBundle: input =>
    Effect.tryPromise({
      catch: error =>
        new BlueprintReplayModuleError({
          moduleRef: 'module.openagents.public_proof_replay_runtime',
          reason: errorText(error),
          replaySlug: input.replaySlug,
        }),
      try: () =>
        buildPublicProofReplayBundleForRequest(
          new Request(
            proofReplayBundleEndpointForSlug(input.replaySlug, input.origin),
          ),
          env,
          { now: () => input.observedAt },
        ),
    }),
})

export const executeBlueprintReplayModule = (
  input: Readonly<{
    binding: BlueprintReplayModuleBinding
    intentRef: string
    observedAt: string
    origin?: string | undefined
    replaySlug?: string | undefined
    runtime: BlueprintReplayModuleRuntime
    targetRef?: string | undefined
    toolRef: string
  }>,
): Effect.Effect<BlueprintReplayModuleEvidence, BlueprintReplayModuleError> =>
  Effect.gen(function* () {
    const replaySlug = input.replaySlug ?? input.binding.defaultReplaySlug
    const origin = input.origin ?? OPENAGENTS_PUBLIC_ORIGIN

    if (!input.binding.allowedReplaySlugs.includes(replaySlug)) {
      return yield* failReplayModule(
        input.binding.moduleRef,
        replaySlug,
        'Replay slug is outside this Blueprint replay module binding.',
      )
    }

    const catalogEntry = proofReplayCatalogEntryForSlug(replaySlug, origin)
    if (catalogEntry === undefined) {
      return yield* failReplayModule(
        input.binding.moduleRef,
        replaySlug,
        'Replay slug is not present in the public proof replay catalog.',
      )
    }

    const bundle = yield* input.runtime.buildProofReplayBundle({
      observedAt: input.observedAt,
      origin,
      replaySlug,
    })
    const renderPlan = buildReplayRenderPlan(bundle as SharedProofReplayBundle)
    const sourceRefs = uniqueStrings([
      ...catalogEntry.primarySourceRefs,
      ...bundle.sourceRefs.map(source => source.ref),
    ])
    const evidenceRefs = uniqueStrings([
      `evidence.openagents.blueprint_replay_module.${safeRefSegment(
        replaySlug,
      )}`,
      ...sourceRefs,
    ])
    const receiptRefs = uniqueStrings([
      'receipt.public_proof_replay_bundle',
      `receipt.openagents.blueprint_replay_module.${safeRefSegment(
        replaySlug,
      )}.${safeRefSegment(bundle.bundleRef)}`,
    ])

    return {
      authorityBoundary: 'evidence_only',
      bundle: bundle as unknown as Record<string, unknown>,
      bundleEndpoint: catalogEntry.bundleEndpoint,
      bundleRef: bundle.bundleRef,
      contentRedacted: true,
      directMutationDisabled: true,
      evidenceRefs,
      intentRef: input.intentRef,
      kind: 'blueprint_replay_module_evidence',
      moduleRef: input.binding.moduleRef,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      observedAt: input.observedAt,
      receiptRefs,
      renderPlan: renderPlan as unknown as Record<string, unknown>,
      replaySlug,
      replayViewSpec: {
        bundleEndpoint: catalogEntry.bundleEndpoint,
        bundleRef: bundle.bundleRef,
        replaySlug,
        ...(catalogEntry.socialPath === undefined
          ? {}
          : { socialPath: catalogEntry.socialPath }),
        websitePath: catalogEntry.websitePath,
      },
      sourceAuthority: bundle.sourceAuthority,
      sourceRefs,
      stepRef: input.binding.stepRef,
      summary: catalogEntry.summary,
      targetRef: input.targetRef ?? `proof_replay.${safeRefSegment(replaySlug)}`,
      title: catalogEntry.title,
      toolRef: input.toolRef,
    }
  })

const safeRefSegment = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, '.')
    .replace(/^\.+|\.+$/g, '')
    .slice(0, 96) || 'unknown'

const uniqueStrings = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values.filter(value => value.trim() !== ''))]

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const failReplayModule = (
  moduleRef: string,
  replaySlug: string,
  reason: string,
): Effect.Effect<never, BlueprintReplayModuleError> =>
  Effect.fail(new BlueprintReplayModuleError({ moduleRef, reason, replaySlug }))

export const DEFAULT_BLUEPRINT_REPLAY_SLUG = FIRST_REAL_SETTLEMENT_REPLAY_SLUG
