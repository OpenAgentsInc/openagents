import { Effect, Schema as S } from 'effect'

import {
  type BlueprintProgramRunRecord,
  blueprintProgramRunIsEvidenceOnly,
} from '../schemas/program-run'

export const BlueprintProgramRunDirectEffectKind = S.Literals([
  'create_pull_request',
  'deploy',
  'mutate_source_fact',
  'send_email',
  'spend_money',
  'upgrade_public_claim',
])
export type BlueprintProgramRunDirectEffectKind =
  typeof BlueprintProgramRunDirectEffectKind.Type

export class BlueprintProgramRunDirectEffectDenied extends S.TaggedErrorClass<BlueprintProgramRunDirectEffectDenied>()(
  'BlueprintProgramRunDirectEffectDenied',
  {
    effectKind: BlueprintProgramRunDirectEffectKind,
    programRunId: S.String,
    reason: S.String,
  },
) {}

export type BlueprintProgramRunAuthorityError =
  BlueprintProgramRunDirectEffectDenied

export const denyProgramRunDirectEffect = (
  run: BlueprintProgramRunRecord,
  effectKind: BlueprintProgramRunDirectEffectKind,
): Effect.Effect<never, BlueprintProgramRunDirectEffectDenied> =>
  Effect.fail(
    new BlueprintProgramRunDirectEffectDenied({
      effectKind,
      programRunId: run.id,
      reason:
        'Blueprint Program Runs are evidence-only. Direct effects must be represented as approval-gated Action Submissions.',
    }),
  )

export const assertProgramRunEvidenceOnly = (
  run: BlueprintProgramRunRecord,
): Effect.Effect<BlueprintProgramRunRecord, BlueprintProgramRunDirectEffectDenied> =>
  blueprintProgramRunIsEvidenceOnly(run)
    ? Effect.succeed(run)
    : Effect.fail(
        new BlueprintProgramRunDirectEffectDenied({
          effectKind: 'mutate_source_fact',
          programRunId: run.id,
          reason:
            'Blueprint Program Run record contains write-authority flags and cannot be accepted as evidence-only.',
        }),
      )
