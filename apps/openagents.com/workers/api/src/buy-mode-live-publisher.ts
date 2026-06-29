/**
 * Live buy-mode relay publisher.
 *
 * Signs the already-built NIP-90 kind-5050 job request with an
 * operator-configured market key and publishes it to the scoped market relay.
 * Unconfigured deploys return undefined so the operator route keeps its
 * fail-closed default publisher.
 */
import { finalizeEvent } from 'nostr-effect/pure'

import { sha256Hex } from './agent-registration'
import type {
  BuyModeRelayJobRequest,
  BuyModeRelayPublisher,
  BuyModeRelayPublishReceipt,
} from './buy-mode-dispatcher'
import { parseJsonUnknown } from './json-boundary'
import { currentEpochSeconds } from './runtime-primitives'

export type BuyModeRelaySocket = Readonly<{
  addEventListener: (
    type: 'close' | 'error' | 'message',
    handler: (event: { data?: unknown }) => void,
  ) => void
  close: () => void
  send: (data: string) => void
}>

export type BuyModeRelayConnector = (
  relayUrl: string,
) => Promise<BuyModeRelaySocket>

type LiveBuyModeRelayPublisherOptions = Readonly<{
  authChallengeWaitMs?: number
  marketSecretKeyHex: string
  connect?: BuyModeRelayConnector
  publishTimeoutMs?: number
}>

const MarketSecretKeyHexPattern = /^[0-9a-f]{64}$/i
const DefaultAuthChallengeWaitMs = 1_500
const DefaultPublishTimeoutMs = 10_000

class BuyModeRelayConnectionError extends Error {
  constructor(reason: string) {
    super(reason)
    this.name = 'BuyModeRelayConnectionError'
  }
}

const hexToBytes = (hex: string): Uint8Array => {
  const bytes = new Uint8Array(hex.length / 2)

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16)
  }

  return bytes
}

export const workersFetchBuyModeRelayConnector: BuyModeRelayConnector =
  async relayUrl => {
    const httpUrl = relayUrl.replace(/^ws(s?):\/\//i, 'http$1://')
    const response = await fetch(httpUrl, {
      headers: { Upgrade: 'websocket' },
    })
    const socket = (response as { webSocket?: WebSocket | null }).webSocket

    if (socket === undefined || socket === null) {
      throw new BuyModeRelayConnectionError('relay refused websocket upgrade')
    }

    ;(socket as unknown as { accept: () => void }).accept()

    return socket as unknown as BuyModeRelaySocket
  }

const failureReceipt = async (
  input: BuyModeRelayJobRequest,
  reasonSlug: string,
): Promise<BuyModeRelayPublishReceipt> => ({
  accepted: false,
  relayRef: `relay.public.${reasonSlug}.${(
    await sha256Hex(input.relayUrl)
  ).slice(0, 32)}`,
  requestEventId: `event.unpublished.${(
    await sha256Hex(JSON.stringify(input.requestEvent))
  ).slice(0, 32)}`,
})

export const awaitBuyModeRelayOk = (
  socket: BuyModeRelaySocket,
  eventId: string,
  timeoutMs: number,
): Promise<Readonly<{ accepted: boolean; message: string }>> =>
  new Promise(resolve => {
    const timeout = setTimeout(() => {
      resolve({ accepted: false, message: 'timeout' })
    }, timeoutMs)
    const settle = (accepted: boolean, message: string) => {
      clearTimeout(timeout)
      resolve({ accepted, message })
    }

    socket.addEventListener('message', event => {
      try {
        const parsed = parseJsonUnknown(String(event.data))

        if (
          Array.isArray(parsed) &&
          parsed[0] === 'OK' &&
          parsed[1] === eventId
        ) {
          settle(
            parsed[2] === true,
            typeof parsed[3] === 'string' ? parsed[3] : '',
          )
        }
      } catch {
        settle(false, 'parse_error')
      }
    })
    socket.addEventListener('error', () => {
      settle(false, 'socket_error')
    })
    socket.addEventListener('close', () => {
      settle(false, 'socket_closed')
    })
  })

const relayRejectionSlug = (message: string): string => {
  const lower = message.toLowerCase()

  if (lower.includes('rate-limited')) return 'relay_publish_rate_limited'
  if (lower.includes('auth')) return 'relay_publish_auth_required'
  if (lower.includes('signature')) return 'relay_publish_signature_invalid'
  if (lower.includes('kind')) return 'relay_publish_kind_blocked'
  if (lower.includes('content exceeds')) return 'relay_publish_content_too_large'
  if (lower.includes('timeout')) return 'relay_publish_timeout'

  return 'relay_publish_rejected'
}

const awaitBuyModeRelayAuthChallenge = (
  socket: BuyModeRelaySocket,
  timeoutMs: number,
): Promise<string | undefined> =>
  new Promise(resolve => {
    const timeout = setTimeout(() => {
      resolve(undefined)
    }, timeoutMs)
    const settle = (challenge: string | undefined) => {
      clearTimeout(timeout)
      resolve(challenge)
    }

    socket.addEventListener('message', event => {
      try {
        const parsed = parseJsonUnknown(String(event.data))

        if (
          Array.isArray(parsed) &&
          parsed[0] === 'AUTH' &&
          typeof parsed[1] === 'string' &&
          parsed[1].trim() !== ''
        ) {
          settle(parsed[1])
        }
      } catch {
        settle(undefined)
      }
    })
    socket.addEventListener('error', () => {
      settle(undefined)
    })
    socket.addEventListener('close', () => {
      settle(undefined)
    })
  })

export const makeLiveBuyModeRelayPublisher = (
  options: LiveBuyModeRelayPublisherOptions,
): BuyModeRelayPublisher => {
  const connect = options.connect ?? workersFetchBuyModeRelayConnector
  const authChallengeWaitMs =
    options.authChallengeWaitMs ?? DefaultAuthChallengeWaitMs
  const publishTimeoutMs = options.publishTimeoutMs ?? DefaultPublishTimeoutMs

  return {
    publishJobRequest: async input => {
      if (!MarketSecretKeyHexPattern.test(options.marketSecretKeyHex)) {
        return failureReceipt(input, 'market_key_invalid')
      }

      const template = input.requestEvent as {
        content?: unknown
        created_at?: unknown
        kind?: unknown
        tags?: unknown
      }

      if (
        typeof template.content !== 'string' ||
        typeof template.created_at !== 'number' ||
        typeof template.kind !== 'number' ||
        !Array.isArray(template.tags)
      ) {
        return failureReceipt(input, 'request_template_invalid')
      }

      const event = finalizeEvent(
        {
          content: template.content,
          created_at: template.created_at,
          kind: template.kind,
          tags: template.tags.map(tag => [...(tag as ReadonlyArray<string>)]),
        },
        hexToBytes(options.marketSecretKeyHex.toLowerCase()),
      )

      let socket: BuyModeRelaySocket

      try {
        socket = await connect(input.relayUrl)
      } catch {
        return failureReceipt(input, 'relay_connect_failed')
      }

      try {
        const challenge = await awaitBuyModeRelayAuthChallenge(
          socket,
          authChallengeWaitMs,
        )

        if (challenge !== undefined) {
          const authEvent = finalizeEvent(
            {
              content: '',
              created_at: currentEpochSeconds(),
              kind: 22242,
              tags: [
                ['relay', input.relayUrl],
                ['challenge', challenge],
              ],
            },
            hexToBytes(options.marketSecretKeyHex.toLowerCase()),
          )
          const authOkPromise = awaitBuyModeRelayOk(
            socket,
            authEvent.id,
            publishTimeoutMs,
          )

          socket.send(JSON.stringify(['AUTH', authEvent]))

          const authVerdict = await authOkPromise

          if (!authVerdict.accepted) {
            return failureReceipt(input, relayRejectionSlug(authVerdict.message))
          }
        }

        const okPromise = awaitBuyModeRelayOk(socket, event.id, publishTimeoutMs)

        socket.send(JSON.stringify(['EVENT', event]))

        const verdict = await okPromise

        if (!verdict.accepted) {
          return failureReceipt(input, relayRejectionSlug(verdict.message))
        }

        return {
          accepted: true,
          relayRef: `relay.public.market.${(
            await sha256Hex(input.relayUrl)
          ).slice(0, 32)}`,
          requestEventId: event.id,
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

export const buyModeRelayPublisherForEnv = (
  environment: unknown,
): BuyModeRelayPublisher | undefined => {
  const env = environment as {
    BUY_MODE_MARKET_SECRET_KEY?: string
    FORUM_WORK_REQUEST_MARKET_SECRET_KEY?: string
  }
  const secret =
    typeof env.BUY_MODE_MARKET_SECRET_KEY === 'string' &&
    env.BUY_MODE_MARKET_SECRET_KEY.length > 0
      ? env.BUY_MODE_MARKET_SECRET_KEY
      : env.FORUM_WORK_REQUEST_MARKET_SECRET_KEY

  if (typeof secret !== 'string' || secret.length === 0) {
    return undefined
  }

  return makeLiveBuyModeRelayPublisher({ marketSecretKeyHex: secret })
}
