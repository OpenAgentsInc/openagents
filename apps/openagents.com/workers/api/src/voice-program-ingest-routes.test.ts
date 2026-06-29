import { describe, expect, test } from 'vitest'

import {
  OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
  exampleOmniVoiceTranscriptSegment,
} from './omni-voice-session-evidence'
import { exampleVoiceProgramIngestSessionMetadata } from './voice-program-ingest'
import {
  VOICE_PROGRAM_INGEST_BLOCKERS,
  VoiceProgramIngestEndpoint,
  handleVoiceProgramIngestApi,
  isVoiceProgramIngestEnabled,
} from './voice-program-ingest-routes'

const ingestUrl = `https://example.com${VoiceProgramIngestEndpoint}`

const postRequest = (body: unknown): Request =>
  new Request(ingestUrl, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const exampleBody = (overrides: Record<string, unknown> = {}) => ({
  metadata: exampleVoiceProgramIngestSessionMetadata(),
  segments: [
    exampleOmniVoiceTranscriptSegment({
      confidenceBps: 9200,
      evidenceRefs: ['evidence.public.voice_segment_1_transcript'],
      segmentRef: 'segment.public.voice_001',
      sourceRefs: ['source.public.browser_audio_summary'],
      speakerRole: 'user',
      textRef: 'text.public.brand_origin_summary',
    }),
    exampleOmniVoiceTranscriptSegment({
      confidenceBps: 8800,
      evidenceRefs: ['evidence.public.voice_segment_2_transcript'],
      segmentRef: 'segment.public.voice_002',
      sourceRefs: ['source.public.browser_audio_summary'],
      speakerRole: 'operator',
      startMillis: 11_000,
      textRef: 'text.public.brand_values_summary',
    }),
  ],
  ...overrides,
})

describe('isVoiceProgramIngestEnabled', () => {
  test('defaults OFF and only arms on explicit truthy tokens', () => {
    expect(isVoiceProgramIngestEnabled(undefined)).toBe(false)
    expect(isVoiceProgramIngestEnabled('')).toBe(false)
    expect(isVoiceProgramIngestEnabled('false')).toBe(false)
    expect(isVoiceProgramIngestEnabled('0')).toBe(false)
    expect(isVoiceProgramIngestEnabled('off')).toBe(false)
    expect(isVoiceProgramIngestEnabled('true')).toBe(true)
    expect(isVoiceProgramIngestEnabled('1')).toBe(true)
    expect(isVoiceProgramIngestEnabled('ON')).toBe(true)
    expect(isVoiceProgramIngestEnabled(' yes ')).toBe(true)
  })
})

describe('handleVoiceProgramIngestApi', () => {
  test('rejects non-POST with 405', async () => {
    const response = await handleVoiceProgramIngestApi(
      new Request(ingestUrl, { method: 'GET' }),
      { enabled: true },
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('POST')
  })

  test('INERT by default: disabled returns an honest red payload and never runs the core', async () => {
    const response = await handleVoiceProgramIngestApi(
      postRequest(exampleBody()),
      { enabled: false },
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')

    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.inert).toBe(true)
    expect(payload.enabled).toBe(false)
    expect(payload.promiseState).toBe('red')
    expect(payload.promiseId).toBe(
      'mobile.voice_session_evidence_transcript_ingest.v1',
    )
    expect(payload.executesProgramRun).toBe(false)
    expect(payload.callsTranscriptionService).toBe(false)
    expect(payload.capturesAudio).toBe(false)
    // The named blocker this endpoint clears, and the two that stay gated.
    expect(payload.blockerCleared).toBe(VOICE_PROGRAM_INGEST_BLOCKERS.cleared)
    expect(payload.blockerCleared).toBe(
      'blocker.product_promises.voice_ingestion_endpoint_missing',
    )
    expect(payload.remainingBlockers).toEqual([
      'blocker.product_promises.voice_transcription_service_missing',
      'blocker.product_promises.voice_proposal_and_approval_ui_missing',
    ])
    // No proposal is produced while inert.
    expect(payload.proposal).toBeUndefined()
  })

  test('ARMED: decodes ref-only segments and returns the approval-gated program-input proposal', async () => {
    const response = await handleVoiceProgramIngestApi(
      postRequest(exampleBody()),
      { enabled: true },
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')

    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.promiseState).toBe('red')
    expect(payload.enabled).toBe(true)
    // The envelope echoes that nothing executes; only program INPUT is proposed.
    expect(payload.executesProgramRun).toBe(false)
    expect(payload.proposesProgramInputOnly).toBe(true)
    // The blocker this clears stays surfaced; settlement-adjacent blockers stay.
    expect(payload.blockerCleared).toBe(
      'blocker.product_promises.voice_ingestion_endpoint_missing',
    )

    const proposal = payload.proposal as Record<string, unknown>
    expect(proposal).toBeDefined()
    expect(proposal.stage).toBe('brand-story')
    expect(proposal.proposesProgramInputOnly).toBe(true)
    expect(proposal.executesProgramRun).toBe(false)
    expect(proposal.admittedSegmentCount).toBe(2)
    expect(proposal.skippedSegmentCount).toBe(0)
    expect(proposal.meanConfidenceBps).toBe(9000)
    expect(
      (proposal.segmentInputs as ReadonlyArray<{ segmentRef: string }>).map(
        input => input.segmentRef,
      ),
    ).toEqual(['segment.public.voice_001', 'segment.public.voice_002'])

    // Read-only authority is echoed: the proposal asserts no mutation/execution.
    const authority = proposal.authority as Record<string, unknown>
    expect(authority.authorityBoundary).toBe(
      OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY.authorityBoundary,
    )
    expect(authority.noCommandExecution).toBe(true)
    expect(authority.noTranscriptMutation).toBe(true)
    expect(authority.noPaymentMutation).toBe(true)
    expect(authority.noAudioCaptureMutation).toBe(true)
  })

  test('ARMED: a malformed body is safely rejected with 400 (no leak)', async () => {
    const response = await handleVoiceProgramIngestApi(
      postRequest({ metadata: { not: 'valid' }, segments: 'nope' }),
      { enabled: true },
    )
    expect(response.status).toBe(400)
    const payload = (await response.json()) as Record<string, unknown>
    // Generic safe message, no transcript content or internal detail.
    expect(JSON.stringify(payload)).not.toContain('nope')
  })

  test('ARMED: a non-read-only authority is rejected by the core as unsafe (422)', async () => {
    const unsafeMetadata = {
      ...exampleVoiceProgramIngestSessionMetadata(),
      authority: {
        ...OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
        // Flip a single read-only guard off: the core must refuse.
        noCommandExecution: false,
      },
    }
    const response = await handleVoiceProgramIngestApi(
      postRequest({ ...exampleBody(), metadata: unsafeMetadata }),
      { enabled: true },
    )
    expect(response.status).toBe(422)
    const payload = (await response.json()) as Record<string, unknown>
    expect(payload.error).toBe('voice_program_ingest_unsafe')
    expect(typeof payload.reason).toBe('string')
  })
})
