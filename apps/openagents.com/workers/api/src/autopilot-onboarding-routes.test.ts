import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OnboardingInferenceClient,
  OnboardingInferenceError,
} from './autopilot-onboarding-program'
import { makeAutopilotOnboardingRoutes } from './autopilot-onboarding-routes'

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

const turnRequest = (sessionId: string, body: unknown): Request =>
  new Request(
    `https://openagents.com/api/autopilot/onboarding/${sessionId}/turn`,
    { body: JSON.stringify(body), method: 'POST' },
  )

const echoInference: OnboardingInferenceClient = request =>
  Effect.succeed(
    `assistant-reply (saw ${request.messages.length} messages)`,
  )

const failingInference: OnboardingInferenceClient = () =>
  new OnboardingInferenceError({ reason: 'no provider lane configured' })

const run = (effect: Effect.Effect<Response> | undefined): Promise<Response> => {
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

const routesWith = (infer: OnboardingInferenceClient) =>
  makeAutopilotOnboardingRoutes<Env>({
    makeInferenceClient: () => infer,
    nowIso: () => '2026-06-23T00:00:00.000Z',
  })

describe('POST /api/autopilot/onboarding/{sessionId}/turn', () => {
  test('does not match an unrelated path', () => {
    const routes = routesWith(echoInference)
    const result = routes.routeOnboardingTurnRequest(
      new Request('https://openagents.com/api/autopilot/goals', {
        method: 'POST',
      }),
      makeEnv(),
    )
    expect(result).toBeUndefined()
  })

  test('rejects non-POST with 405', async () => {
    const routes = routesWith(echoInference)
    const response = await run(
      routes.routeOnboardingTurnRequest(
        new Request(
          'https://openagents.com/api/autopilot/onboarding/sess-1/turn',
          { method: 'GET' },
        ),
        makeEnv(),
      ),
    )
    expect(response.status).toBe(405)
  })

  test('advances a turn and returns the assistant reply + spec', async () => {
    const routes = routesWith(echoInference)
    const response = await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-1', { userText: 'I run a bakery.' }),
        makeEnv(),
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      sessionId: string
      reply: string
      status: string
      turnCount: number
      outputSpec: Record<string, unknown>
    }
    expect(body.sessionId).toBe('sess-1')
    expect(body.turnCount).toBe(1)
    expect(body.status).toBe('interviewing')
    expect(body.reply).toContain('assistant-reply')
    expect(body.outputSpec).toEqual({})
  })

  test('a multi-turn session persists and advances on the same env', async () => {
    const routes = routesWith(echoInference)
    const env = makeEnv()

    await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-multi', { userText: 'turn one' }),
        env,
      ),
    )
    const second = await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-multi', { userText: 'turn two' }),
        env,
      ),
    )
    const body = (await second.json()) as { turnCount: number }
    expect(body.turnCount).toBe(2)
  })

  test('rejects an empty user text with a validation error (400)', async () => {
    const routes = routesWith(echoInference)
    const response = await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-2', { userText: '   ' }),
        makeEnv(),
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('validation_error')
  })

  test('rejects a malformed body (400)', async () => {
    const routes = routesWith(echoInference)
    const response = await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-3', { notUserText: 'oops' }),
        makeEnv(),
      ),
    )
    expect(response.status).toBe(400)
  })

  test('maps an inference failure to a stable 502', async () => {
    const routes = routesWith(failingInference)
    const response = await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-4', { userText: 'hello' }),
        makeEnv(),
      ),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('inference_unavailable')
  })

  test('passes the vertical overlay through to the inference seam', async () => {
    const seen: string[] = []
    const overlayInference: OnboardingInferenceClient = request => {
      const system = request.messages[0]?.content ?? ''
      seen.push(system)
      return Effect.succeed('ok')
    }
    const routes = routesWith(overlayInference)
    await run(
      routes.routeOnboardingTurnRequest(
        turnRequest('sess-legal', {
          userText: 'I run a law firm.',
          verticalOverlay: 'Legal vertical: review-gated, no legal advice.',
        }),
        makeEnv(),
      ),
    )
    expect(seen[0]).toContain('Legal vertical')
    expect(seen[0]).toContain('VERTICAL OVERLAY')
  })
})
