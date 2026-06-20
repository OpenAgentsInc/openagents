import { describe, expect, test } from 'vitest'

import { artanisMindComplete } from './artanis-mind'
import {
  type ComposerForumPost,
  type ComposerTip,
  runArtanisComposerTick,
} from './artanis-reply-composer'
import { artanisResponderTipReceiptRef } from './tip-ladder'

type MindInput = Parameters<typeof artanisMindComplete>[0]

type ComposerActionRow = {
  id: string
  topic_id: string
  first_post_id: string
  question_class: string
  state: string
  proposal_json: string
  reply_post_id: string | null
  asked_at: string
  replied_at: string | null
  created_at: string
  updated_at: string
  tip_receipt_ref: string | null
  tip_pay_in_id: string | null
  tip_ladder_rung: string | null
  tip_ladder_reason: string | null
}

type ComposerStore = {
  actions: Array<ComposerActionRow>
  bodies: Map<string, string>
  payIns: Array<{
    cost_msat: number
    created_at: string
    idempotency_key: string
    pay_in_type: string
    payer_ref: string
    state: string
  }>
  topics: Map<string, { title: string }>
}

type BindValue = string | number | null

class ComposerStatement {
  constructor(
    private readonly query: string,
    private readonly store: ComposerStore,
    private readonly values: ReadonlyArray<BindValue> = [],
  ) {}

  bind(...values: Array<BindValue>) {
    return new ComposerStatement(this.query, this.store, values)
  }

  async all() {
    if (this.query.includes('FROM artanis_responder_actions a')) {
      const limit = Number(this.values[0] ?? 0)
      const results = [...this.store.actions]
        .filter(action => action.state === 'proposed')
        .sort((left, right) => left.created_at.localeCompare(right.created_at))
        .slice(0, limit)
        .map(action => ({
          asked_at: action.asked_at,
          body_text: this.store.bodies.get(action.first_post_id) ?? '',
          first_post_id: action.first_post_id,
          id: action.id,
          question_class: action.question_class,
          title: this.store.topics.get(action.topic_id)?.title ?? '',
          topic_id: action.topic_id,
        }))

      return { results, success: true }
    }

    return { results: [], success: true }
  }

  async first() {
    if (this.query.includes('FROM pay_ins')) {
      const payerRef = String(this.values[0] ?? '')
      const sinceIso = String(this.values[1] ?? '')
      const spent = this.store.payIns
        .filter(
          payIn =>
            payIn.payer_ref === payerRef &&
            payIn.pay_in_type === 'tip' &&
            payIn.state === 'paid' &&
            payIn.idempotency_key.startsWith('artanis-responder-tip:') &&
            payIn.created_at >= sinceIso,
        )
        .reduce((sum, payIn) => sum + payIn.cost_msat, 0)

      return { spent }
    }

    return null
  }

  async run() {
    if (this.query.includes("SET state = 'blocked'")) {
      this.updateAction(String(this.values[2] ?? ''), action => {
        action.state = 'blocked'
        action.proposal_json = String(this.values[0] ?? '{}')
        action.updated_at = String(this.values[1] ?? '')
      })
    }

    if (this.query.includes("SET state = 'responded'")) {
      this.updateAction(String(this.values[3] ?? ''), action => {
        action.state = 'responded'
        action.reply_post_id = String(this.values[0] ?? '')
        action.replied_at = String(this.values[1] ?? '')
        action.updated_at = String(this.values[2] ?? '')
      })
    }

    if (this.query.includes("SET state = 'tipped'")) {
      this.updateAction(String(this.values[5] ?? ''), action => {
        action.state = 'tipped'
        action.tip_receipt_ref = String(this.values[0] ?? '')
        action.tip_pay_in_id = String(this.values[1] ?? '')
        action.tip_ladder_rung = String(this.values[2] ?? '')
        action.tip_ladder_reason = String(this.values[3] ?? '')
        action.updated_at = String(this.values[4] ?? '')
      })
    }

    return { meta: { changes: 1 }, results: [], success: true }
  }

  async raw() {
    return []
  }

  private updateAction(
    actionId: string,
    update: (action: ComposerActionRow) => void,
  ) {
    const action = this.store.actions.find(row => row.id === actionId)

    if (action !== undefined) {
      update(action)
    }
  }
}

const composerDb = (store: ComposerStore): D1Database =>
  ({
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    prepare: (query: string) => new ComposerStatement(query, store),
    withSession: () => composerDb(store),
  }) as unknown as D1Database

const composerStore = (): ComposerStore => ({
  actions: [
    {
      asked_at: '2026-06-20T03:09:58.000Z',
      created_at: '2026-06-20T03:10:00.000Z',
      first_post_id: 'post_question_1',
      id: 'action_1',
      proposal_json: '{}',
      question_class: 'pylon_troubleshooting',
      replied_at: null,
      reply_post_id: null,
      state: 'proposed',
      tip_ladder_reason: null,
      tip_ladder_rung: null,
      tip_pay_in_id: null,
      tip_receipt_ref: null,
      topic_id: 'topic_1',
      updated_at: '2026-06-20T03:10:00.000Z',
    },
  ],
  bodies: new Map([
    [
      'post_question_1',
      'How do I make sparkPayoutTargetReady true and keep executor-trace capability refs through heartbeat?',
    ],
  ]),
  payIns: [],
  topics: new Map([
    [
      'topic_1',
      {
        title: 'Pylon preflight payout and capability refs',
      },
    ],
  ]),
})

const successfulMind =
  (text: string) =>
  async (input: MindInput): ReturnType<typeof artanisMindComplete> => ({
    gatewayId: null,
    model: 'test-model',
    promptChars: input.prompt.length,
    responseChars: text.length,
    servedVia: 'google_direct',
    text,
  })

const runComposer = async (
  store: ComposerStore,
  deps: Readonly<{
    forumPost: ComposerForumPost
    mindText?: string
    tip: ComposerTip
  }>,
) =>
  runArtanisComposerTick(composerDb(store), {
    artanisActorRef: 'agent:artanis',
    forumPost: deps.forumPost,
    geminiApiKey: 'gemini-test-key',
    mindComplete: successfulMind(
      deps.mindText ??
        'Set sparkPayoutTargetReady from the wallet-readiness path and preserve executor-trace capability refs in the heartbeat payload.\n\n- Artanis (automated responder; the mind proposes, schemas validate, gates hold)',
    ),
    nowIso: '2026-06-20T03:15:16.000Z',
    tip: deps.tip,
  })

describe('Artanis reply composer receipts', () => {
  test('tips before posting and publishes only the returned receipt ref', async () => {
    const store = composerStore()
    const events: Array<string> = []
    const posts: Array<Parameters<ComposerForumPost>[0]> = []
    const returnedReceiptRef =
      'receipt.forum.tip_ladder.resolved_artanis_topic_1'

    const outcome = await runComposer(store, {
      forumPost: async input => {
        events.push('post')
        posts.push(input)
        return { postId: 'post_reply_1' }
      },
      tip: async () => {
        events.push('tip')
        return {
          ladderReason: 'recipient_destination_missing',
          payInId: 'payin_1',
          receiptRef: returnedReceiptRef,
          rung: 'credited',
        }
      },
    })

    expect(events).toEqual(['tip', 'post'])
    expect(outcome).toMatchObject({ blocked: 0, responded: 1, tipped: 1 })
    expect(posts[0]?.bodyText).toContain(
      `Responder tip receipt: ${returnedReceiptRef}`,
    )
    expect(posts[0]?.bodyText).not.toContain(
      artanisResponderTipReceiptRef('topic_1'),
    )
    expect(store.actions[0]).toMatchObject({
      reply_post_id: 'post_reply_1',
      state: 'tipped',
      tip_ladder_reason: 'recipient_destination_missing',
      tip_ladder_rung: 'credited',
      tip_pay_in_id: 'payin_1',
      tip_receipt_ref: returnedReceiptRef,
    })
  })

  test('posts no receipt claim when the tip path fails', async () => {
    const store = composerStore()
    const events: Array<string> = []
    const posts: Array<Parameters<ComposerForumPost>[0]> = []

    const outcome = await runComposer(store, {
      forumPost: async input => {
        events.push('post')
        posts.push(input)
        return { postId: 'post_reply_1' }
      },
      tip: async () => {
        events.push('tip')
        return { error: 'tip_ladder_failed' }
      },
    })

    expect(events).toEqual(['tip', 'post'])
    expect(outcome).toMatchObject({ blocked: 0, responded: 1, tipped: 0 })
    expect(posts[0]?.bodyText).not.toContain('Responder tip receipt:')
    expect(posts[0]?.bodyText).not.toContain('receipt.forum.tip_ladder.')
    expect(store.actions[0]).toMatchObject({
      reply_post_id: 'post_reply_1',
      state: 'responded',
      tip_receipt_ref: null,
    })
  })
})
