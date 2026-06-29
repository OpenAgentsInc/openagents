/**
 * Live Forum work-request relay publisher (P1, #4777).
 *
 * Signs the ref-only NIP-LBR kind-5934 draft with the operator-configured
 * market key and publishes it to the owned scoped market relay. This is the
 * production counterpart of `defaultForumWorkRequestRelayPublisher`, which
 * deterministically rejects so unconfigured deploys never publish by
 * accident.
 *
 * Boundaries:
 * - The market secret key never appears in receipts, events, errors, or logs.
 * - Any failure (bad key, upgrade refusal, relay NACK, timeout) returns
 *   `accepted: false` with a public-safe relayRef reason slug; the route
 *   surfaces that as a 503 before any Forum/DB side effects.
 */
import {
  lbrAcceptanceToDraft,
  makeLbrAcceptance,
} from '@openagentsinc/nip90'
import { finalizeEvent } from 'nostr-effect/pure'

import { sha256Hex } from './agent-registration'
import type {
  ForumWorkRequestAcceptanceRelayPublishInput,
  ForumWorkRequestAcceptanceRelayPublishReceipt,
  ForumWorkRequestRelayPublishInput,
  ForumWorkRequestRelayPublishReceipt,
  ForumWorkRequestRelayPublisher,
} from './forum-work-requests'
import { parseJsonUnknown } from './json-boundary'
import { currentEpochSeconds } from './runtime-primitives'

export type ForumWorkRequestRelaySocket = Readonly<{
  addEventListener: (
    type: 'close' | 'error' | 'message',
    handler: (event: { data?: unknown }) => void,
  ) => void
  close: () => void
  send: (data: string) => void
}>

export type ForumWorkRequestRelayConnector = (
  relayUrl: string,
) => Promise<ForumWorkRequestRelaySocket>

export type LiveForumWorkRequestRelayPublisherOptions = Readonly<{
  /** 64-char hex Nostr secret key for the bridge-held market identity. */
  marketSecretKeyHex: string
  /** Injectable relay socket factory; defaults to Workers fetch-upgrade. */
  connect?: ForumWorkRequestRelayConnector
  nowEpochSeconds?: () => number
  publishTimeoutMs?: number
}>

const MarketSecretKeyHexPattern = /^[0-9a-f]{64}$/i

const DefaultPublishTimeoutMs = 10_000

export class ForumWorkRequestRelayConnectionError extends Error {
  readonly _tag = 'ForumWorkRequestRelayConnectionError'

  constructor(reason: string) {
    super(reason)
    this.name = 'ForumWorkRequestRelayConnectionError'
  }
}

const hexToBytes = (hex: string): Uint8Array => {
  const bytes = new Uint8Array(hex.length / 2)

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16)
  }

  return bytes
}

/**
 * Cloudflare Workers outbound WebSocket via fetch upgrade. The standard
 * `new WebSocket(url)` client constructor is not available in workerd;
 * upgrading a fetch is the supported path.
 */
export const workersFetchRelayConnector: ForumWorkRequestRelayConnector =
  async relayUrl => {
    const httpUrl = relayUrl.replace(/^ws(s?):\/\//i, 'http$1://')
    const response = await fetch(httpUrl, {
      headers: { Upgrade: 'websocket' },
    })
    const socket = (response as { webSocket?: WebSocket | null }).webSocket

    if (socket === undefined || socket === null) {
      throw new ForumWorkRequestRelayConnectionError(
        'relay refused websocket upgrade',
      )
    }

    ;(socket as unknown as { accept: () => void }).accept()

    return socket as unknown as ForumWorkRequestRelaySocket
  }

const failureReceipt = async (
  input: ForumWorkRequestRelayPublishInput,
  reasonSlug: string,
): Promise<ForumWorkRequestRelayPublishReceipt> => ({
  accepted: false,
  event: null,
  jobEventId: `event.unpublished.${(
    await sha256Hex(JSON.stringify(input.draft))
  ).slice(0, 32)}`,
  relayRef: `relay.public.${reasonSlug}.${(
    await sha256Hex(input.relayUrl)
  ).slice(0, 32)}`,
  relayUrl: input.relayUrl,
})

const awaitRelayOk = (
  socket: ForumWorkRequestRelaySocket,
  eventId: string,
  timeoutMs: number,
): Promise<Readonly<{ accepted: boolean }>> =>
  new Promise(resolve => {
    const timeout = setTimeout(() => {
      resolve({ accepted: false })
    }, timeoutMs)
    const settle = (accepted: boolean) => {
      clearTimeout(timeout)
      resolve({ accepted })
    }

    socket.addEventListener('message', event => {
      try {
        const parsed = parseJsonUnknown(String(event.data))

        if (
          Array.isArray(parsed) &&
          parsed[0] === 'OK' &&
          parsed[1] === eventId
        ) {
          settle(parsed[2] === true)
        }
      } catch {
        settle(false)
      }
    })
    socket.addEventListener('error', () => {
      settle(false)
    })
    socket.addEventListener('close', () => {
      settle(false)
    })
  })

const acceptanceFailureReceipt = async (
  input: ForumWorkRequestAcceptanceRelayPublishInput,
  reasonSlug: string,
): Promise<ForumWorkRequestAcceptanceRelayPublishReceipt> => ({
  accepted: false,
  acceptanceEventId: null,
  event: null,
  relayRef: `relay.public.${reasonSlug}.${(
    await sha256Hex(input.relayUrl)
  ).slice(0, 32)}`,
  relayUrl: input.relayUrl,
})

export const makeLiveForumWorkRequestRelayPublisher = (
  options: LiveForumWorkRequestRelayPublisherOptions,
): ForumWorkRequestRelayPublisher => {
  const connect = options.connect ?? workersFetchRelayConnector
  const nowEpochSeconds = options.nowEpochSeconds ?? currentEpochSeconds
  const publishTimeoutMs = options.publishTimeoutMs ?? DefaultPublishTimeoutMs

  return {
    publishAcceptance: async input => {
      if (!MarketSecretKeyHexPattern.test(options.marketSecretKeyHex)) {
        return acceptanceFailureReceipt(input, 'market_key_invalid')
      }

      let draft
      try {
        const acceptance = makeLbrAcceptance({
          acceptanceRef: input.acceptanceRef,
          escrowReceiptRef: input.escrowReceiptRef,
          providerPubkey: input.providerPubkey,
          requestId: input.jobEventId,
          requestRelay: input.relayUrl,
        })
        draft = lbrAcceptanceToDraft(acceptance)
      } catch {
        // Protocol-unsafe refs or a non-hex job event id / provider pubkey:
        // never publish, surface a public-safe failure slug instead.
        return acceptanceFailureReceipt(input, 'acceptance_draft_invalid')
      }

      const event = finalizeEvent(
        {
          content: draft.content,
          created_at: nowEpochSeconds(),
          kind: draft.kind,
          tags: draft.tags.map(tag => [...tag]),
        },
        hexToBytes(options.marketSecretKeyHex.toLowerCase()),
      )

      let socket: ForumWorkRequestRelaySocket

      try {
        socket = await connect(input.relayUrl)
      } catch {
        return acceptanceFailureReceipt(input, 'relay_connect_failed')
      }

      try {
        const okPromise = awaitRelayOk(socket, event.id, publishTimeoutMs)

        socket.send(JSON.stringify(['EVENT', event]))

        const verdict = await okPromise

        if (!verdict.accepted) {
          return acceptanceFailureReceipt(input, 'relay_publish_rejected')
        }

        return {
          accepted: true,
          acceptanceEventId: event.id,
          event,
          relayRef: `relay.public.market.${(
            await sha256Hex(input.relayUrl)
          ).slice(0, 32)}`,
          relayUrl: input.relayUrl,
        }
      } finally {
        try {
          socket.close()
        } catch {
          // socket already closed
        }
      }
    },
    publishWorkRequest: async input => {
      if (!MarketSecretKeyHexPattern.test(options.marketSecretKeyHex)) {
        return failureReceipt(input, 'market_key_invalid')
      }

      const event = finalizeEvent(
        {
          content: input.draft.content,
          created_at: nowEpochSeconds(),
          kind: input.draft.kind,
          tags: input.draft.tags.map(tag => [...tag]),
        },
        hexToBytes(options.marketSecretKeyHex.toLowerCase()),
      )

      let socket: ForumWorkRequestRelaySocket

      try {
        socket = await connect(input.relayUrl)
      } catch {
        return failureReceipt(input, 'relay_connect_failed')
      }

      try {
        const okPromise = awaitRelayOk(socket, event.id, publishTimeoutMs)

        socket.send(JSON.stringify(['EVENT', event]))

        const verdict = await okPromise

        if (!verdict.accepted) {
          return failureReceipt(input, 'relay_publish_rejected')
        }

        return {
          accepted: true,
          event,
          jobEventId: event.id,
          relayRef: `relay.public.market.${(
            await sha256Hex(input.relayUrl)
          ).slice(0, 32)}`,
          relayUrl: input.relayUrl,
        }
      } finally {
        try {
          socket.close()
        } catch {
          // socket already closed
        }
      }
    },
  }
}

/**
 * Env-driven construction: returns the live publisher only when the operator
 * has configured `FORUM_WORK_REQUEST_MARKET_SECRET_KEY` (wrangler secret).
 * Unconfigured deploys keep the rejecting default publisher.
 */
export const forumWorkRequestRelayPublisherForEnv = (
  environment: unknown,
): ForumWorkRequestRelayPublisher | undefined => {
  const secret = (
    environment as { FORUM_WORK_REQUEST_MARKET_SECRET_KEY?: string }
  ).FORUM_WORK_REQUEST_MARKET_SECRET_KEY

  if (typeof secret !== 'string' || secret.length === 0) {
    return undefined
  }

  return makeLiveForumWorkRequestRelayPublisher({ marketSecretKeyHex: secret })
}
