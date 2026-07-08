import { Effect, Schema as S } from 'effect'

export const BLUEPRINT_TASSADAR_DENSE_FIXTURE_MODULE_REF =
  'module.blueprint.tassadar.dense.archived'
export const BLUEPRINT_TASSADAR_LINKED_FIXTURE_MODULE_REF =
  'module.blueprint.tassadar.linked_dense.archived'
export const BLUEPRINT_TASSADAR_MODULE_FIXTURE_REGISTRY_REF =
  'registry.blueprint.tassadar_modules.archived_to_backroom'
export const BLUEPRINT_TASSADAR_DENSE_MODULE_CLAIM_CLASS =
  'claim.blueprint.tassadar_dense_module.archived'
export const BLUEPRINT_TASSADAR_MODULE_REQUIRED_TRUST_POSTURE =
  'archived_to_backroom'

export const BlueprintTassadarModuleStepVerdict = S.Literals(['refused'])
export type BlueprintTassadarModuleStepVerdict =
  typeof BlueprintTassadarModuleStepVerdict.Type
export type BlueprintTassadarModuleStepEvidence = Readonly<{
  archived: true
  evidenceRefs: ReadonlyArray<string>
  moduleRef: string
  reason: string
  receiptRefs: ReadonlyArray<string>
  stepRef: string
  toolRef: string
  verdict: BlueprintTassadarModuleStepVerdict
}>
export const BlueprintTassadarModuleStepEvidence = S.Struct({
  archived: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  moduleRef: S.String,
  reason: S.String,
  receiptRefs: S.Array(S.String),
  stepRef: S.String,
  toolRef: S.String,
  verdict: BlueprintTassadarModuleStepVerdict,
})
export type BlueprintTassadarModuleStepRefused = Readonly<{
  reason: 'archived'
}>
export const BlueprintTassadarModuleStepRefused = S.Struct({
  reason: S.Literal('archived'),
})
export type BlueprintTassadarModuleStepUnsafe = Readonly<{
  reason: 'archived'
}>
export const BlueprintTassadarModuleStepUnsafe = S.Struct({
  reason: S.Literal('archived'),
})
export class BlueprintTassadarModuleStepError extends Error {
  readonly reason = 'archived_to_backroom'
}

export const executeBlueprintTassadarModuleStep = (
  input: Readonly<{
    [key: string]: unknown
    stepRef?: string
    toolRef?: string
    tassadarModuleStep?: { moduleRef?: string; stepRef?: string } | undefined
  }>,
  _options?: unknown,
): Effect.Effect<
  BlueprintTassadarModuleStepEvidence,
  BlueprintTassadarModuleStepError
> =>
  Effect.succeed({
    archived: true,
    evidenceRefs: ['backroom:openagents-prune-20260708-tassadar-psionic'],
    moduleRef:
      input.tassadarModuleStep?.moduleRef ??
      'module.blueprint.tassadar.archived',
    reason: 'Tassadar module execution was retired and archived to backroom.',
    receiptRefs: ['receipt.blueprint.tassadar.archived_to_backroom'],
    stepRef:
      input.stepRef ??
      input.tassadarModuleStep?.stepRef ??
      'step.blueprint.tassadar.archived',
    toolRef: input.toolRef ?? 'tool.blueprint.tassadar.archived',
    verdict: 'refused',
  })

export const makeTassadarModuleStepRuntime = (..._args: unknown[]) => ({
  execute: executeBlueprintTassadarModuleStep,
})
