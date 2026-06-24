// Full server-side onboarding resume handshake — the SERVER half of the
// client↔server reload-resume contract, wired through the REAL routes, a REAL
// in-process durable substrate (the `@openagentsinc/durable-stream`
// `MemoryStreamStore` behind `handleRequest`), a SQLite-backed D1 session
// store, and a SCRIPTED multi-delta inference source.
//
// This regression-covers the live "refresh loses the whole conversation" bug.
// The root cause is a server-side TIMING fact the client must tolerate: the
// session ROW is only written when a turn FINALIZES (after the stream drains).
// So a refresh that lands MID-FIRST-TURN sees:
//   - GET /api/autopilot/onboarding/{sessionId}  => 404 (row not written yet)
//   - GET .../turn/{turnIndex}/stream?offset=... => 200 (durable log replays)
// The client previously treated that 404 as "stale, clear everything" and wiped
// the user's real, resumable conversation. This test pins the server behaviour
// (404-before-finalize, durable-replay-while-in-flight, 200-after-finalize) so
// the client contract it feeds (see the web `resume-handshake.test.ts`) stays
// honest.

import { DatabaseSync } from 'node:sqlite'

import {
  MemoryStreamStore,
  type StreamStore,
  handleRequest,
  streamIdFromUrl,
} from '@openagentsinc/durable-stream'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OnboardingInferenceClient,
  type OnboardingStreamClient,
  type OnboardingStreamSource,
} from './autopilot-onboarding-program'
import { makeAutopilotOnboardingRoutes } from './autopilot-onboarding-routes'
import { type DurableStreamNamespace } from './inference/durable-inference-do-transport'

// MINIMAL D1 OVER node:sqlite (mirrors the routes test harness) --------------

type Row = Record<string, unknown>
class Stmt {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): Stmt {
    this.bound = values.map(v => (v === undefined ? null : v))
    return this
  }
  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async run(): Promise<{ success: true; results: [] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}
class D1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): Stmt {
    return new Stmt(this.db, sql)
  }
}

const SCHEMA = `
CREATE TABLE autopilot_onboarding_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  vertical_overlay TEXT,
  status TEXT NOT NULL DEFAULT 'interviewing'
    CHECK (status IN ('interviewing', 'complete')),
  transcript_json TEXT NOT NULL DEFAULT '[]',
  output_spec_json TEXT NOT NULL DEFAULT '{}',
  turn_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

type Env = { OPENAGENTS_DB: D1Database }

const makeEnv = (): Env => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return { OPENAGENTS_DB: new D1(raw) as unknown as D1Database }
}

const run = (effect: Effect.Effect<Response> | undefined): Promise<Response> => {
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

const echoInference: OnboardingInferenceClient = request =>
  Effect.succeed(`assistant-reply (saw ${request.messages.length} messages)`)

// REAL in-process DO backend: one MemoryStreamStore per stream id.
class StreamRegistry {
  private readonly stores = new Map<string, StreamStore>()
  private storeFor(streamId: string): StreamStore {
    let s = this.stores.get(streamId)
    if (s === undefined) {
      s = new MemoryStreamStore()
      this.stores.set(streamId, s)
    }
    return s
  }
  fetch(request: Request): Promise<Response> {
    const streamId = streamIdFromUrl(request.url)
    if (streamId === null) {
      return Promise.resolve(new Response('nope', { status: 404 }))
    }
    return handleRequest(this.storeFor(streamId), request, { streamId })
  }
}

const durableNamespace = (registry: StreamRegistry): DurableStreamNamespace => ({
  getByName: () => ({ fetch: (r: Request) => registry.fetch(r) }),
})

// A SCRIPTED stream source that emits MULTIPLE deltas and a controllable
// completion gate. The producer drain runs after the request Effect resolves;
// `release()` lets the test hold the turn "mid-stream" so it can probe the
// not-yet-finalized server state before the durable tee + finalize complete.
const gatedStream = (
  chunks: ReadonlyArray<string>,
): { client: OnboardingStreamClient; release: () => void; done: Promise<void> } => {
  let releaseGate: () => void = () => {}
  const gate = new Promise<void>(resolve => {
    releaseGate = resolve
  })
  let resolveDone: () => void = () => {}
  const done = new Promise<void>(resolve => {
    resolveDone = resolve
  })
  const client: OnboardingStreamClient = () =>
    Effect.succeed<OnboardingStreamSource>({
      deltas: (async function* () {
        for (let i = 0; i < chunks.length; i += 1) {
          // Hold AFTER the first delta until the test releases, so the turn is
          // genuinely mid-stream (durable log has bytes, session row absent).
          if (i === 1) {
            await gate
          }
          yield chunks[i]!
        }
        resolveDone()
      })(),
      final: () => chunks.join(''),
    })
  return { client, release: () => releaseGate(), done }
}

const streamRequest = (sessionId: string, body: unknown): Request =>
  new Request(
    `https://openagents.com/api/autopilot/onboarding/${sessionId}/turn`,
    {
      body: JSON.stringify(body),
      method: 'POST',
      headers: { accept: 'text/event-stream' },
    },
  )

const getSession = (
  routes: ReturnType<typeof makeAutopilotOnboardingRoutes<Env>>,
  env: Env,
  sessionId: string,
) =>
  run(
    routes.routeOnboardingTurnRequest(
      new Request(`https://openagents.com/api/autopilot/onboarding/${sessionId}`, {
        method: 'GET',
      }),
      env,
    ),
  )

const resumeRead = (
  routes: ReturnType<typeof makeAutopilotOnboardingRoutes<Env>>,
  env: Env,
  sessionId: string,
  turnIndex: number,
  offset: string,
) =>
  run(
    routes.routeOnboardingTurnRequest(
      new Request(
        `https://openagents.com/api/autopilot/onboarding/${sessionId}/turn/${turnIndex}/stream?offset=${offset}`,
        { method: 'GET' },
      ),
      env,
    ),
  )

describe('onboarding resume handshake — server contract for reload-resume', () => {
  test('GET session is 404 MID-FIRST-TURN, but the durable read replays the in-flight turn (the bug condition)', async () => {
    const registry = new StreamRegistry()
    const env = makeEnv()
    const gated = gatedStream(['AAA', 'BBB', 'CCC'])
    const routes = makeAutopilotOnboardingRoutes<Env>({
      makeInferenceClient: () => echoInference,
      makeStreamClient: () => gated.client,
      resolveDurableStream: () => durableNamespace(registry),
      nowIso: () => '2026-06-23T00:00:00.000Z',
    })

    // Open the first turn's stream. The handshake + first delta flush; the
    // source then holds (mid-stream) until we release.
    const stream = await run(
      routes.routeOnboardingTurnRequest(
        streamRequest('sess-mid', { userText: 'I run a bakery.' }),
        env,
      ),
    )
    expect(stream.status).toBe(200)
    // The durable stream id + resume url the client persists from the handshake.
    expect(stream.headers.get('openagents-onboarding-stream-id')).toBe(
      'onboarding:sess-mid:0',
    )

    // Begin draining so the producer tee writes the handshake + first delta into
    // the durable log, then pauses at the gate.
    const body = stream.body
    if (body === null) throw new Error('no stream body')
    const reader = body.getReader()
    // Pull enough to ensure the handshake + first delta have been teed.
    await reader.read()
    await reader.read()
    // Give the async tee a tick to flush the held bytes to the durable store.
    await new Promise(resolve => setTimeout(resolve, 10))

    // MID-FIRST-TURN REFRESH: the session ROW is not written yet (finalize has
    // not run), so GET session is a 404 — exactly what made the client wipe the
    // conversation.
    const midSession = await getSession(routes, env, 'sess-mid')
    expect(midSession.status).toBe(404)

    // ...yet the durable log already holds the in-flight turn and replays it,
    // so the conversation is genuinely RESUMABLE despite the 404. (The client
    // must therefore NOT clear on this 404.)
    const midResume = await resumeRead(routes, env, 'sess-mid', 0, '0')
    expect(midResume.status).toBe(200)
    const midResumeBody = await midResume.text()
    expect(midResumeBody).toContain('event: delta\ndata: {"text":"AAA"}')

    // Release the held stream and drain to completion (finalize runs: append +
    // persist the session row).
    gated.release()
    for (;;) {
      const { done } = await reader.read()
      if (done) break
    }
    await gated.done
    // Let the post-drain finalize (Effect.runPromise) settle.
    await new Promise(resolve => setTimeout(resolve, 10))

    // AFTER FINALIZE: the session row exists; the reconcile GET now returns the
    // authoritative transcript.
    const doneSession = await getSession(routes, env, 'sess-mid')
    expect(doneSession.status).toBe(200)
    const json = (await doneSession.json()) as {
      sessionId: string
      turnCount: number
      transcript: ReadonlyArray<{ role: string; content: string }>
    }
    expect(json.sessionId).toBe('sess-mid')
    expect(json.turnCount).toBe(1)
    expect(json.transcript.some(t => t.role === 'user')).toBe(true)
    expect(json.transcript.some(t => t.role === 'assistant')).toBe(true)

    // The completed durable log replays the full turn (delta..done) on resume.
    const finalResume = await resumeRead(routes, env, 'sess-mid', 0, '0')
    expect(finalResume.status).toBe(200)
    expect(finalResume.headers.get('stream-closed')).toBe('true')
    const finalBody = await finalResume.text()
    expect(finalBody).toContain('event: delta\ndata: {"text":"AAA"}')
    expect(finalBody).toContain('event: delta\ndata: {"text":"CCC"}')
    expect(finalBody).toContain('event: done')
  })
})
