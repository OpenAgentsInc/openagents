import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  deliveryPipelineProgramForStage,
  deliveryPipelineProgramTypeId,
} from './blueprint/delivery-pipeline-programs'
import {
  OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
  exampleOmniVoiceTranscriptSegment,
} from './omni-voice-session-evidence'
import {
  buildVoiceProgramIngestProposal,
  exampleVoiceProgramIngestSessionMetadata,
  VoiceProgramIngestSessionMetadata,
  VoiceProgramIngestUnsafe,
} from './voice-program-ingest'

const run = <A>(
  effect: Effect.Effect<A, VoiceProgramIngestUnsafe>,
): Promise<A> => Effect.runPromise(effect)

const runExit = <A>(effect: Effect.Effect<A, VoiceProgramIngestUnsafe>) =>
  Effect.runPromiseExit(effect)

describe('buildVoiceProgramIngestProposal', () => {
  test('maps user segments to a brand-story program input shape', async () => {
    const segments = [
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
    ]

    const proposal = await run(
      buildVoiceProgramIngestProposal(
        segments,
        exampleVoiceProgramIngestSessionMetadata(),
      ),
    )

    expect(proposal.stage).toBe('brand-story')
    expect(proposal.programTypeId).toBe(
      deliveryPipelineProgramTypeId('brand-story'),
    )
    expect(proposal.outputSchemaRef).toBe(
      deliveryPipelineProgramForStage('brand-story')?.outputSchemaRef,
    )
    expect(proposal.proposesProgramInputOnly).toBe(true)
    expect(proposal.executesProgramRun).toBe(false)
    expect(proposal.admittedSegmentCount).toBe(2)
    expect(proposal.skippedSegmentCount).toBe(0)
    expect(proposal.segmentInputs.map(input => input.segmentRef)).toEqual([
      'segment.public.voice_001',
      'segment.public.voice_002',
    ])
    expect(proposal.meanConfidenceBps).toBe(9000)
  })

  test('respects read-only authority (echoes the read-only authority and does not execute)', async () => {
    const proposal = await run(
      buildVoiceProgramIngestProposal(
        [exampleOmniVoiceTranscriptSegment()],
        exampleVoiceProgramIngestSessionMetadata(),
      ),
    )

    expect(proposal.authority).toEqual(
      OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
    )
    expect(proposal.executesProgramRun).toBe(false)
    expect(proposal.proposesProgramInputOnly).toBe(true)
  })

  test('rejects metadata whose authority is not read-only', async () => {
    const metadata = new VoiceProgramIngestSessionMetadata({
      ...exampleVoiceProgramIngestSessionMetadata(),
      authority: {
        ...OMNI_VOICE_SESSION_READ_ONLY_AUTHORITY,
        noTranscriptMutation: false,
      },
    })

    const exit = await runExit(
      buildVoiceProgramIngestProposal(
        [exampleOmniVoiceTranscriptSegment()],
        metadata,
      ),
    )

    expect(exit._tag).toBe('Failure')
  })

  test('preserves and aggregates evidence + source refs from segments and metadata', async () => {
    const segments = [
      exampleOmniVoiceTranscriptSegment({
        evidenceRefs: ['evidence.public.seg_a', 'evidence.public.seg_shared'],
        segmentRef: 'segment.public.a',
        sourceRefs: ['source.public.seg_a'],
        speakerRole: 'user',
        textRef: 'text.public.a',
      }),
    ]

    const proposal = await run(
      buildVoiceProgramIngestProposal(
        segments,
        exampleVoiceProgramIngestSessionMetadata({
          evidenceRefs: [
            'evidence.public.session',
            'evidence.public.seg_shared',
          ],
          sourceRefs: ['source.public.session'],
        }),
      ),
    )

    // Segment evidence refs preserved.
    expect(proposal.evidenceRefs).toContain('evidence.public.seg_a')
    // Session evidence refs preserved.
    expect(proposal.evidenceRefs).toContain('evidence.public.session')
    // Deduplicated.
    expect(
      proposal.evidenceRefs.filter(
        ref => ref === 'evidence.public.seg_shared',
      ),
    ).toHaveLength(1)
    // Sorted.
    expect([...proposal.evidenceRefs]).toEqual(
      [...proposal.evidenceRefs].sort(),
    )
    expect(proposal.sourceRefs).toContain('source.public.seg_a')
    expect(proposal.sourceRefs).toContain('source.public.session')
    // contextRefs union of evidence + source.
    expect(proposal.contextRefs).toContain('evidence.public.seg_a')
    expect(proposal.contextRefs).toContain('source.public.session')
  })

  test('skips low-confidence and non-content segments deterministically', async () => {
    const segments = [
      exampleOmniVoiceTranscriptSegment({
        confidenceBps: 9000,
        segmentRef: 'segment.public.high',
        speakerRole: 'user',
        textRef: 'text.public.high',
      }),
      // Below the default 6000 bps floor.
      exampleOmniVoiceTranscriptSegment({
        confidenceBps: 2000,
        segmentRef: 'segment.public.low',
        speakerRole: 'user',
        textRef: 'text.public.low',
      }),
      // Agent echo role is excluded by default.
      exampleOmniVoiceTranscriptSegment({
        confidenceBps: 9500,
        segmentRef: 'segment.public.agent',
        speakerRole: 'agent',
        textRef: 'text.public.agent',
      }),
    ]

    const proposal = await run(
      buildVoiceProgramIngestProposal(
        segments,
        exampleVoiceProgramIngestSessionMetadata(),
      ),
    )

    expect(proposal.admittedSegmentCount).toBe(1)
    expect(proposal.skippedSegmentCount).toBe(2)
    expect(proposal.segmentInputs.map(input => input.segmentRef)).toEqual([
      'segment.public.high',
    ])
  })

  test('handles the empty / all-skipped case with a null mean confidence', async () => {
    const proposal = await run(
      buildVoiceProgramIngestProposal(
        [],
        exampleVoiceProgramIngestSessionMetadata(),
      ),
    )

    expect(proposal.admittedSegmentCount).toBe(0)
    expect(proposal.skippedSegmentCount).toBe(0)
    expect(proposal.meanConfidenceBps).toBeNull()
    expect(proposal.segmentInputs).toHaveLength(0)
    // Session-level refs are still preserved as context.
    expect(proposal.evidenceRefs.length).toBeGreaterThan(0)
    expect(proposal.sourceRefs.length).toBeGreaterThan(0)
  })

  test('is deterministic: same inputs produce identical output regardless of segment order', async () => {
    const segmentA = exampleOmniVoiceTranscriptSegment({
      segmentRef: 'segment.public.zeta',
      speakerRole: 'user',
      textRef: 'text.public.zeta',
    })
    const segmentB = exampleOmniVoiceTranscriptSegment({
      segmentRef: 'segment.public.alpha',
      speakerRole: 'user',
      textRef: 'text.public.alpha',
    })
    const metadata = exampleVoiceProgramIngestSessionMetadata()

    const forward = await run(
      buildVoiceProgramIngestProposal([segmentA, segmentB], metadata),
    )
    const reversed = await run(
      buildVoiceProgramIngestProposal([segmentB, segmentA], metadata),
    )

    expect(JSON.stringify(forward)).toBe(JSON.stringify(reversed))
    expect(forward.segmentInputs.map(input => input.segmentRef)).toEqual([
      'segment.public.alpha',
      'segment.public.zeta',
    ])
  })

  test('supports the web-copy stage hook', async () => {
    const proposal = await run(
      buildVoiceProgramIngestProposal(
        [
          exampleOmniVoiceTranscriptSegment({
            speakerRole: 'user',
            textRef: 'text.public.web_copy_request',
          }),
        ],
        exampleVoiceProgramIngestSessionMetadata(),
        { stage: 'web-copy' },
      ),
    )

    expect(proposal.stage).toBe('web-copy')
    expect(proposal.programTypeId).toBe(
      deliveryPipelineProgramTypeId('web-copy'),
    )
    expect(proposal.outputSchemaRef).toBe(
      deliveryPipelineProgramForStage('web-copy')?.outputSchemaRef,
    )
  })
})
