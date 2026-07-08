import { Effect, Schema as S } from 'effect'

export const DEFAULT_BLUEPRINT_REPLAY_SLUG = 'archived-proof-replay'

export type BlueprintReplayModuleRuntimeInput = Readonly<{
  replaySlug?: string | undefined
  targetRef?: string | undefined
}>
export type BlueprintReplayModuleEvidence = Readonly<{
  archived: true
  bundle: Readonly<Record<string, unknown>>
  bundleRef: string
  evidenceRefs: ReadonlyArray<string>
  reason: string
  receiptRefs: ReadonlyArray<string>
  renderPlan: Readonly<Record<string, unknown>>
  replaySlug: string
  replayViewSpec: BlueprintReplayModuleViewSpec
  stepRef: string
  toolRef: string
}>
export const BlueprintReplayModuleEvidence = S.Struct({
  archived: S.Literal(true),
  bundle: S.Record(S.String, S.Unknown),
  bundleRef: S.String,
  evidenceRefs: S.Array(S.String),
  reason: S.String,
  receiptRefs: S.Array(S.String),
  renderPlan: S.Record(S.String, S.Unknown),
  replaySlug: S.String,
  replayViewSpec: S.Struct({
    archived: S.Literal(true),
    replaySlug: S.String,
  }),
  stepRef: S.String,
  toolRef: S.String,
})
export type BlueprintReplayModuleViewSpec = Readonly<{
  archived: true
  replaySlug: string
}>
export const BlueprintReplayModuleViewSpec = S.Struct({
  archived: S.Literal(true),
  replaySlug: S.String,
})
export type BlueprintReplayModuleRuntime = Readonly<{
  render: (
    input: BlueprintReplayModuleRuntimeInput,
  ) => Effect.Effect<BlueprintReplayModuleViewSpec, never>
}>
export class BlueprintReplayModuleError extends Error {
  readonly reason = 'archived_to_backroom'
}

export const executeBlueprintReplayModule = (
  input: Readonly<{
    [key: string]: unknown
    binding?: { stepRef?: string } | undefined
    intentRef?: string | undefined
    replaySlug?: string | undefined
    runtime?: BlueprintReplayModuleRuntime | undefined
    targetRef?: string | undefined
    toolRef?: string | undefined
  }>,
): Effect.Effect<BlueprintReplayModuleEvidence, BlueprintReplayModuleError> =>
  Effect.succeed({
    archived: true,
    bundle: { archived: true },
    bundleRef: 'bundle.blueprint.proof_replay.archived',
    evidenceRefs: ['backroom:openagents-prune-20260708-tassadar-psionic'],
    reason: 'Proof replay rendering was retired and archived to backroom.',
    receiptRefs: ['receipt.blueprint.proof_replay.archived_to_backroom'],
    renderPlan: { archived: true },
    replaySlug: input.replaySlug ?? DEFAULT_BLUEPRINT_REPLAY_SLUG,
    replayViewSpec: {
      archived: true,
      replaySlug: input.replaySlug ?? DEFAULT_BLUEPRINT_REPLAY_SLUG,
    },
    stepRef: input.binding?.stepRef ?? 'step.blueprint.proof_replay.archived',
    toolRef: input.toolRef ?? 'tool.blueprint.proof_replay.archived',
  })

export const makePublicProofReplayModuleRuntime = (
  ..._args: unknown[]
): BlueprintReplayModuleRuntime => ({
  render: input =>
    Effect.succeed({
      archived: true,
      replaySlug: input.replaySlug ?? DEFAULT_BLUEPRINT_REPLAY_SLUG,
    }),
})
