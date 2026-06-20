import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  type ComposerForumPost,
  type ComposerTip,
  runArtanisComposerTick,
} from './artanis-reply-composer'
import { isTipLadderReceiptRef } from './tip-ladder'

// Real-SQL D1 adapter backed by node:sqlite so the responder lifecycle
// (proposed -> responded -> tipped) is exercised against genuine SQL, the
// same minimal adapter pattern used by native-lists.test.ts.
type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
  }

  async run(): Promise<{ success: true }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const ARTANIS_ACTOR = 'agent:user_artanis'
const TOPIC_ID = 'topic_abc'
const FIRST_POST_ID = 'post_first'

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  // Minimal forum tables the composer reads from.
  db.exec(
    `CREATE TABLE forum_topics (
       id TEXT PRIMARY KEY,
       first_post_id TEXT,
       title TEXT NOT NULL,
       created_at TEXT NOT NULL
     )`,
  )
  db.exec(
    `CREATE TABLE forum_post_bodies (
       post_id TEXT PRIMARY KEY,
       body_text TEXT NOT NULL
     )`,
  )
  db.exec(migration('0161_artanis_responder.sql'))
  // 0169 adds the public_receipt_ref column to pay_ins; stub the minimal
  // pay_ins table so the budget query and the ALTERs apply cleanly.
  db.exec(
    `CREATE TABLE pay_ins (
       id TEXT PRIMARY KEY,
       payer_ref TEXT,
       pay_in_type TEXT,
       state TEXT,
       rung TEXT,
       cost_msat INTEGER,
       idempotency_key TEXT,
       context_ref TEXT,
       created_at TEXT
     )`,
  )
  db.exec(migration('0169_tip_ladder_public_receipts.sql'))

  // Seed one proposed responder action with an operational question.
  db.prepare(
    `INSERT INTO forum_topics (id, first_post_id, title, created_at)
     VALUES (?, ?, ?, ?)`,
  ).run(
    TOPIC_ID,
    FIRST_POST_ID,
    'How do I make sparkPayoutTargetReady true?',
    '2026-06-19T00:00:00.000Z',
  )
  db.prepare(
    `INSERT INTO forum_post_bodies (post_id, body_text) VALUES (?, ?)`,
  ).run(
    FIRST_POST_ID,
    'My Pylon shows payout-target not ready and executor-trace capability drops. How do I fix it?',
  )
  db.prepare(
    `INSERT INTO artanis_responder_actions
       (id, topic_id, first_post_id, question_class, state, proposal_json, asked_at, created_at, updated_at)
     VALUES (?, ?, ?, ?, 'proposed', '{}', ?, ?, ?)`,
  ).run(
    'action_1',
    TOPIC_ID,
    FIRST_POST_ID,
    'pylon_troubleshooting',
    '2026-06-19T00:00:00.000Z',
    '2026-06-19T00:00:00.000Z',
    '2026-06-19T00:00:00.000Z',
  )

  return new SqliteD1(db) as unknown as D1Database
}

// Capture what the composer posts so we can assert on the reply body.
const makeCapturingForumPost = (): {
  fn: ComposerForumPost
  posts: Array<{ topicId: string; bodyText: string }>
} => {
  const posts: Array<{ topicId: string; bodyText: string }> = []
  return {
    fn: async input => {
      posts.push({ bodyText: input.bodyText, topicId: input.topicId })
      return { postId: `reply_${posts.length}` }
    },
    posts,
  }
}

// A grounded mind reply long enough to look real, with the operational
// answer the question demanded.
const groundedReply =
  'To make payout-target readiness true, run pylon wallet request-payout-target-admission with kind bolt12_offer. ' +
  'Keep executor-trace capability refs live by running pylon presence heartbeat on a loop. ' +
  '- Artanis (automated responder; the mind proposes, schemas validate, gates hold)'

const stubMindFetch = (text: string) => {
  vi.stubGlobal(
    'fetch',
    async () =>
      new Response(
        JSON.stringify({ candidates: [{ content: { parts: [{ text }] } }] }),
        { status: 200 },
      ),
  )
}

const baseDeps = (overrides: Partial<Parameters<typeof runArtanisComposerTick>[1]>) => ({
  artanisActorRef: ARTANIS_ACTOR,
  forumPost: makeCapturingForumPost().fn,
  geminiApiKey: 'k',
  nowIso: '2026-06-19T01:00:00.000Z',
  tip: (async () => ({ error: 'unused' })) as ComposerTip,
  ...overrides,
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('artanis reply composer - receipt honesty (#5540 defect 1)', () => {
  beforeEach(() => stubMindFetch(groundedReply))

  test('emits NO receipt ref in the reply when the tip fails', async () => {
    const db = makeDb()
    const capture = makeCapturingForumPost()
    const failingTip: ComposerTip = async () => ({ error: 'tip_failed' })

    const outcome = await runArtanisComposerTick(
      db,
      baseDeps({ forumPost: capture.fn, tip: failingTip }),
    )

    expect(outcome.responded).toBe(1)
    expect(outcome.tipped).toBe(0)
    expect(capture.posts).toHaveLength(1)
    // The reply must not carry a tip-receipt ref at all when no tip settled.
    expect(capture.posts[0]!.bodyText).not.toContain('Responder tip receipt')
    expect(capture.posts[0]!.bodyText).not.toContain('receipt.forum.tip_ladder')

    // The action is responded, not tipped, and carries no tip_receipt_ref.
    const action = (await db
      .prepare(
        `SELECT state, tip_receipt_ref FROM artanis_responder_actions WHERE id = 'action_1'`,
      )
      .first()) as { state: string; tip_receipt_ref: string | null }
    expect(action.state).toBe('responded')
    expect(action.tip_receipt_ref).toBeNull()
  })

  test('emits the receipt ref ONLY when the tip settles, and uses the ladder-returned dereferenceable ref', async () => {
    const db = makeDb()
    const capture = makeCapturingForumPost()
    const settledRef = 'receipt.forum.tip_ladder.artanis_responder.topic_abc'
    const settlingTip: ComposerTip = async () => ({
      ladderReason: 'below_send_threshold',
      payInId: 'payin_1',
      receiptRef: settledRef,
      rung: 'credited',
    })

    const outcome = await runArtanisComposerTick(
      db,
      baseDeps({ forumPost: capture.fn, tip: settlingTip }),
    )

    expect(outcome.responded).toBe(1)
    expect(outcome.tipped).toBe(1)
    expect(isTipLadderReceiptRef(settledRef)).toBe(true)
    // The exact ladder-returned ref is what appears in the public reply.
    expect(capture.posts[0]!.bodyText).toContain(
      `Responder tip receipt: ${settledRef}`,
    )

    const action = (await db
      .prepare(
        `SELECT state, tip_receipt_ref, tip_pay_in_id FROM artanis_responder_actions WHERE id = 'action_1'`,
      )
      .first()) as {
      state: string
      tip_receipt_ref: string
      tip_pay_in_id: string
    }
    expect(action.state).toBe('tipped')
    expect(action.tip_receipt_ref).toBe(settledRef)
    expect(action.tip_pay_in_id).toBe('payin_1')
  })

  test('emits NO receipt ref when the tip returns a malformed (non-dereferenceable) ref', async () => {
    const db = makeDb()
    const capture = makeCapturingForumPost()
    const malformedTip: ComposerTip = async () => ({
      ladderReason: 'credited',
      payInId: 'payin_x',
      receiptRef: 'not-a-real-receipt-ref',
      rung: 'credited',
    })

    await runArtanisComposerTick(
      db,
      baseDeps({ forumPost: capture.fn, tip: malformedTip }),
    )

    expect(capture.posts[0]!.bodyText).not.toContain('Responder tip receipt')
  })
})

describe('artanis reply composer - operational grounding (#5540 defect 2)', () => {
  test('grounds the mind prompt on the operational runbooks, not just registry copy', async () => {
    const db = makeDb()
    // Capture the prompt the composer hands to the mind.
    let capturedBody = ''
    vi.stubGlobal('fetch', async (_input: unknown, init: RequestInit) => {
      capturedBody = String(init.body)
      return new Response(
        JSON.stringify({
          candidates: [{ content: { parts: [{ text: groundedReply }] } }],
        }),
        { status: 200 },
      )
    })

    await runArtanisComposerTick(db, baseDeps({}))

    // The grounding payload must include the operational runbook facts.
    expect(capturedBody).toContain('operationalDocs')
    expect(capturedBody).toContain('request-payout-target-admission')
    expect(capturedBody).toContain('presence heartbeat')
    expect(capturedBody).toContain('send_readiness_unproven')
    // And the registry grounding is still present.
    expect(capturedBody).toContain('promiseRegistry')
  })
})

describe('artanis reply composer - truncation (#5540 defect 3)', () => {
  test('blocks (never posts) when the mind reply is truncated at MAX_TOKENS', async () => {
    const db = makeDb()
    const capture = makeCapturingForumPost()
    // Every model call truncates, even at the escalated cap.
    vi.stubGlobal(
      'fetch',
      async () =>
        new Response(
          JSON.stringify({
            candidates: [
              {
                content: { parts: [{ text: 'To make sparkPayoutTargetReady tr' }] },
                finishReason: 'MAX_TOKENS',
              },
            ],
          }),
          { status: 200 },
        ),
    )

    const outcome = await runArtanisComposerTick(
      db,
      baseDeps({ forumPost: capture.fn }),
    )

    // No truncated reply is ever posted; the action is blocked honestly.
    expect(capture.posts).toHaveLength(0)
    expect(outcome.responded).toBe(0)
    expect(outcome.blocked).toBe(1)

    const action = (await db
      .prepare(
        `SELECT state, proposal_json FROM artanis_responder_actions WHERE id = 'action_1'`,
      )
      .first()) as { state: string; proposal_json: string }
    expect(action.state).toBe('blocked')
    expect(action.proposal_json).toContain('mind_unavailable_at_compose')
  })
})
