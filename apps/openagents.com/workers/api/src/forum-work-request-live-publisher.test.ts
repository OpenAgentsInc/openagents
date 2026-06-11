import { type Event as SignedNostrEvent, verifyEvent } from 'nostr-effect/pure'
import { describe, expect, it } from 'vitest'

import {
  type ForumWorkRequestRelaySocket,
  forumWorkRequestRelayPublisherForEnv,
  makeLiveForumWorkRequestRelayPublisher,
} from './forum-work-request-live-publisher'
import type { ForumWorkRequestRelayPublishInput } from './forum-work-requests'

const testMarketSecretKeyHex = 'aa'.repeat(32)

const publishInput = (): ForumWorkRequestRelayPublishInput => ({
  bridgeActorRef: 'agent:openagents_market_bridge',
  draft: {
    content: '',
    kind: 5934,
    tags: [
      ['i', 'issue.public.openagents.4781', 'text'],
      [
        'param',
        'verification_command_ref',
        'command.public.pylon.labor.bun_test',
      ],
      ['bid', '2000000'],
      ['output', 'output_only'],
    ],
  },
  idempotencyKey: 'p1-live-publisher-test',
  lbrRequest: {} as ForumWorkRequestRelayPublishInput['lbrRequest'],
  relayUrl: 'wss://openagents-market-relay.openagents.workers.dev',
  topicId: 'topic-p1',
  workRequestId: 'work-request-p1',
})

type FakeSocket = ForumWorkRequestRelaySocket &
  Readonly<{
    closed: () => boolean
    emit: (type: 'close' | 'error' | 'message', data?: unknown) => void
    sent: () => ReadonlyArray<string>
  }>

const makeFakeSocket = (
  onSend?: (data: string, socket: FakeSocket) => void,
): FakeSocket => {
  const handlers = new Map<string, Array<(event: { data?: unknown }) => void>>()
  const sent: Array<string> = []
  let closed = false
  const socket: FakeSocket = {
    addEventListener: (type, handler) => {
      handlers.set(type, [...(handlers.get(type) ?? []), handler])
    },
    close: () => {
      closed = true
    },
    closed: () => closed,
    emit: (type, data) => {
      for (const handler of handlers.get(type) ?? []) {
        handler({ data })
      }
    },
    send: data => {
      sent.push(data)
      onSend?.(data, socket)
    },
    sent: () => sent,
  }

  return socket
}

const ackingSocket = (ok: boolean): FakeSocket =>
  makeFakeSocket((data, socket) => {
    const [, event] = JSON.parse(data) as ['EVENT', SignedNostrEvent]

    queueMicrotask(() => {
      socket.emit('message', JSON.stringify(['OK', event.id, ok, '']))
    })
  })

describe('makeLiveForumWorkRequestRelayPublisher', () => {
  it('signs the LBR draft and returns an accepted receipt on relay OK', async () => {
    const socket = ackingSocket(true)
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => socket,
      marketSecretKeyHex: testMarketSecretKeyHex,
      nowEpochSeconds: () => 1_770_000_000,
    })

    const input = publishInput()
    const receipt = await publisher.publishWorkRequest(input)

    expect(receipt.accepted).toBe(true)
    expect(receipt.relayUrl).toBe(input.relayUrl)
    expect(receipt.relayRef).toMatch(/^relay\.public\.market\.[0-9a-f]{32}$/)

    const event = receipt.event as SignedNostrEvent

    expect(receipt.jobEventId).toBe(event.id)
    expect(event.kind).toBe(5934)
    expect(event.created_at).toBe(1_770_000_000)
    expect(event.tags).toEqual(input.draft.tags.map(tag => [...tag]))
    expect(event.content).toBe(input.draft.content)
    expect(verifyEvent(event)).toBe(true)
    expect(socket.closed()).toBe(true)
    expect(JSON.stringify(receipt)).not.toContain(testMarketSecretKeyHex)
  })

  it('returns a rejected receipt when the relay NACKs the event', async () => {
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => ackingSocket(false),
      marketSecretKeyHex: testMarketSecretKeyHex,
    })

    const receipt = await publisher.publishWorkRequest(publishInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(/^relay\.public\.relay_publish_rejected\./)
  })

  it('returns a rejected receipt when the relay sends malformed JSON', async () => {
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () =>
        makeFakeSocket((_data, socket) => {
          queueMicrotask(() => {
            socket.emit('message', '{')
          })
        }),
      marketSecretKeyHex: testMarketSecretKeyHex,
      publishTimeoutMs: 20,
    })

    const receipt = await publisher.publishWorkRequest(publishInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(/^relay\.public\.relay_publish_rejected\./)
  })

  it('rejects without connecting when the market key is malformed', async () => {
    let connected = false
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => {
        connected = true
        return makeFakeSocket()
      },
      marketSecretKeyHex: 'not-a-key',
    })

    const receipt = await publisher.publishWorkRequest(publishInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(/^relay\.public\.market_key_invalid\./)
    expect(connected).toBe(false)
    expect(JSON.stringify(receipt)).not.toContain('not-a-key')
  })

  it('returns a rejected receipt when the relay connection fails', async () => {
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => {
        throw new Error('upgrade refused')
      },
      marketSecretKeyHex: testMarketSecretKeyHex,
    })

    const receipt = await publisher.publishWorkRequest(publishInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(/^relay\.public\.relay_connect_failed\./)
  })

  it('returns a rejected receipt when no OK arrives before the timeout', async () => {
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => makeFakeSocket(),
      marketSecretKeyHex: testMarketSecretKeyHex,
      publishTimeoutMs: 20,
    })

    const receipt = await publisher.publishWorkRequest(publishInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(/^relay\.public\.relay_publish_rejected\./)
  })
})

describe('forumWorkRequestRelayPublisherForEnv', () => {
  it('returns undefined when the market secret is not configured', () => {
    expect(forumWorkRequestRelayPublisherForEnv({})).toBeUndefined()
    expect(
      forumWorkRequestRelayPublisherForEnv({
        FORUM_WORK_REQUEST_MARKET_SECRET_KEY: '',
      }),
    ).toBeUndefined()
  })

  it('returns a publisher when the market secret is configured', () => {
    expect(
      forumWorkRequestRelayPublisherForEnv({
        FORUM_WORK_REQUEST_MARKET_SECRET_KEY: testMarketSecretKeyHex,
      }),
    ).toBeDefined()
  })
})
