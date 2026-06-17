import { describe, expect, test } from 'vitest'

import {
  buildForgeMultimodalInputInput,
  projectForgeMultimodalInput,
} from './multimodal-input'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-18T00:10:00.000Z',
  snapshotRef: 'multimodal-input-snapshot.public.work_1',
  versionRef: 'multimodal-input-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge voice and multimodal input projection', () => {
  test('projects public voice and multimodal evidence as refs-only non-authoritative state', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      entries: [
        {
          attachmentRefs: ['attachment.public.voice_note'],
          captureSurfaceRefs: ['capture-surface.public.browser_audio'],
          consentRefs: ['consent.public.voice_note'],
          contextIngestionRefs: ['context-ingestion.public.voice_note'],
          endpointRefs: ['endpoint.public.vad_summary'],
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.voice_note',
          modality: 'audio',
          policyRefs: ['policy.public.multimodal.consent_required'],
          redactionRefs: ['redaction.public.transcript.safe'],
          state: 'ingested',
          transcriptRefs: ['transcript.public.voice_note.summary'],
          vadRefs: ['vad.public.voice_note.boundary'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      captureReady: 0,
      ingested: 1,
      pending: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      cameraAccessAuthority: false,
      clipboardReadAuthority: false,
      deploymentAuthority: false,
      fileAttachAuthority: false,
      fileReadAuthority: false,
      imageProcessingAuthority: false,
      instructionInjectionAuthority: false,
      mediaCaptureAuthority: false,
      microphoneAccessAuthority: false,
      promptAuthority: false,
      publicClaimAuthority: false,
      screenCaptureAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      transcriptionAuthority: false,
      toolExecutionAuthority: false,
      toolRoutingAuthority: false,
      vadExecutionAuthority: false,
      videoProcessingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing multimodal state as empty', () => {
    const view = projectForgeMultimodalInput({
      generatedAt: '2026-06-18T00:10:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale multimodal evidence', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          inputRef: 'multimodal-input.public.stale',
          modality: 'audio',
          policyRefs: ['policy.public.multimodal.ready'],
          state: 'ingested',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.work_1:stale-multimodal-evidence:multimodal-input.public.stale',
    )
  })

  test('blocks capture-ready state without consent and policy refs', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.no_consent',
          modality: 'audio',
          state: 'capture_ready',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.work_1:capture-consent-policy-missing:multimodal-input.public.no_consent',
    )
  })

  test('blocks transcript refs without redaction refs', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.no_redaction',
          modality: 'audio',
          policyRefs: ['policy.public.multimodal.transcript'],
          state: 'ingested',
          transcriptRefs: ['transcript.public.no_redaction'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.work_1:transcript-redaction-ref-missing:multimodal-input.public.no_redaction',
    )
  })

  test('blocks context ingestion without attachment refs', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      entries: [
        {
          contextIngestionRefs: ['context-ingestion.public.no_attachment'],
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.no_attachment',
          modality: 'image',
          policyRefs: ['policy.public.multimodal.context'],
          state: 'ingested',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.work_1:context-ingestion-attachment-ref-missing:multimodal-input.public.no_attachment',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeMultimodalInput({
      generatedAt: '2026-06-18T00:10:00.000Z',
      entries: [
        {
          consentRefs: ['consent.public.no_snapshot'],
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.no_snapshot',
          modality: 'audio',
          policyRefs: ['policy.public.multimodal.ready'],
          state: 'capture_ready',
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.no_snapshot:missing-multimodal-input-snapshot-ref',
    )
  })

  test('omits unsafe private multimodal material before projection', () => {
    const view = projectForgeMultimodalInput({
      ...baseInput,
      blockerRefs: [
        'multimodal-blocker.public.safe',
        'raw audio /Users/christopher/audio.wav',
      ],
      entries: [
        {
          attachmentRefs: ['attachment.public.safe', 'raw file /Users/christopher/image.png'],
          captureSurfaceRefs: ['capture-surface.public.safe'],
          consentRefs: ['consent.public.safe'],
          contextIngestionRefs: ['context-ingestion.public.safe'],
          endpointRefs: ['endpoint.public.safe'],
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.safe',
          modality: 'audio',
          policyRefs: ['policy.public.safe', 'provider prompt sk-private'],
          redactionRefs: ['redaction.public.safe'],
          state: 'ingested',
          transcriptRefs: [
            'transcript.public.safe',
            'transcript body private prompt',
          ],
          vadRefs: ['vad.public.safe', 'raw vad /Users/christopher/vad.json'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.attachmentRefs).toEqual(['attachment.public.safe'])
    expect(view.entries[0]?.transcriptRefs).toEqual(['transcript.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-multimodal-input-blocker:work.public.work_1:unsafe-multimodal-input-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw audio')
    expect(payload).not.toContain('raw file')
    expect(payload).not.toContain('provider prompt')
    expect(payload).not.toContain('transcript body')
    expect(payload).not.toContain('raw vad')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T00:11:00.000Z',
      multimodalInput: {
        entries: [
          {
            consentRefs: ['consent.public.work_2'],
            freshness: 'fresh',
            inputRef: 'multimodal-input.public.work_2',
            modality: 'audio',
            policyRefs: ['policy.public.work_2'],
            state: 'capture_ready',
          },
        ],
        snapshotRef: 'multimodal-input-snapshot.public.work_2',
        versionRef: 'multimodal-input-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeMultimodalInputInput(work)).toEqual({
      entries: [
        {
          consentRefs: ['consent.public.work_2'],
          freshness: 'fresh',
          inputRef: 'multimodal-input.public.work_2',
          modality: 'audio',
          policyRefs: ['policy.public.work_2'],
          state: 'capture_ready',
        },
      ],
      generatedAt: '2026-06-18T00:11:00.000Z',
      snapshotRef: 'multimodal-input-snapshot.public.work_2',
      versionRef: 'multimodal-input-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
