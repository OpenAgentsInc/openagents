// Voice-session transcript ingestion endpoint
// (EPIC #5523 / DE-7 #5530; promise
// mobile.voice_session_evidence_transcript_ingest.v1, red).
//
// INERT by default. This route is wired into the live Worker but is gated by
// VOICE_PROGRAM_INGEST_ENABLED. When the flag is OFF (default) the endpoint
// NEVER touches the ingest core: it returns an honest inert JSON
// (`inert: true`, `promiseState: 'red'`, all three named blockers listed) so
// no behavior changes on the deployed Worker.
//
// When the flag is ARMED, the endpoint decodes ALREADY-TRANSCRIBED, redacted,
// ref-only voice transcript segments + session metadata and runs the EXISTING
// pure, deterministic `buildVoiceProgramIngestProposal` core
// (voice-program-ingest.ts, #4992) to return an approval-gated program-input
// proposal (`proposesProgramInputOnly: true`, `executesProgramRun: false`).
//
// Authority boundary (carried verbatim from the ingest core): a voice
// transcript is EVIDENCE of intent, not command authority. This endpoint:
//   - performs NO speech-to-text (no STT vendor is chosen or called here);
//   - performs NO audio capture;
//   - performs NO mutation (transcripts/proposals/providers/payments);
//   - executes NO program run, NO command, NO payment, NO settlement, NO spend;
//   - upgrades NO public claim.
// It only PROPOSES a structured program input that downstream Blueprint
// program runs consume behind their own approval/release gates.
//
// This wiring clears ONLY blocker.product_promises.voice_ingestion_endpoint_missing.
// The other two blockers stay open and owner/product-gated:
//   - blocker.product_promises.voice_transcription_service_missing (STT vendor
//     + live audio capture is a product decision), and
//   - blocker.product_promises.voice_proposal_and_approval_ui_missing (the
//     approval UI).
// The promise stays red; no green flip is claimed (owner-signed per
// proof.claim_upgrade_receipts.v1).

import { badRequest, jsonResponse } from '@openagentsinc/sync-worker'
import { Effect, Exit } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import {
  OmniVoiceTranscriptSegment,
} from './omni-voice-session-evidence'
import {
  buildVoiceProgramIngestProposal,
  VOICE_INGEST_SUPPORTED_STAGES,
  VoiceProgramIngestSessionMetadata,
  VoiceProgramIngestUnsafe,
  type VoiceIngestSupportedStage,
  type VoiceProgramIngestOptions,
} from './voice-program-ingest'

type HttpResponse = globalThis.Response

export const VoiceProgramIngestEndpoint = '/api/mobile/voice-sessions/ingest'

export const VOICE_PROGRAM_INGEST_PROMISE_ID =
  'mobile.voice_session_evidence_transcript_ingest.v1'

// The three named blockers on the promise. This endpoint clears the first; the
// other two stay owner/product-gated and are always surfaced honestly.
export const VOICE_PROGRAM_INGEST_BLOCKERS = {
  cleared: 'blocker.product_promises.voice_ingestion_endpoint_missing',
  remaining: [
    'blocker.product_promises.voice_transcription_service_missing',
    'blocker.product_promises.voice_proposal_and_approval_ui_missing',
  ],
} as const

// Parse the VOICE_PROGRAM_INGEST_ENABLED flag. Default OFF: anything other than
// an explicit truthy token leaves the endpoint inert.
export const isVoiceProgramIngestEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type VoiceProgramIngestDeps = Readonly<{
  // Whether the ingestion endpoint is armed. When false (default) the endpoint
  // returns the inert response and never runs the ingest core.
  enabled: boolean
}>

type VoiceProgramIngestApprovalGate = Readonly<{
  approvalMutationAllowed: false
  approvalRequired: true
  approvalRequirement: 'operator_required'
  commandExecutionAllowed: false
  riskLabel: 'Medium risk'
  riskLevel: 'medium'
  state: 'needs_approval'
  stateLabel: 'Needs approval'
}>

const approvalGatePayload = (): VoiceProgramIngestApprovalGate => ({
  approvalMutationAllowed: false,
  approvalRequired: true,
  approvalRequirement: 'operator_required',
  commandExecutionAllowed: false,
  riskLabel: 'Medium risk',
  riskLevel: 'medium',
  state: 'needs_approval',
  stateLabel: 'Needs approval',
})

// The honest inert/disabled payload. Read-only, no live capability claimed.
const inertPayload = (): Record<string, unknown> => ({
  promiseId: VOICE_PROGRAM_INGEST_PROMISE_ID,
  promiseState: 'red',
  inert: true,
  enabled: false,
  // Echo the read-only authority boundary so callers can see this endpoint
  // never executes, mutates, captures audio, or settles.
  authorityBoundary: 'read_only_voice_session_evidence',
  executesProgramRun: false,
  proposesProgramInputOnly: true,
  capturesAudio: false,
  callsTranscriptionService: false,
  blockerCleared: VOICE_PROGRAM_INGEST_BLOCKERS.cleared,
  remainingBlockers: VOICE_PROGRAM_INGEST_BLOCKERS.remaining,
  note:
    'Voice transcript ingestion is flag-gated and INERT by default. Arm ' +
    'VOICE_PROGRAM_INGEST_ENABLED to accept already-transcribed, redacted, ' +
    'ref-only segments and return an approval-gated program-input proposal. ' +
    'No STT vendor or approval UI is wired; the promise stays red.',
})

interface VoiceProgramIngestRequest {
  readonly metadata: VoiceProgramIngestSessionMetadata
  readonly segments: ReadonlyArray<OmniVoiceTranscriptSegment>
  readonly stage: VoiceIngestSupportedStage | undefined
  readonly confidenceFloorBps: number | undefined
  readonly includeSpeakerRoles: ReadonlyArray<string> | undefined
}

const SUPPORTED_STAGE_VALUES: ReadonlyArray<string> = [
  ...VOICE_INGEST_SUPPORTED_STAGES.literals,
]

const isSupportedStage = (value: unknown): value is VoiceIngestSupportedStage =>
  typeof value === 'string' && SUPPORTED_STAGE_VALUES.includes(value)

// Decode the request body into the typed ingest inputs. Throws (caught by the
// caller into a safe 400) on any schema or shape violation — no raw body or
// internal detail leaks.
const decodeRequest = (
  body: Record<string, unknown>,
): VoiceProgramIngestRequest => {
  const metadata = decodeUnknownWithSchema(
    VoiceProgramIngestSessionMetadata,
    body.metadata,
  )

  const rawSegments = Array.isArray(body.segments) ? body.segments : []
  const segments = rawSegments.map(segment =>
    decodeUnknownWithSchema(OmniVoiceTranscriptSegment, segment),
  )

  const stage = isSupportedStage(body.stage) ? body.stage : undefined

  const confidenceFloorBps =
    typeof body.confidenceFloorBps === 'number'
      ? body.confidenceFloorBps
      : undefined

  const includeSpeakerRoles: ReadonlyArray<string> | undefined =
    Array.isArray(body.includeSpeakerRoles) &&
    body.includeSpeakerRoles.every(role => typeof role === 'string')
      ? body.includeSpeakerRoles.filter(
          (role): role is string => typeof role === 'string',
        )
      : undefined

  return {
    confidenceFloorBps,
    includeSpeakerRoles,
    metadata,
    segments,
    stage,
  }
}

// Extract the safe failure reason from a failed ingest Exit. The core fails
// (via Effect.try) with a VoiceProgramIngestUnsafe carried in a `Fail` cause
// node; we read it structurally so no Cause namespace helper is required, and
// fall back to a generic safe reason if the shape is unexpected. Never leaks
// transcript content.
const FALLBACK_INGEST_REASON =
  'Voice program ingest failed to build a proposal.'

// Read the `error` payload from a `Fail` cause node without a type assertion.
const failCauseError = (cause: unknown): unknown => {
  if (
    typeof cause === 'object' &&
    cause !== null &&
    '_tag' in cause &&
    cause._tag === 'Fail' &&
    'error' in cause
  ) {
    return cause.error
  }
  return undefined
}

const voiceIngestFailureReason = (
  exit: Exit.Exit<unknown, VoiceProgramIngestUnsafe>,
): string => {
  if (exit._tag !== 'Failure') {
    return FALLBACK_INGEST_REASON
  }
  const error = failCauseError(exit.cause)
  return error instanceof VoiceProgramIngestUnsafe
    ? error.reason
    : FALLBACK_INGEST_REASON
}

/**
 * Handle a voice-session transcript ingestion request.
 *
 * POST only. Flag-gated: returns the inert payload when disabled (default).
 * When armed, decodes ref-only segments + metadata and returns the
 * approval-gated program-input proposal from the pure ingest core. No STT, no
 * mutation, no execution.
 */
export const handleVoiceProgramIngestApi = async (
  request: Request,
  deps: VoiceProgramIngestDeps,
): Promise<HttpResponse> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  if (!deps.enabled) {
    return noStoreJsonResponse(inertPayload())
  }

  let decoded: VoiceProgramIngestRequest
  try {
    const body = await readJsonObject(request)
    decoded = decodeRequest(body)
  } catch {
    return badRequest(
      'Invalid voice ingest request: expected { metadata, segments } with ' +
        'read-only authority and ref-only redacted transcript segments.',
    )
  }

  // The core is a pure Effect that fails with VoiceProgramIngestUnsafe on any
  // read-only-authority or ref-safety violation. Fold success/failure into a
  // tagged Exit, then branch on it (no Cause traversal needed). A failure
  // surfaces the safe reason without leaking any transcript content.
  // Build the options object with only the keys the caller actually provided,
  // so exactOptionalPropertyTypes is satisfied (no explicit-undefined keys).
  const ingestOptions: VoiceProgramIngestOptions = {
    ...(decoded.stage !== undefined ? { stage: decoded.stage } : {}),
    ...(decoded.confidenceFloorBps !== undefined
      ? { confidenceFloorBps: decoded.confidenceFloorBps }
      : {}),
    ...(decoded.includeSpeakerRoles !== undefined
      ? { includeSpeakerRoles: decoded.includeSpeakerRoles }
      : {}),
  }

  const exit = await Effect.runPromiseExit(
    buildVoiceProgramIngestProposal(
      decoded.segments,
      decoded.metadata,
      ingestOptions,
    ),
  )

  if (exit._tag === 'Failure') {
    return jsonResponse(
      {
        error: 'voice_program_ingest_unsafe',
        reason: voiceIngestFailureReason(exit),
      },
      { status: 422 },
    )
  }

  const proposal = exit.value

  // Honest envelope: the proposal proposes program INPUT only; nothing executed.
  return noStoreJsonResponse({
    promiseId: VOICE_PROGRAM_INGEST_PROMISE_ID,
    promiseState: 'red' as const,
    enabled: true as const,
    executesProgramRun: false as const,
    proposesProgramInputOnly: true as const,
    approvalGate: approvalGatePayload(),
    blockerCleared: VOICE_PROGRAM_INGEST_BLOCKERS.cleared,
    remainingBlockers: VOICE_PROGRAM_INGEST_BLOCKERS.remaining,
    proposal,
  })
}
