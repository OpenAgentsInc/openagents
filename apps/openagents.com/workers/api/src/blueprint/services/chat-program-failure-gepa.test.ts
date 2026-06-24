import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { BlueprintProgramRunRecord } from '../schemas/program-run'
import {
  GepaCandidateFeedback,
  GepaCandidateFeedbackError,
  PROGRAM_FAILURE_GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION,
  PROGRAM_FAILURE_GEPA_RELEASE_GATE_REF,
  emitGepaCandidateFeedback,
  evaluateGepaCandidateReleaseGateWithoutApproval,
} from './chat-program-failure-gepa'
import {
  KHALA_PROGRAM_MODEL,
  khalaFailureTurnToGepaCandidateFeedback,
} from './chat-program-runtime-khala'
import type { KhalaProgramTurnRequest } from './chat-program-runtime-khala'
import type { BlueprintChatProgramRuntimePrimitives } from './chat-program-runtime'

const deterministicRuntime: BlueprintChatProgramRuntimePrimitives = {
  makeLookupId: () => 'khala_signature_lookup.test',
  makeMenuId: () => 'khala_tool_menu.test',
  makeProgramRunId: () => 'khala_program_run.test',
  nowIso: () => '2026-06-24T12:00:00.000Z',
}

const baseRequest = (): KhalaProgramTurnRequest => ({
  turnRef: 'khala.turn.fail.1',
  promptSummaryRef: 'prompt_summary.khala.turn_fail_1',
  inputSnapshotHash: 'sha256:khala_turn_fail_1',
  sessionObjectiveRef: 'objective_ref.khala.turn_fail_1',
  allowedSurfaces: ['agent_api'],
  supportedToolRefs: ['tool.context_pack.read', 'tool.action_submission.propose'],
  riskCeiling: 'medium',
  preferredFamily: 'continuation',
  contextPackRef: 'context_pack.khala.turn_fail_1',
  sourceAuthorityRefs: ['source_authority.repo.openagents'],
  backendCapabilityRefs: ['capability.autopilot.session_spawn'],
})

const findings = [
  {
    claimId: 'claims-redirect',
    claim: '/login redirects away (FALSE claim under test)',
    evidenceSummary: 'observed step "redirects away from /login" = failed',
  },
]

const evidenceOnlyRun = (): BlueprintProgramRunRecord => ({
  actorRef: 'actor.khala.public_chat',
  archivedAt: null,
  authorityBoundary: 'evidence_only',
  confidence: 0.8,
  costRef: 'cost.khala_program.test',
  createdAt: '2026-06-24T12:00:00.000Z',
  directMutationDisabled: true,
  evidenceRefs: ['evidence.khala_offer.response.test'],
  id: 'khala_program_run.test',
  idempotencyKey: 'khala_program_run.test.idem',
  inputSnapshotHash: 'sha256:khala_turn_fail_1',
  latencyMs: 0,
  metadata: { model: KHALA_PROGRAM_MODEL },
  moduleVersionId: 'module_version.khala.test',
  noDeploy: true,
  noEmail: true,
  noSourceMutation: true,
  noSpend: true,
  programSignatureId: 'program_signature.continuation.test',
  programTypeId: 'program_type.continuation',
  purposeRef: 'purpose.khala.chat',
  receiptRefs: ['receipt.program_run'],
  routeRef: 'route.khala.chat_program',
  typedOutput: { responseRef: 'response.khala_offer.test', state: 'completed' },
  updatedAt: '2026-06-24T12:00:00.000Z',
})

describe('QA failure learning → GEPA candidate-feedback (worker Blueprint surface, #6195)', () => {
  test('emits an evidence-only, governed candidate-feedback signal from a failed run', async () => {
    const feedback = await Effect.runPromise(
      emitGepaCandidateFeedback({ programRun: evidenceOnlyRun(), trigger: 'REFUTED', findings }),
    )
    // decodes against the schema (every governance literal pinned)
    expect(S.decodeUnknownSync(GepaCandidateFeedback)(feedback)).toEqual(feedback)
    expect(feedback.schemaVersion).toBe(PROGRAM_FAILURE_GEPA_CANDIDATE_FEEDBACK_SCHEMA_VERSION)
    expect(feedback.schemaVersion).toContain('psionic.probe_gepa_candidate')
    expect(feedback.trigger).toBe('REFUTED')
    expect(feedback.optimizerKind).toBe('gepa_style_reflection')
    expect(feedback.model).toBe(KHALA_PROGRAM_MODEL)
    expect(feedback.sourceProgramRunRef).toBe('khala_program_run.test')
    expect(feedback.items).toHaveLength(1)
    expect(feedback.items[0]!.polarity).toBe('negative')
    // governance: evidence-only, gated, never self-promoted, not live
    expect(feedback.governance.authorityBoundary).toBe('evidence_only')
    expect(feedback.governance.requiresReleaseGate).toBe(true)
    expect(feedback.governance.selfPromotionAllowed).toBe(false)
    expect(feedback.governance.live).toBe(false)
    expect(feedback.governance.releaseGateRef).toBe(PROGRAM_FAILURE_GEPA_RELEASE_GATE_REF)
  })

  test('the Release Gate REJECTS an unapproved candidate (no self-promotion)', async () => {
    const feedback = await Effect.runPromise(
      emitGepaCandidateFeedback({ programRun: evidenceOnlyRun(), trigger: 'REFUTED', findings }),
    )
    const decision = evaluateGepaCandidateReleaseGateWithoutApproval(feedback)
    expect(decision.promoted).toBe(false)
    expect(decision.reason).toContain('release_gate_rejected')
    expect(decision.reason).toContain('requires an explicit operator approval')
  })

  test('fails closed when the source program-run record carries write authority', async () => {
    const writeRun = { ...evidenceOnlyRun(), noSpend: false }
    await expect(
      Effect.runPromise(
        emitGepaCandidateFeedback({ programRun: writeRun, trigger: 'REFUTED', findings }),
      ),
    ).rejects.toBeInstanceOf(GepaCandidateFeedbackError)
  })

  test('fails closed when there are no contradicted findings (never fabricates feedback)', async () => {
    await expect(
      Effect.runPromise(
        emitGepaCandidateFeedback({ programRun: evidenceOnlyRun(), trigger: 'REFUTED', findings: [] }),
      ),
    ).rejects.toBeInstanceOf(GepaCandidateFeedbackError)
  })

  test('the Khala surface wires a failed turn to a governed candidate-feedback signal', async () => {
    const feedback = await Effect.runPromise(
      khalaFailureTurnToGepaCandidateFeedback({
        request: baseRequest(),
        runtime: deterministicRuntime,
        trigger: 'REFUTED',
        findings,
        optimizerKind: 'retained_failure_replay',
      }),
    )
    expect(feedback.optimizerKind).toBe('retained_failure_replay')
    expect(feedback.governance.authorityBoundary).toBe('evidence_only')
    expect(feedback.governance.selfPromotionAllowed).toBe(false)
    // the gate still rejects it unapproved
    expect(evaluateGepaCandidateReleaseGateWithoutApproval(feedback).promoted).toBe(false)
  })
})
