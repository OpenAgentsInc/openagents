import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BlueprintProgramRunRecord,
  blueprintProgramRunIsEvidenceOnly,
} from '../schemas/program-run'
import { BlueprintProgramRunDirectEffectDenied } from './program-run-authority'
import {
  executeBlueprintChatProgramTurn,
  type BlueprintChatProgramRuntimePrimitives,
  type BlueprintChatProgramSessionRuntime,
} from './chat-program-runtime'
import {
  executeKhalaProgramTurn,
  khalaTurnToProgramRunRecord,
  KHALA_PROGRAM_MODEL,
  makeKhalaOfferSessionRuntime,
  type KhalaProgramTurnRequest,
} from './chat-program-runtime-khala'

const deterministicRuntime: BlueprintChatProgramRuntimePrimitives = {
  makeLookupId: () => 'khala_signature_lookup.test',
  makeMenuId: () => 'khala_tool_menu.test',
  makeProgramRunId: () => 'khala_program_run.test',
  nowIso: () => '2026-06-24T12:00:00.000Z',
}

const baseRequest = (
  overrides: Partial<KhalaProgramTurnRequest> = {},
): KhalaProgramTurnRequest => ({
  turnRef: 'khala.turn.1',
  promptSummaryRef: 'prompt_summary.khala.turn_1',
  inputSnapshotHash: 'sha256:khala_turn_1',
  sessionObjectiveRef: 'objective_ref.khala.turn_1',
  allowedSurfaces: ['agent_api'],
  supportedToolRefs: ['tool.context_pack.read', 'tool.action_submission.propose'],
  riskCeiling: 'medium',
  preferredFamily: 'continuation',
  contextPackRef: 'context_pack.khala.turn_1',
  sourceAuthorityRefs: ['source_authority.repo.openagents'],
  backendCapabilityRefs: ['capability.autopilot.session_spawn'],
  ...overrides,
})

const turnInput = (req: KhalaProgramTurnRequest) => ({
  actorRef: 'actor.khala.public_chat',
  allowedSurfaces: [...req.allowedSurfaces],
  backendCapabilityRefs: [...req.backendCapabilityRefs],
  backendKind: 'claude_agent',
  backendProfileId: 'backend_profile.khala.inference',
  costRef: `cost.khala_program.${req.turnRef}`,
  inputSnapshotHash: req.inputSnapshotHash,
  model: KHALA_PROGRAM_MODEL,
  preferredFamily: req.preferredFamily ?? ('continuation' as const),
  promptSummaryRef: req.promptSummaryRef,
  riskCeiling: req.riskCeiling,
  routeRef: 'route.khala.chat_program',
  sessionAdapter: 'claude_agent' as const,
  sessionObjectiveRef: req.sessionObjectiveRef,
  sourceAuthorityRefs: [...req.sourceAuthorityRefs],
  supportedToolRefs: [...req.supportedToolRefs],
  turnRef: req.turnRef,
  ...(req.contextPackRef === undefined ? {} : { contextPackRef: req.contextPackRef }),
})

describe('Khala → Blueprint program wiring (spec §B, evidence-only)', () => {
  test('one Khala turn emits an evidence-only BlueprintProgramRunRecord', async () => {
    const record = await Effect.runPromise(
      khalaTurnToProgramRunRecord({
        request: baseRequest(),
        runtime: deterministicRuntime,
      }),
    )

    expect(S.decodeUnknownSync(BlueprintProgramRunRecord)(record)).toEqual(record)
    expect(record.authorityBoundary).toBe('evidence_only')
    expect(record.directMutationDisabled).toBe(true)
    expect(record.noDeploy).toBe(true)
    expect(record.noEmail).toBe(true)
    expect(record.noSpend).toBe(true)
    expect(record.noSourceMutation).toBe(true)
    expect(blueprintProgramRunIsEvidenceOnly(record)).toBe(true)
    // run = receipt: the record carries the program-run receipt + the Khala model
    expect(record.receiptRefs).toContain('receipt.program_run')
    expect(record.metadata.model).toBe(KHALA_PROGRAM_MODEL)
  })

  test('selection rides typed constraints (no keyword routing) and resolves a signature', async () => {
    const result = await Effect.runPromise(
      executeKhalaProgramTurn({ request: baseRequest(), runtime: deterministicRuntime }),
    )
    expect(result.lookup.programSignatureIds.length).toBeGreaterThan(0)
    expect(result.programRun.programSignatureId).toMatch(/^program_signature\./)
    expect(result.turnRef).toBe('khala.turn.1')
  })

  test('a requested direct effect from the session is DENIED (no write path)', async () => {
    const req = baseRequest()
    const denyingSession: BlueprintChatProgramSessionRuntime = {
      spawnSession: input =>
        makeKhalaOfferSessionRuntime(req, deterministicRuntime)
          .spawnSession(input)
          .pipe(
            Effect.map(result => ({
              ...result,
              requestedDirectEffects: ['deploy' as const],
            })),
          ),
    }

    await expect(
      Effect.runPromise(
        executeBlueprintChatProgramTurn({
          runtime: deterministicRuntime,
          sessionRuntime: denyingSession,
          turn: turnInput(req),
        }),
      ),
    ).rejects.toBeInstanceOf(BlueprintProgramRunDirectEffectDenied)
  })
})
