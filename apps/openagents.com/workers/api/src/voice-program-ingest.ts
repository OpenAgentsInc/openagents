import { Effect, Schema as S } from 'effect'

import {
  deliveryPipelineProgramForStage,
  deliveryPipelineProgramTypeId,
} from './blueprint/delivery-pipeline-programs'
import {
  OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
  OmniVoiceSessionAuthority,
  OmniVoiceTranscriptSegment,
} from './omni-voice-session-evidence'

// ---------------------------------------------------------------------------
// Voice transcript -> delivery-pipeline program INPUT ingest core
// (WS-G, OpenAgents #4992)
//
// This module is the PURE, vendor-agnostic core that maps voice-session
// transcript segments into a structured PROGRAM INPUT payload for the
// brand-story delivery-pipeline stage (with a typed hook for web-copy).
//
// Read-only authority boundary (from omni-voice-session-evidence.ts):
//   - It PROPOSES a program input. It does NOT execute a program run, does NOT
//     mutate transcripts/proposals/providers/payments, and does NOT itself
//     produce the program OUTPUT (brand story copy). Producing the output and
//     publishing it stays behind the Blueprint program run + Action Submission
//     + per-stage operator_review release gate.
//   - The actual speech-to-text vendor and live audio capture are
//     owner/desktop-gated and OUT OF SCOPE here. This core never calls an STT
//     vendor; it only consumes already-transcribed, redacted-ref-only segments
//     and produces a deterministic proposal.
//
// Determinism: given the same segments + metadata + options, this function
// always produces byte-identical output. Refs are deduplicated and sorted, and
// only segments that pass the deterministic confidence floor and have content
// (a textRef) and source/evidence refs are admitted.
// ---------------------------------------------------------------------------

/**
 * Stages this ingest core can target. brand-story is the primary lane;
 * web-copy is the typed hook for the next stage in the pipeline. Both stages
 * accept the same shaped program-input payload (upstream context refs +
 * voice-derived evidence/source refs).
 */
export const VOICE_INGEST_SUPPORTED_STAGES = S.Literals([
  'brand-story',
  'web-copy',
])
export type VoiceIngestSupportedStage =
  typeof VOICE_INGEST_SUPPORTED_STAGES.Type

/**
 * Default confidence floor (basis points) below which a transcript segment is
 * treated as too low-confidence to contribute to a program input. 0..10000.
 */
export const VOICE_INGEST_DEFAULT_CONFIDENCE_FLOOR_BPS = 6_000

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/

export class VoiceProgramIngestUnsafe extends S.TaggedErrorClass<VoiceProgramIngestUnsafe>()(
  'VoiceProgramIngestUnsafe',
  {
    reason: S.String,
  },
) {}

/**
 * The deterministic, redacted-ref-only metadata that describes the voice
 * session feeding a program input. These are all refs (never raw transcript
 * text, audio, names, or contact info), consistent with the voice session
 * read-only evidence authority.
 */
export class VoiceProgramIngestSessionMetadata extends S.Class<VoiceProgramIngestSessionMetadata>(
  'VoiceProgramIngestSessionMetadata',
)({
  authority: OmniVoiceSessionAuthority,
  evidenceRefs: S.Array(S.String),
  languageRef: S.String,
  redactionPolicyRefs: S.Array(S.String),
  sessionRef: S.String,
  sourceRefs: S.Array(S.String),
  workroomRef: S.String,
}) {}

/**
 * One admitted transcript segment, reduced to the deterministic, ref-only
 * fields the program input cares about. textRef is the upstream-redacted
 * pointer to transcript content; it is NOT raw transcript text.
 */
export class VoiceProgramIngestSegmentInput extends S.Class<VoiceProgramIngestSegmentInput>(
  'VoiceProgramIngestSegmentInput',
)({
  confidenceBps: S.Number,
  evidenceRefs: S.Array(S.String),
  segmentRef: S.String,
  sourceRefs: S.Array(S.String),
  speakerRole: S.String,
  textRef: S.String,
}) {}

/**
 * The structured PROGRAM INPUT proposal. This is the payload a coordinator
 * would hand to a Blueprint program RUN for the target stage; it is evidence
 * the run consumes, not the run output and not a write.
 */
export class VoiceProgramIngestProposal extends S.Class<VoiceProgramIngestProposal>(
  'VoiceProgramIngestProposal',
)({
  // Authority echo: this payload only proposes input; the program run is the
  // execution boundary and stays behind approval/release gates.
  authority: OmniVoiceSessionAuthority,
  proposesProgramInputOnly: S.Literal(true),
  executesProgramRun: S.Literal(false),

  // Target program identity (matches delivery-pipeline-programs.ts).
  stage: VOICE_INGEST_SUPPORTED_STAGES,
  programTypeId: S.String,
  outputSchemaRef: S.String,

  // Voice session provenance, ref-only.
  sessionRef: S.String,
  languageRef: S.String,

  // Upstream context the program run will read. These map onto the program's
  // contextPack / upstream-artifact evidence requirements.
  contextRefs: S.Array(S.String),

  // The admitted, deterministic segment inputs (ref-only).
  segmentInputs: S.Array(VoiceProgramIngestSegmentInput),

  // Aggregate evidence + source refs preserved from the admitted segments and
  // the session metadata (deduplicated + sorted).
  evidenceRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),

  // Deterministic diagnostics.
  admittedSegmentCount: S.Number,
  skippedSegmentCount: S.Number,
  confidenceFloorBps: S.Number,
  // null when no segments were admitted.
  meanConfidenceBps: S.NullOr(S.Number),
}) {}

export interface VoiceProgramIngestOptions {
  readonly stage?: VoiceIngestSupportedStage
  readonly confidenceFloorBps?: number
  /**
   * Speaker roles whose segments are admitted as program-input signal. Defaults
   * to user + operator + customer (the people whose words drive brand-story
   * content), excluding the agent/system echo.
   */
  readonly includeSpeakerRoles?: ReadonlyArray<string>
}

const DEFAULT_INCLUDE_SPEAKER_ROLES: ReadonlyArray<string> = [
  'customer',
  'operator',
  'user',
]

const uniqueSortedRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertReadOnlyAuthority = (
  authority: OmniVoiceSessionAuthority,
): void => {
  if (
    authority.authorityBoundary !== 'read_only_voice_session_evidence' ||
    authority.noApprovalMutation !== true ||
    authority.noAudioCaptureMutation !== true ||
    authority.noCommandExecution !== true ||
    authority.noPaymentMutation !== true ||
    authority.noProposalMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noTranscriptMutation !== true
  ) {
    throw new VoiceProgramIngestUnsafe({
      reason:
        'Voice program ingest only accepts read-only voice session evidence; it cannot capture audio, mutate transcripts/proposals/providers/payments, approve, execute, or upgrade public claims.',
    })
  }
}

const assertSafeRef = (label: string, ref: string): void => {
  if (!safeRefPattern.test(ref)) {
    throw new VoiceProgramIngestUnsafe({
      reason: `${label} must be a safe redacted ref.`,
    })
  }
}

const assertConfidenceFloor = (value: number): void => {
  if (!Number.isInteger(value) || value < 0 || value > 10_000) {
    throw new VoiceProgramIngestUnsafe({
      reason:
        'Voice program ingest confidence floor must be an integer from 0 to 10000 basis points.',
    })
  }
}

const segmentIsAdmissible = (
  segment: OmniVoiceTranscriptSegment,
  floorBps: number,
  includeRoles: ReadonlySet<string>,
): boolean =>
  Number.isInteger(segment.confidenceBps) &&
  segment.confidenceBps >= floorBps &&
  includeRoles.has(segment.speakerRole) &&
  segment.textRef.trim() !== '' &&
  segment.sourceRefs.length > 0 &&
  segment.evidenceRefs.length > 0

const compareSegmentInputs = (
  a: VoiceProgramIngestSegmentInput,
  b: VoiceProgramIngestSegmentInput,
): number => (a.segmentRef < b.segmentRef ? -1 : a.segmentRef > b.segmentRef ? 1 : 0)

/**
 * Pure, vendor-agnostic, deterministic mapping of voice transcript segments to
 * a delivery-pipeline program INPUT proposal. No STT calls, no mutation, no
 * program execution. Returns an Effect that fails with VoiceProgramIngestUnsafe
 * when the read-only authority or ref-safety contract is violated.
 */
export const buildVoiceProgramIngestProposal = (
  segments: ReadonlyArray<OmniVoiceTranscriptSegment>,
  metadata: VoiceProgramIngestSessionMetadata,
  options: VoiceProgramIngestOptions = {},
): Effect.Effect<VoiceProgramIngestProposal, VoiceProgramIngestUnsafe> =>
  Effect.try({
    catch: error =>
      error instanceof VoiceProgramIngestUnsafe
        ? error
        : new VoiceProgramIngestUnsafe({
            reason: 'Voice program ingest failed to build a proposal.',
          }),
    try: () => {
      const stage: VoiceIngestSupportedStage = options.stage ?? 'brand-story'
      const floorBps =
        options.confidenceFloorBps ?? VOICE_INGEST_DEFAULT_CONFIDENCE_FLOOR_BPS

      assertReadOnlyAuthority(metadata.authority)
      assertConfidenceFloor(floorBps)

      const program = deliveryPipelineProgramForStage(stage)
      if (program === undefined) {
        throw new VoiceProgramIngestUnsafe({
          reason: `No delivery-pipeline program is declared for stage ${stage}.`,
        })
      }

      assertSafeRef('Voice session sessionRef', metadata.sessionRef)
      assertSafeRef('Voice session languageRef', metadata.languageRef)
      assertSafeRef('Voice session workroomRef', metadata.workroomRef)
      ;[
        ...metadata.evidenceRefs,
        ...metadata.sourceRefs,
        ...metadata.redactionPolicyRefs,
      ].forEach(ref => assertSafeRef('Voice session metadata ref', ref))

      if (metadata.sourceRefs.length === 0) {
        throw new VoiceProgramIngestUnsafe({
          reason: 'Voice session metadata requires at least one source ref.',
        })
      }
      if (metadata.redactionPolicyRefs.length === 0) {
        throw new VoiceProgramIngestUnsafe({
          reason:
            'Voice session metadata requires at least one redaction policy ref.',
        })
      }

      const includeRoles = new Set(
        options.includeSpeakerRoles ?? DEFAULT_INCLUDE_SPEAKER_ROLES,
      )

      const admitted: ReadonlyArray<OmniVoiceTranscriptSegment> =
        segments.filter(segment =>
          segmentIsAdmissible(segment, floorBps, includeRoles),
        )

      admitted.forEach(segment => {
        assertSafeRef('Voice segment segmentRef', segment.segmentRef)
        assertSafeRef('Voice segment textRef', segment.textRef)
        ;[...segment.evidenceRefs, ...segment.sourceRefs].forEach(ref =>
          assertSafeRef('Voice segment ref', ref),
        )
      })

      const segmentInputs: ReadonlyArray<VoiceProgramIngestSegmentInput> =
        admitted
          .map(
            segment =>
              new VoiceProgramIngestSegmentInput({
                confidenceBps: segment.confidenceBps,
                evidenceRefs: uniqueSortedRefs(segment.evidenceRefs),
                segmentRef: segment.segmentRef,
                sourceRefs: uniqueSortedRefs(segment.sourceRefs),
                speakerRole: segment.speakerRole,
                textRef: segment.textRef,
              }),
          )
          .sort(compareSegmentInputs)

      const aggregateEvidence = uniqueSortedRefs([
        ...metadata.evidenceRefs,
        ...admitted.flatMap(segment => [...segment.evidenceRefs]),
      ])
      const aggregateSources = uniqueSortedRefs([
        ...metadata.sourceRefs,
        ...admitted.flatMap(segment => [...segment.sourceRefs]),
      ])
      const aggregateRedaction = uniqueSortedRefs([
        ...metadata.redactionPolicyRefs,
        ...admitted.flatMap(segment => [...segment.redactionPolicyRefs]),
      ])

      // Context refs the program run will read: the session-level evidence and
      // source refs serve as the voice-derived context pack / upstream
      // artifact pointers for the stage.
      const contextRefs = uniqueSortedRefs([
        ...aggregateEvidence,
        ...aggregateSources,
      ])

      const meanConfidenceBps =
        segmentInputs.length === 0
          ? null
          : Math.round(
              segmentInputs.reduce(
                (total, input) => total + input.confidenceBps,
                0,
              ) / segmentInputs.length,
            )

      return new VoiceProgramIngestProposal({
        admittedSegmentCount: segmentInputs.length,
        authority: OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
        confidenceFloorBps: floorBps,
        contextRefs,
        evidenceRefs: aggregateEvidence,
        executesProgramRun: false,
        languageRef: metadata.languageRef,
        meanConfidenceBps,
        outputSchemaRef: program.outputSchemaRef,
        programTypeId: deliveryPipelineProgramTypeId(stage),
        proposesProgramInputOnly: true,
        redactionPolicyRefs: aggregateRedaction,
        segmentInputs,
        sessionRef: metadata.sessionRef,
        skippedSegmentCount: segments.length - segmentInputs.length,
        sourceRefs: aggregateSources,
        stage,
      })
    },
  })

/**
 * Convenience example metadata for tests and fixtures (ref-only, public-safe).
 */
export const exampleVoiceProgramIngestSessionMetadata = (
  overrides: Partial<VoiceProgramIngestSessionMetadata> = {},
): VoiceProgramIngestSessionMetadata =>
  new VoiceProgramIngestSessionMetadata({
    authority: OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
    evidenceRefs: ['evidence.public.voice_session_report'],
    languageRef: 'language.public.en',
    redactionPolicyRefs: ['redaction.public.refs_only_transcript'],
    sessionRef: 'session.public.otec_voice_1',
    sourceRefs: ['source.public.browser_audio_summary'],
    workroomRef: 'workroom.public.otec_brand_story',
    ...overrides,
  })

// ---------------------------------------------------------------------------
// COORDINATOR WIRING (integration deferred; do NOT wire in this lane):
//
// 1. Export surface. Add to `workers/api/src/blueprint/index.ts` (or the api
//    barrel the coordinator picks) — do NOT edit shared files in this lane:
//
//      export {
//        buildVoiceProgramIngestProposal,
//        exampleVoiceProgramIngestSessionMetadata,
//        VOICE_INGEST_DEFAULT_CONFIDENCE_FLOOR_BPS,
//        VOICE_INGEST_SUPPORTED_STAGES,
//        VoiceProgramIngestProposal,
//        type VoiceIngestSupportedStage,
//        type VoiceProgramIngestOptions,
//        VoiceProgramIngestSegmentInput,
//        VoiceProgramIngestSessionMetadata,
//        VoiceProgramIngestUnsafe,
//      } from './voice-program-ingest'
//
// 2. Source of segments. A read-only voice session report
//    (`OmniVoiceSessionReportRecord`, via `projectOmniVoiceSessionReport`) is
//    the upstream authority. The coordinator passes
//    `record.transcriptSegments` plus a
//    `VoiceProgramIngestSessionMetadata` built from the record's session-level
//    refs (sessionRef, languageRef, evidenceRefs, sourceRefs,
//    redactionPolicyRefs, workroomRef, authority) into
//    `buildVoiceProgramIngestProposal(...)`.
//
// 3. Program run boundary. The returned `VoiceProgramIngestProposal` is INPUT
//    only: `proposesProgramInputOnly: true`, `executesProgramRun: false`. The
//    coordinator hands `contextRefs` / `evidenceRefs` to a Blueprint program
//    RUN for `programTypeId` (brand-story first; web-copy via the same shape).
//    Producing the brand-story OUTPUT and publishing it remains behind the
//    program run + Action Submission + per-stage `operator_review` release gate
//    declared in `delivery-pipeline-programs.ts`. This core never executes the
//    run and never mutates voice evidence.
//
// 4. OWNER/DESKTOP-GATED FOLLOW-UP (OUT OF SCOPE here):
//      - Live audio capture (browser_microphone / phone_bridge / realtime_api).
//      - The speech-to-text VENDOR that turns audio into transcript segments.
//      - Any live wiring of capture -> transcription -> session report.
//    Those run on the owner/desktop path and produce the
//    `OmniVoiceSessionReportRecord` this core consumes. This module stays pure
//    and vendor-agnostic: it only maps already-transcribed, ref-only segments
//    to a deterministic program-input proposal.
// ---------------------------------------------------------------------------
