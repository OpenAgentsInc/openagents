import { describe, expect, test } from 'vitest'

const smoke = await import('./visibility-browser-smoke.mjs')

const sourceRef = { kind: 'api', ref: 'route:/api/public/proof' }

const sourcedReplayBundle = () => ({
  actors: [{ actorRef: 'actor.worker' }],
  cameraCues: [
    {
      cueRef: 'cue.follow_worker',
      focusRefs: ['actor.worker'],
      sourceRefs: ['route:/api/public/proof'],
    },
  ],
  captions: [
    {
      captionRef: 'caption.worker_verified',
      sourceRefs: ['route:/api/public/proof'],
    },
  ],
  events: [
    {
      actorRefs: ['actor.worker'],
      eventRef: 'event.worker_verified',
      sourceRefs: ['route:/api/public/proof'],
      targetRefs: ['stage.proof'],
    },
  ],
  flows: [
    {
      flowRef: 'flow.worker_to_stage',
      fromRef: 'actor.worker',
      sourceRefs: ['route:/api/public/proof'],
      toRef: 'stage.proof',
    },
  ],
  gaps: [
    {
      gapRef: 'gap.historical',
      sourceRefs: ['route:/api/public/proof'],
    },
  ],
  sourceRefs: [sourceRef],
  stages: [
    {
      sourceRefs: ['route:/api/public/proof'],
      stageRef: 'stage.proof',
    },
  ],
})

describe('visibility browser smoke helpers', () => {
  test('parses browser smoke options', () => {
    expect(
      smoke.parseArgs([
        '--base-url',
        'http://localhost:5173',
        '--api-base-url',
        'https://openagents.com',
        '--timeout-ms',
        '12000',
        '--proof-limit',
        '2',
        '--headed',
      ]),
    ).toMatchObject({
      apiBaseUrl: 'https://openagents.com',
      baseUrl: 'http://localhost:5173',
      headless: false,
      proofLimit: 2,
      timeoutMs: 12000,
    })
  })

  test('requires positive timeout and proof link limits', () => {
    expect(() => smoke.parseArgs(['--timeout-ms', '999'])).toThrow(
      '--timeout-ms must be a number >= 1000.',
    )
    expect(() => smoke.parseArgs(['--proof-limit', '0'])).toThrow(
      '--proof-limit must be a number >= 1.',
    )
  })

  test('accepts replay motion only when moving records have source refs', () => {
    expect(smoke.motionSourceRefGaps(sourcedReplayBundle())).toEqual([])

    const bundle = sourcedReplayBundle()
    bundle.events = [
      {
        actorRefs: ['actor.worker'],
        eventRef: 'event.worker_verified',
        sourceRefs: [],
        targetRefs: ['stage.proof'],
      },
    ]
    bundle.flows = []
    bundle.actors = [{ actorRef: 'actor.worker' }, { actorRef: 'actor.orphan' }]

    expect(smoke.motionSourceRefGaps(bundle)).toEqual([
      'events.event.worker_verified',
      'actors.actor.orphan',
    ])
  })

  test('classifies useful canvas probes as nonblank', () => {
    expect(
      smoke.canvasProbePassed({
        distinctColorCount: 7,
        nonBlankPixels: 40,
        nonTransparentPixels: 600,
        nonUniformPixels: 80,
        samplePixels: 1000,
      }),
    ).toBe(true)

    expect(
      smoke.canvasProbePassed({
        distinctColorCount: 1,
        nonBlankPixels: 0,
        nonTransparentPixels: 1000,
        nonUniformPixels: 0,
        samplePixels: 1000,
      }),
    ).toBe(false)
  })
})
