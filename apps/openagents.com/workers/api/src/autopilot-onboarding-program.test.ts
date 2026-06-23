import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildHonestyContractBlock,
  buildOnboardingSystemPrompt,
} from './autopilot-onboarding-system-prompt'
import {
  KHALA_ONBOARDING_MODEL,
  type OnboardingInferenceClient,
  OnboardingInferenceError,
  type OnboardingSessionStore,
  makeD1OnboardingSessionStore,
  runOnboardingTurn,
} from './autopilot-onboarding-program'
import type { InferenceRequest } from './inference/provider-adapter'
import { publicProductPromisesDocument } from './product-promises'

// Minimal real-sqlite D1 shim (same shape used by other route/store tests).
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
  async all<T = Row>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
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
  async batch(statements: ReadonlyArray<Stmt>): Promise<Array<{ success: true }>> {
    for (const s of statements) await s.run()
    return statements.map(() => ({ success: true as const }))
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

const makeD1 = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new D1(raw) as unknown as D1Database
}

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

// An inference stub that records the messages it received and returns a fixed
// reply, so tests can assert on the assembled prompt and on persistence.
const recordingInferenceClient = (
  reply: string,
): { client: OnboardingInferenceClient; calls: InferenceRequest[] } => {
  const calls: InferenceRequest[] = []
  const client: OnboardingInferenceClient = request => {
    calls.push(request)
    return Effect.succeed(reply)
  }
  return { calls, client }
}

const failingInferenceClient: OnboardingInferenceClient = () =>
  new OnboardingInferenceError({ reason: 'no provider lane configured' })

describe('onboarding system prompt', () => {
  test('honesty contract is rebuilt from the live promise registry', () => {
    const block = buildHonestyContractBlock()
    const document = publicProductPromisesDocument()

    expect(block).toContain('HONESTY CONTRACT')
    expect(block).toContain(document.version)
    // It lists the offering bindings with their live availability label.
    expect(block).toContain('1. Coding & agent work')
    expect(block).toContain('5. Distributed compute / training')
  })

  test('green promises are sellable as Available now; roadmap promises are not', () => {
    const block = buildHonestyContractBlock()
    const document = publicProductPromisesDocument()

    const greenForumPromise = document.promises.find(
      p => p.promiseId === 'labor.forum_work_requests.v1',
    )
    expect(greenForumPromise?.state).toBe('green')
    // A green-gated line is labeled Available now.
    expect(block).toContain('labor.forum_work_requests.v1')
    expect(block).toMatch(
      /labor\.forum_work_requests\.v1.*Available now/,
    )

    // The cloud fine-tuning service is red/planned in the registry, so it must
    // be presented as Roadmap, never Available now.
    const fineTuning = document.promises.find(
      p => p.promiseId === 'cloud.fine_tuning_service.v1',
    )
    if (fineTuning !== undefined) {
      expect(['red', 'planned', 'yellow', 'degraded']).toContain(
        fineTuning.state,
      )
      expect(block).toMatch(
        /cloud\.fine_tuning_service\.v1.*(Roadmap|Operator-assisted)/,
      )
    }
  })

  test('the prompt instructs the agent to refuse promising beyond the registry', () => {
    const prompt = buildOnboardingSystemPrompt(null)
    expect(prompt).toContain('Do NOT promise it')
    expect(prompt).toContain('Never invent capabilities')
    // It carries the interview script (7 areas) and the 10-section spec.
    expect(prompt).toContain('THE SEVEN AREAS')
    expect(prompt).toContain('THE 10-SECTION OUTPUT SPEC')
  })

  test('vertical overlay is injected but does not relax the honesty contract', () => {
    const prompt = buildOnboardingSystemPrompt(
      'Legal vertical: never give legal advice; everything is review-gated.',
    )
    expect(prompt).toContain('VERTICAL OVERLAY')
    expect(prompt).toContain('Legal vertical')
    expect(prompt).toContain('does NOT relax the honesty contract')
    // The honesty contract is still present below the overlay.
    expect(prompt).toContain('HONESTY CONTRACT')
  })

  test('no overlay produces no overlay block', () => {
    expect(buildOnboardingSystemPrompt(null)).not.toContain('VERTICAL OVERLAY')
    expect(buildOnboardingSystemPrompt('')).not.toContain('VERTICAL OVERLAY')
  })
})

describe('onboarding turn driver', () => {
  test('a first turn creates a session, calls khala-mini, and persists', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())
    const { calls, client } = recordingInferenceClient(
      'Great — to start, in a sentence or two, what does your business do?',
    )

    const result = await run(
      runOnboardingTurn(
        { sessionId: 'sess-1', userText: 'Hi, I want help.', verticalOverlay: null },
        { infer: client, nowIso: () => '2026-06-23T00:00:00.000Z', store },
      ),
    )

    expect(result.sessionId).toBe('sess-1')
    expect(result.turnCount).toBe(1)
    expect(result.status).toBe('interviewing')
    expect(result.reply).toContain('what does your business do')

    // It dispatched to khala-mini with a system prompt then the user turn.
    expect(calls).toHaveLength(1)
    expect(calls[0]?.model).toBe(KHALA_ONBOARDING_MODEL)
    expect(calls[0]?.messages[0]?.role).toBe('system')
    expect(calls[0]?.messages.at(-1)).toEqual({
      role: 'user',
      content: 'Hi, I want help.',
    })

    // The session persisted with both transcript turns.
    const persisted = await run(store.read('sess-1'))
    expect(persisted?.turnCount).toBe(1)
    expect(persisted?.transcript).toHaveLength(2)
  })

  test('a multi-turn session advances and replays prior transcript', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())
    const first = recordingInferenceClient('Got it. Who are your customers?')

    await run(
      runOnboardingTurn(
        { sessionId: 'sess-2', userText: 'We run a bakery.', verticalOverlay: null },
        { infer: first.client, nowIso: () => '2026-06-23T00:00:00.000Z', store },
      ),
    )

    const second = recordingInferenceClient(
      'Thanks. What outcome do you want this month?',
    )
    const result = await run(
      runOnboardingTurn(
        {
          sessionId: 'sess-2',
          userText: 'Mostly local walk-ins.',
          verticalOverlay: null,
        },
        { infer: second.client, nowIso: () => '2026-06-23T00:01:00.000Z', store },
      ),
    )

    expect(result.turnCount).toBe(2)
    // The second call's messages replay the prior exchange before the new turn.
    const messages = second.calls[0]?.messages ?? []
    expect(messages.map(m => m.content)).toContain('We run a bakery.')
    expect(messages.map(m => m.content)).toContain('Got it. Who are your customers?')
    expect(messages.at(-1)?.content).toBe('Mostly local walk-ins.')

    const persisted = await run(store.read('sess-2'))
    expect(persisted?.transcript).toHaveLength(4)
  })

  test('the vertical overlay is fixed on session creation and carried forward', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())
    const { calls, client } = recordingInferenceClient('ok')

    await run(
      runOnboardingTurn(
        {
          sessionId: 'sess-legal',
          userText: 'I run a small law firm.',
          verticalOverlay: 'Legal vertical: review-gated, no legal advice.',
        },
        { infer: client, nowIso: () => '2026-06-23T00:00:00.000Z', store },
      ),
    )

    expect(calls[0]?.messages[0]?.content).toContain('Legal vertical')
    const persisted = await run(store.read('sess-legal'))
    expect(persisted?.verticalOverlay).toBe(
      'Legal vertical: review-gated, no legal advice.',
    )
  })

  test('empty user text is a validation error and does not persist', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())
    const { client } = recordingInferenceClient('ok')

    const exit = await Effect.runPromiseExit(
      runOnboardingTurn(
        { sessionId: 'sess-3', userText: '   ', verticalOverlay: null },
        { infer: client, nowIso: () => '2026-06-23T00:00:00.000Z', store },
      ),
    )

    expect(exit._tag).toBe('Failure')
    const persisted = await run(store.read('sess-3'))
    expect(persisted).toBeUndefined()
  })

  test('an inference failure surfaces as a typed error and does not advance', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())

    const exit = await Effect.runPromiseExit(
      runOnboardingTurn(
        { sessionId: 'sess-4', userText: 'hello', verticalOverlay: null },
        {
          infer: failingInferenceClient,
          nowIso: () => '2026-06-23T00:00:00.000Z',
          store,
        },
      ),
    )

    expect(exit._tag).toBe('Failure')
    // The session was never persisted because the inference call failed first.
    const persisted = await run(store.read('sess-4'))
    expect(persisted).toBeUndefined()
  })
})

describe('onboarding session store', () => {
  test('read returns undefined for an unknown session', async () => {
    const store: OnboardingSessionStore = makeD1OnboardingSessionStore(makeD1())
    const result = await run(store.read('nope'))
    expect(result).toBeUndefined()
  })

  test('upsert is idempotent on the session id (overwrites prior state)', async () => {
    const store = makeD1OnboardingSessionStore(makeD1())
    await run(
      store.upsert({
        id: 'sess-5',
        verticalOverlay: null,
        status: 'interviewing',
        transcript: [{ role: 'user', content: 'a' }],
        outputSpec: { business: 'bakery' },
        turnCount: 1,
        createdAt: '2026-06-23T00:00:00.000Z',
        updatedAt: '2026-06-23T00:00:00.000Z',
      }),
    )
    await run(
      store.upsert({
        id: 'sess-5',
        verticalOverlay: null,
        status: 'complete',
        transcript: [
          { role: 'user', content: 'a' },
          { role: 'assistant', content: 'b' },
        ],
        outputSpec: { business: 'bakery', quickWin: 'fix site' },
        turnCount: 2,
        createdAt: '2026-06-23T00:00:00.000Z',
        updatedAt: '2026-06-23T00:02:00.000Z',
      }),
    )

    const persisted = await run(store.read('sess-5'))
    expect(persisted?.status).toBe('complete')
    expect(persisted?.turnCount).toBe(2)
    expect(persisted?.outputSpec.quickWin).toBe('fix site')
    expect(persisted?.transcript).toHaveLength(2)
  })
})
