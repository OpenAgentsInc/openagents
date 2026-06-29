import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BlueprintProgramRunRecord } from '../schemas/program-run'
import {
  assertProgramRunEvidenceOnly,
  BlueprintProgramRunDirectEffectDenied,
  type BlueprintProgramRunDirectEffectKind,
  denyProgramRunDirectEffect,
} from './program-run-authority'

const evidenceOnlyRun: BlueprintProgramRunRecord = {
  actorRef: 'actor.adjutant',
  archivedAt: null,
  authorityBoundary: 'evidence_only',
  confidence: 0.9,
  costRef: 'cost.program_run_1',
  createdAt: '2026-06-05T00:00:00.000Z',
  directMutationDisabled: true,
  evidenceRefs: ['evidence.context_pack_1'],
  id: 'blueprint_program_run_1',
  idempotencyKey: 'program-run:1',
  inputSnapshotHash: 'sha256.input_snapshot_1',
  latencyMs: 900,
  metadata: {},
  moduleVersionId: 'module_version.continuation_prompt.v1',
  noDeploy: true,
  noEmail: true,
  noSourceMutation: true,
  noSpend: true,
  programSignatureId: 'program_signature.autopilot_continuation.v1',
  programTypeId: 'program_type.autopilot_continuation',
  purposeRef: 'purpose.decide_next_step',
  receiptRefs: ['receipt.program_run_1'],
  routeRef: 'route.continue',
  typedOutput: { action: 'continue' },
  updatedAt: '2026-06-05T00:00:00.000Z',
}

describe('Blueprint Program Run authority service', () => {
  test('accepts evidence-only runs for evidence handling', async () => {
    await expect(
      Effect.runPromise(assertProgramRunEvidenceOnly(evidenceOnlyRun)),
    ).resolves.toStrictEqual(evidenceOnlyRun)
  })

  test.each([
    'create_pull_request',
    'deploy',
    'mutate_source_fact',
    'send_email',
    'spend_money',
    'upgrade_public_claim',
  ] satisfies ReadonlyArray<BlueprintProgramRunDirectEffectKind>)(
    'denies %s from Program Run authority',
    async effectKind => {
      await expect(
        Effect.runPromise(
          denyProgramRunDirectEffect(evidenceOnlyRun, effectKind),
        ),
      ).rejects.toBeInstanceOf(BlueprintProgramRunDirectEffectDenied)
    },
  )

  test('rejects any Program Run record that carries write authority flags', async () => {
    await expect(
      Effect.runPromise(
        assertProgramRunEvidenceOnly({
          ...evidenceOnlyRun,
          noDeploy: false,
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintProgramRunDirectEffectDenied)
  })
})
