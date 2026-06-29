import { decodeLbrAcceptanceEvent } from '@openagentsinc/nip90'
import { type Event as SignedNostrEvent, verifyEvent } from 'nostr-effect/pure'
import { describe, expect, it } from 'vitest'

import {
  type ForumWorkRequestRelaySocket,
  forumWorkRequestRelayPublisherForEnv,
  makeLiveForumWorkRequestRelayPublisher,
} from './forum-work-request-live-publisher'
import type {
  ForumWorkRequestAcceptanceRelayPublishInput,
  ForumWorkRequestRelayPublishInput,
} from './forum-work-requests'

const testMarketSecretKeyHex = 'aa'.repeat(32)

const acceptanceInput = (): ForumWorkRequestAcceptanceRelayPublishInput => ({
  acceptanceRef: 'acceptance.public.forum_lbr.wr1.quote1',
  escrowReceiptRef: 'receipt.labor_escrow.reserve.wr1.quote1',
  jobEventId: 'a'.repeat(64),
  providerPubkey: '2'.repeat(64),
  quoteRef: 'quote.public.live.one',
  relayUrl: 'wss://relay.openagents.com',
  workRequestId: 'wr1',
})

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
  relayUrl: 'wss://relay.openagents.com',
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

describe('makeLiveForumWorkRequestRelayPublisher.publishAcceptance', () => {
  it('signs a ref-only kind-7000 acceptance carrying the reserve receipt + provider pubkey', async () => {
    // Distinct from any all-`a`/all-`2` fixture refs so the secret-leak check
    // cannot coincidentally collide with public event material.
    const acceptanceMarketSecretKeyHex = 'bc'.repeat(32)
    const socket = ackingSocket(true)
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => socket,
      marketSecretKeyHex: acceptanceMarketSecretKeyHex,
      nowEpochSeconds: () => 1_770_000_000,
    })

    const input = acceptanceInput()
    const receipt = await publisher.publishAcceptance!(input)

    expect(receipt.accepted).toBe(true)
    expect(receipt.relayRef).toMatch(/^relay\.public\.market\.[0-9a-f]{32}$/)

    const event = receipt.event as SignedNostrEvent
    expect(receipt.acceptanceEventId).toBe(event.id)
    expect(event.kind).toBe(7000)
    expect(verifyEvent(event)).toBe(true)

    const decoded = decodeLbrAcceptanceEvent(event)
    expect(decoded.escrowReceiptRef).toBe(input.escrowReceiptRef)
    expect(decoded.acceptanceRef).toBe(input.acceptanceRef)
    expect(decoded.providerPubkey).toBe(input.providerPubkey)
    expect(decoded.requestId).toBe(input.jobEventId)

    // public-safe: no wallet/payment material and never the market secret key.
    expect(JSON.stringify(receipt)).not.toContain(acceptanceMarketSecretKeyHex)
    expect(JSON.stringify(receipt)).not.toMatch(
      /lnbc|preimage|payment_hash|mnemonic|secret|xprv|\/Users\//i,
    )
    expect(socket.closed()).toBe(true)
  })

  it('refuses without connecting when the market key is malformed', async () => {
    let connected = false
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => {
        connected = true
        return makeFakeSocket()
      },
      marketSecretKeyHex: 'not-a-key',
    })

    const receipt = await publisher.publishAcceptance!(acceptanceInput())

    expect(receipt.accepted).toBe(false)
    expect(receipt.acceptanceEventId).toBe(null)
    expect(receipt.relayRef).toMatch(/^relay\.public\.market_key_invalid\./)
    expect(connected).toBe(false)
  })

  it('refuses without connecting when refs are protocol-unsafe', async () => {
    let connected = false
    const publisher = makeLiveForumWorkRequestRelayPublisher({
      connect: async () => {
        connected = true
        return makeFakeSocket()
      },
      marketSecretKeyHex: testMarketSecretKeyHex,
    })

    const receipt = await publisher.publishAcceptance!({
      ...acceptanceInput(),
      // not a 64-hex request id: draft construction must reject before publish
      jobEventId: 'not-a-hex-id',
    })

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toMatch(
      /^relay\.public\.acceptance_draft_invalid\./,
    )
    expect(connected).toBe(false)
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
