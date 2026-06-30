import { EventEmitter } from "node:events"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  consumeKhalaCodexRateLimitResetCredit,
  fetchKhalaCodexRateLimitStatus,
} from "../src/bun/codex-rate-limits.js"

type FakeChild = EventEmitter & {
  readonly stdout: EventEmitter
  readonly stderr: EventEmitter
  readonly stdin: { readonly write: (line: string) => void }
  readonly kill: () => void
}

const fixedNow = () => new Date("2026-06-29T19:00:00.000Z")

const makeRpcChild = (
  onMessage: (message: { id?: number; method?: string }, child: FakeChild) => void,
): FakeChild => {
  const child = new EventEmitter() as FakeChild
  Object.assign(child, {
    stdout: new EventEmitter(),
    stderr: new EventEmitter(),
    stdin: {
      write: (line: string) => {
        const message = JSON.parse(line) as { id?: number; method?: string }
        queueMicrotask(() => onMessage(message, child))
      },
    },
    kill: () => undefined,
  })
  return child
}

describe("Khala Codex rate limits", () => {
  test("does not spawn Codex when auth.json is missing", async () => {
    let spawned = false
    const status = await fetchKhalaCodexRateLimitStatus({
      authExists: () => false,
      now: fixedNow,
      spawnFn: () => {
        spawned = true
        throw new Error("should not spawn")
      },
    })

    expect(spawned).toBe(false)
    expect(status).toMatchObject({
      provider: "codex",
      session: null,
      weekly: null,
      status: "unavailable",
      error: "Codex not signed in",
      updatedAtIso: "2026-06-29T19:00:00.000Z",
    })
  })

  test("reads 5h and weekly windows from the Codex app-server RPC", async () => {
    const child = makeRpcChild((message, rpcChild) => {
      if (message.method === "initialize") {
        rpcChild.stdout.emit("data", Buffer.from(JSON.stringify({
          jsonrpc: "2.0",
          id: message.id,
          result: {},
        }) + "\n"))
      }
      if (message.method === "account/rateLimits/read") {
        rpcChild.stdout.emit("data", Buffer.from(JSON.stringify({
          jsonrpc: "2.0",
          id: message.id,
          result: {
            rateLimits: {
              primary: { usedPercent: 17, windowDurationMins: 299, resetsAt: 1782788400 },
              secondary: { usedPercent: 42, windowDurationMins: 10079 },
            },
            rateLimitResetCredits: {
              availableCount: 1,
              credits: [
                {
                  status: "available",
                  expiresAt: "1782874800",
                  grantedAt: "1782262800000",
                },
              ],
            },
          },
        }) + "\n"))
      }
    })

    const status = await fetchKhalaCodexRateLimitStatus({
      authExists: () => true,
      now: fixedNow,
      spawnFn: () => child,
    })

    expect(status.status).toBe("ok")
    expect(status.session).toMatchObject({
      usedPercent: 17,
      remainingPercent: 83,
      windowMinutes: 300,
      resetsAtIso: "2026-06-30T03:00:00.000Z",
    })
    expect(status.weekly).toMatchObject({
      usedPercent: 42,
      remainingPercent: 58,
      windowMinutes: 10080,
    })
    expect(status.rateLimitResetCredits).toMatchObject({
      availableCount: 1,
      nextExpiresAtIso: "2026-07-01T03:00:00.000Z",
      credits: [
        {
          status: "available",
          expiresAtIso: "2026-07-01T03:00:00.000Z",
          grantedAtIso: "2026-06-24T01:00:00.000Z",
        },
      ],
    })
  })

  test("fills reset-credit metadata from the backend when the RPC omits it", async () => {
    const child = makeRpcChild((message, rpcChild) => {
      if (message.method === "initialize") {
        rpcChild.stdout.emit("data", Buffer.from(JSON.stringify({
          jsonrpc: "2.0",
          id: message.id,
          result: {},
        }) + "\n"))
      }
      if (message.method === "account/rateLimits/read") {
        rpcChild.stdout.emit("data", Buffer.from(JSON.stringify({
          jsonrpc: "2.0",
          id: message.id,
          result: {
            rateLimits: {
              primary: { usedPercent: 3 },
              secondary: { usedPercent: 4 },
            },
          },
        }) + "\n"))
      }
    })
    const calls: unknown[][] = []
    const fetchFn = ((...args: unknown[]) => {
      calls.push(args)
      return Promise.resolve({
        ok: true,
        json: async () => ({
          available_count: 2,
          total_earned_count: 3,
          credits: [
            {
              status: "available",
              expires_at: "2026-06-25T12:00:00Z",
              granted_at: "2026-06-18T12:00:00Z",
            },
            {
              status: "redeemed",
              expires_at: "2026-06-24T12:00:00Z",
              granted_at: "2026-06-17T12:00:00Z",
            },
          ],
        }),
      } as Response)
    }) as typeof fetch

    const status = await fetchKhalaCodexRateLimitStatus({
      authExists: () => true,
      codexHomePath: "/managed/codex-home",
      fetchFn,
      now: fixedNow,
      readFileFn: async path => {
        expect(path).toBe(join("/managed/codex-home", "auth.json"))
        return JSON.stringify({
          tokens: {
            access_token: "access-token",
            account_id: "account-id",
          },
        })
      },
      spawnFn: () => child,
    })

    expect(calls[0][0]).toBe("https://chatgpt.com/backend-api/wham/rate-limit-reset-credits")
    expect(calls[0][1]).toMatchObject({
      headers: {
        Authorization: "Bearer access-token",
        "ChatGPT-Account-Id": "account-id",
        "OpenAI-Beta": "codex-1",
      },
    })
    expect(status.rateLimitResetCredits).toMatchObject({
      availableCount: 2,
      totalEarnedCount: 3,
      nextExpiresAtIso: "2026-06-25T12:00:00.000Z",
    })
  })

  test("consumes a provider reset credit with a stable idempotency key", async () => {
    const calls: unknown[][] = []
    const outcome = await consumeKhalaCodexRateLimitResetCredit({
      codexHomePath: "/managed/codex-home",
      idempotencyKey: "redeem-123",
      readFileFn: async () => JSON.stringify({
        tokens: {
          access_token: "access-token",
          account_id: "account-id",
        },
      }),
      fetchFn: ((...args: unknown[]) => {
        calls.push(args)
        return Promise.resolve({
          ok: true,
          json: async () => ({ code: "nothing_to_reset" }),
        } as Response)
      }) as typeof fetch,
    })

    expect(outcome).toBe("nothingToReset")
    expect(calls[0][0]).toBe(
      "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume",
    )
    expect(calls[0][1]).toMatchObject({
      method: "POST",
      headers: {
        Authorization: "Bearer access-token",
        "ChatGPT-Account-Id": "account-id",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ redeem_request_id: "redeem-123" }),
    })
  })
})
