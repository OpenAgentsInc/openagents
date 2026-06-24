/**
 * Live buy-mode eval bridge.
 *
 * Waits on the scoped market relay for the provider's NIP-90 result event and
 * returns a private settlement payload to the operator route. The route is
 * responsible for paying and recording the result through `settleBuyModeResult`.
 */
import type { BuyModeEvalBridge } from './operator-buy-mode-routes'
import { parseJsonUnknown } from './json-boundary'
import {
  type BuyModeRelayConnector,
  workersFetchBuyModeRelayConnector,
} from './buy-mode-live-publisher'

type LiveBuyModeEvalBridgeOptions = Readonly<{
  connect?: BuyModeRelayConnector
  timeoutMs?: number
}>

type RelayMessage = ReadonlyArray<unknown>

const DefaultTimeoutMs = 300_000

const resultFromEvent = (
  event: Readonly<{
    content?: unknown
    id?: unknown
    kind?: unknown
    pubkey?: unknown
    tags?: unknown
  }>,
): Awaited<ReturnType<BuyModeEvalBridge['dispatchEval']>>['settlement'] | undefined => {
  if (
    typeof event.id !== 'string' ||
    typeof event.content !== 'string' ||
    event.kind !== 6050 ||
    !Array.isArray(event.tags)
  ) {
    return undefined
  }

  const amountTag = event.tags.find(
    tag => Array.isArray(tag) && tag[0] === 'amount',
  ) as ReadonlyArray<unknown> | undefined
  const amountMsats = Number(amountTag?.[1])
  const bolt11 = amountTag?.[2]
  const providerPubkey = event.pubkey

  if (
    !Number.isInteger(amountMsats) ||
    amountMsats <= 0 ||
    typeof bolt11 !== 'string' ||
    !bolt11.startsWith('lnbc') ||
    typeof providerPubkey !== 'string' ||
    providerPubkey.length !== 64
  ) {
    return undefined
  }

  return {
    amountMsats,
    bolt11,
    content: event.content,
    providerPubkey,
    resultEventId: event.id,
  }
}

export const makeLiveBuyModeEvalBridge = (
  options: LiveBuyModeEvalBridgeOptions = {},
): BuyModeEvalBridge => {
  const connect = options.connect ?? workersFetchBuyModeRelayConnector
  const timeoutMs = options.timeoutMs ?? DefaultTimeoutMs

  return {
    dispatchEval: async input => {
      const socket = await connect(input.relayUrl)
      try {
        const settlement = await new Promise<
          Awaited<ReturnType<BuyModeEvalBridge['dispatchEval']>>['settlement']
        >((resolve, reject) => {
          const timer = setTimeout(() => {
            reject(new Error('buy-mode eval result timed out'))
          }, timeoutMs)
          const settle = (
            value: Awaited<
              ReturnType<BuyModeEvalBridge['dispatchEval']>
            >['settlement'],
          ) => {
            clearTimeout(timer)
            resolve(value)
          }

          socket.addEventListener('message', event => {
            const parsed = parseJsonUnknown(String(event.data)) as RelayMessage
            if (parsed[0] !== 'EVENT' || parsed[1] !== 'buy-mode-eval') {
              return
            }
            const eventBody = parsed[2]
            if (typeof eventBody !== 'object' || eventBody === null) {
              return
            }
            const settlement = resultFromEvent(eventBody)
            if (
              settlement !== undefined &&
              settlement.amountMsats === input.job.amountMsats
            ) {
              settle(settlement)
            }
          })
          socket.addEventListener('error', () => {
            reject(new Error('buy-mode eval relay error'))
          })
          socket.addEventListener('close', () => {
            reject(new Error('buy-mode eval relay closed'))
          })
          socket.send(
            JSON.stringify([
              'REQ',
              'buy-mode-eval',
              { '#e': [input.requestEventId], kinds: [6050, 7000] },
            ]),
          )
        })

        return {
          settledMsats: input.job.amountMsats,
          ...(settlement === undefined ? {} : { settlement }),
          verdict: {
            class: 'exact_trace_replay',
            passed: settlement !== undefined,
          },
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

export const buyModeEvalBridgeForEnv = (
  environment: unknown,
): BuyModeEvalBridge | undefined => {
  const env = environment as {
    BUY_MODE_EVAL_BRIDGE?: string
    BUY_MODE_EVAL_TIMEOUT_MS?: string
  }

  if (env.BUY_MODE_EVAL_BRIDGE !== 'relay') {
    return undefined
  }

  const timeoutMs = Number(env.BUY_MODE_EVAL_TIMEOUT_MS ?? DefaultTimeoutMs)

  return makeLiveBuyModeEvalBridge({
    timeoutMs: Number.isInteger(timeoutMs) && timeoutMs > 0
      ? timeoutMs
      : DefaultTimeoutMs,
  })
}
