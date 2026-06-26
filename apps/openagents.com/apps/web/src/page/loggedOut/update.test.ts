import { Effect } from 'effect'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { LandingRoute, StatsRoute, TraceRoute } from '../../route'
import {
  ClickedCopyAgentInstructions,
  ClickedEnterKhala,
  ClickedEnterTassadar,
  ClickedExitKhala,
  CompletedCopyAgentInstructions,
  FailedLoadKhalaTokensServedSnapshot,
  FailedLoadTrace,
  SucceededLoadKhalaTokensServedSnapshot,
  SucceededLoadPublicKhalaTokensServedModelMix,
  SucceededLoadTrace,
  ToggledGymLane,
  UpdatedGymSamplesPerCell,
} from './message'
import { init } from './model'
import {
  LoadPublicKhalaTokensServedHistory,
  LoadPublicKhalaTokensServedModelMix,
  LoadTrace,
  TASSADAR_AGENT_INSTRUCTIONS,
  initialCommands,
  update,
} from './update'
import { sampleTrajectory, SAMPLE_TRACE_UUID } from '../trace/sample'

const model = init(LandingRoute())

const commandNames = (commands: ReadonlyArray<{ readonly name: string }>) =>
  commands.map(command => command.name)

describe('logged-out nav + copy update', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('ClickedEnterKhala navigates to /khala', () => {
    const [, commands] = update(model, ClickedEnterKhala())
    expect(commandNames(commands)).toEqual(['NavigateToKhala'])
  })

  test('ClickedEnterTassadar navigates to /tassadar', () => {
    const [, commands] = update(model, ClickedEnterTassadar())
    expect(commandNames(commands)).toEqual(['NavigateToTassadar'])
  })

  // #6324: the snapshot load (success OR failure) is what unlocks the live
  // tokens-served socket. It must flip `snapshotLoaded`, so the stream opens at
  // the SEEDED cursor instead of racing the snapshot and replaying from 0.
  test('SucceededLoadKhalaTokensServedSnapshot seeds the cursor + unlocks the socket', () => {
    expect(model.khalaTokensServedStream.snapshotLoaded).toBe(false)

    const [next] = update(
      model,
      SucceededLoadKhalaTokensServedSnapshot({
        cursor: 7364,
        summary: {
          observedAt: '2026-06-26T00:00:00.000Z',
          tokensServedTotal: 87_900_000,
        },
      }),
    )

    expect(next.khalaTokensServedStream.snapshotLoaded).toBe(true)
    expect(next.khalaTokensServedStream.cursor).toBe(7364)
    expect(next.publicKhalaTokensServed).toMatchObject({
      _tag: 'PublicKhalaTokensServedLoaded',
      served: { tokensServed: 87_900_000 },
    })
  })

  test('FailedLoadKhalaTokensServedSnapshot still unlocks the socket (scalar fallback seeds)', () => {
    const [next] = update(
      model,
      FailedLoadKhalaTokensServedSnapshot({ error: 'HTTP 503' }),
    )

    expect(next.khalaTokensServedStream.snapshotLoaded).toBe(true)
    // No seeded cursor on failure; self-heals via per-event authoritative totals.
    expect(next.khalaTokensServedStream.cursor).toBe(0)
  })

  test('ClickedExitKhala returns home to / (shared by both info pages)', () => {
    const [, commands] = update(model, ClickedExitKhala())
    expect(commandNames(commands)).toEqual(['NavigateToLanding'])
  })

  test('ClickedCopyAgentInstructions issues a clipboard copy command', () => {
    const [, commands] = update(
      model,
      ClickedCopyAgentInstructions({ text: TASSADAR_AGENT_INSTRUCTIONS }),
    )
    expect(commandNames(commands)).toEqual(['CopyAgentInstructions'])
  })

  test('CompletedCopyAgentInstructions flips the "Copied" affirmation flag', () => {
    expect(model.copiedAgentInstructions).toBe(false)
    const [next, commands] = update(model, CompletedCopyAgentInstructions())
    expect(next.copiedAgentInstructions).toBe(true)
    expect(commands).toEqual([])
  })

  test('Gym lane toggles keep at least one provider lane selected', () => {
    const onlyLane = {
      ...model,
      gym: {
        ...model.gym,
        experiment: {
          ...model.gym.experiment,
          fanout: {
            ...model.gym.experiment.fanout,
            lanes: ['provider-baseline' as const],
          },
        },
      },
    }

    const [next] = update(
      onlyLane,
      ToggledGymLane({ lane: 'provider-baseline' }),
    )

    expect(next.gym.experiment.fanout.lanes).toEqual(['provider-baseline'])
  })

  test('Gym samples per cell clamps public config input', () => {
    const [next] = update(model, UpdatedGymSamplesPerCell({ value: '9000' }))

    expect(next.gym.experiment.samplesPerCell).toBe(25)
  })

  test('the copied agent instructions are grounded in AGENTS.md', () => {
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'POST https://openagents.com/api/agents/register',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain('npx @openagentsinc/pylon')
  })

  test('LoadPublicKhalaTokensServedHistory requests America/Chicago day buckets', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          window: '30d',
          bucket: 'day',
          timezone: 'America/Chicago',
          series: [{ day: '2026-06-24', tokensServed: 14_680_776 }],
        }),
        { headers: { 'content-type': 'application/json' } },
      ),
    )

    const message = await Effect.runPromise(
      LoadPublicKhalaTokensServedHistory().effect,
    )

    expect(message._tag).toBe('SucceededLoadPublicKhalaTokensServedHistory')
    const [requestUrl, requestInit] = fetchSpy.mock.calls[0]!
    expect(requestUrl).toBe(
      '/api/public/khala-tokens-served/history?bucket=day&timezone=America%2FChicago&window=30d',
    )
    expect(requestInit).toEqual({
      cache: 'no-store',
      headers: { accept: 'application/json' },
    })
  })

  test('Stats route loads public Khala aggregate endpoints', () => {
    const statsModel = init(StatsRoute())

    expect(commandNames(initialCommands(statsModel))).toEqual([
      'LoadPublicPylonStats',
      'LoadKhalaTokensServedSnapshot',
      'LoadPublicKhalaTokensServed',
      'LoadPublicKhalaTokensServedHistory',
      'LoadPublicKhalaTokensServedModelMix',
      'LoadPublicForumLaunchStatus',
      'LoadPublicForumTipLeaderboards',
      'LoadSettledFeedSnapshot',
    ])
  })

  test('LoadPublicKhalaTokensServedModelMix reads canonical aggregate family mix', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          window: '30d',
          totalTokensServed: 14_680_776,
          generatedAt: '2026-06-24T12:00:00.000Z',
          families: [
            {
              family: 'openai',
              tokensServed: 10_000_000,
              usageEvents: 9,
              share: 0.6812,
            },
            {
              family: 'pylon_codex',
              tokensServed: 4_680_776,
              usageEvents: 3,
              share: 0.3188,
            },
          ],
        }),
        { headers: { 'content-type': 'application/json' } },
      ),
    )

    const message = await Effect.runPromise(
      LoadPublicKhalaTokensServedModelMix().effect,
    )

    expect(message._tag).toBe('SucceededLoadPublicKhalaTokensServedModelMix')
    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/public/khala-tokens-served/model-mix?window=30d',
      {
        cache: 'no-store',
        headers: { accept: 'application/json' },
      },
    )
    if (message._tag === 'SucceededLoadPublicKhalaTokensServedModelMix') {
      expect(message.mix.families.map(row => row.family)).toEqual([
        'openai',
        'pylon_codex',
      ])
    }
  })

  test('SucceededLoadPublicKhalaTokensServedModelMix stores aggregate family rows', () => {
    const [next] = update(
      init(StatsRoute()),
      SucceededLoadPublicKhalaTokensServedModelMix({
        mix: {
          window: '30d',
          totalTokensServed: 14_680_776,
          generatedAt: '2026-06-24T12:00:00.000Z',
          families: [
            {
              family: 'openai',
              tokensServed: 14_680_776,
              usageEvents: 12,
              share: 1,
            },
          ],
        },
      }),
    )

    expect(next.publicKhalaTokensServedModelMix).toMatchObject({
      _tag: 'PublicKhalaTokensServedModelMixLoaded',
      mix: {
        totalTokensServed: 14_680_776,
        families: [{ family: 'openai' }],
      },
    })
  })
})

// Live `/trace/{uuid}` read wiring (issue #6209): on entering a real Trace route
// the page enters the loading state and fetches the read API; the committed
// sample uuid stays a clean local fallback with no network round-trip.
const REAL_TRACE_UUID = '24c6fea6-b271-46c6-a9a9-bc614440e9ef'

const traceApiResponse = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    ...init,
  })

describe('trace route load wiring', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('a real Trace uuid enters loading and dispatches LoadTrace', () => {
    const traceModel = init(TraceRoute({ uuid: REAL_TRACE_UUID }))
    expect(traceModel.trace).toEqual({
      _tag: 'TraceLoading',
      uuid: REAL_TRACE_UUID,
    })
    const commands = initialCommands(traceModel)
    expect(commandNames(commands)).toEqual(['LoadTrace'])
  })

  test('the committed sample uuid stays Idle with no network fetch', () => {
    const traceModel = init(TraceRoute({ uuid: SAMPLE_TRACE_UUID }))
    expect(traceModel.trace._tag).toBe('TraceIdle')
    expect(initialCommands(traceModel)).toEqual([])
  })

  test('LoadTrace decodes the { trace: { trajectory } } shape on 200', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      traceApiResponse({
        trace: {
          uuid: REAL_TRACE_UUID,
          visibility: 'public',
          stepCount: sampleTrajectory.steps.length,
          trajectory: JSON.parse(JSON.stringify(sampleTrajectory)),
        },
      }),
    )

    const message = await Effect.runPromise(
      LoadTrace({ uuid: REAL_TRACE_UUID }).effect,
    )

    expect(message._tag).toBe('SucceededLoadTrace')
    expect(globalThis.fetch).toHaveBeenCalledWith(
      `/api/traces/${REAL_TRACE_UUID}`,
      expect.objectContaining({ credentials: 'include' }),
    )
    if (message._tag === 'SucceededLoadTrace') {
      expect(message.uuid).toBe(REAL_TRACE_UUID)
      expect(message.trajectory.steps.length).toBe(sampleTrajectory.steps.length)
    }
  })

  test('LoadTrace surfaces a 404 with status 404', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      traceApiResponse({ error: 'trace_not_found' }, { status: 404 }),
    )

    const message = await Effect.runPromise(
      LoadTrace({ uuid: REAL_TRACE_UUID }).effect,
    )

    expect(message._tag).toBe('FailedLoadTrace')
    if (message._tag === 'FailedLoadTrace') {
      expect(message.status).toBe(404)
    }
  })

  test('SucceededLoadTrace loads the trajectory into the model for the active uuid', () => {
    const loading = init(TraceRoute({ uuid: REAL_TRACE_UUID }))
    const [next] = update(
      loading,
      SucceededLoadTrace({
        uuid: REAL_TRACE_UUID,
        trajectory: sampleTrajectory,
      }),
    )
    expect(next.trace._tag).toBe('TraceLoaded')
    if (next.trace._tag === 'TraceLoaded') {
      expect(next.trace.uuid).toBe(REAL_TRACE_UUID)
    }
  })

  test('a stale SucceededLoadTrace for a different uuid is ignored', () => {
    const loading = init(TraceRoute({ uuid: REAL_TRACE_UUID }))
    const [next] = update(
      loading,
      SucceededLoadTrace({
        uuid: 'some-other-uuid',
        trajectory: sampleTrajectory,
      }),
    )
    expect(next.trace).toEqual(loading.trace)
  })

  test('FailedLoadTrace with status 404 yields the not-found state', () => {
    const loading = init(TraceRoute({ uuid: REAL_TRACE_UUID }))
    const [next] = update(
      loading,
      FailedLoadTrace({
        uuid: REAL_TRACE_UUID,
        error: 'Trace returned HTTP 404.',
        status: 404,
      }),
    )
    expect(next.trace._tag).toBe('TraceNotFound')
  })

  test('FailedLoadTrace with a non-404 status yields the failed state', () => {
    const loading = init(TraceRoute({ uuid: REAL_TRACE_UUID }))
    const [next] = update(
      loading,
      FailedLoadTrace({
        uuid: REAL_TRACE_UUID,
        error: 'network down',
        status: 0,
      }),
    )
    expect(next.trace._tag).toBe('TraceFailed')
  })
})
