import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { setTimeout as sleep } from "node:timers/promises"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { decodeBundledServerRequestResponse } from "@openagentsinc/codex-app-server-protocol/decode"
import {
  denyCodexReverseRpc,
  makeCodexReverseRpcArbiter,
} from "./codex-reverse-rpc-arbiter.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const request = (method: string, id: string | number = `request-${method}`) => ({
  id,
  method,
  params: { threadId: "thread-1", turnId: "turn-1", itemId: `item-${method}` },
})

const methods = [
  "item/commandExecution/requestApproval",
  "item/fileChange/requestApproval",
  "item/tool/requestUserInput",
  "mcpServer/elicitation/request",
  "item/permissions/requestApproval",
  "item/tool/call",
  "account/chatgptAuthTokens/refresh",
  "attestation/generate",
  "currentTime/read",
  "applyPatchApproval",
  "execCommandApproval",
] as const

describe("Codex reverse RPC arbiter", () => {
  test("provides generated method-correct deny/success/error behavior for all 11 methods", async () => {
    const arbiter = makeCodexReverseRpcArbiter()
    for (const method of methods) {
      const operation = arbiter.arbitrate({
        connectionKey: "connection-1",
        generation: 1,
        request: request(method),
        proposers: [],
      })
      if (method === "account/chatgptAuthTokens/refresh" || method === "attestation/generate") {
        await expect(operation, method).rejects.toMatchObject({ code: -32_003, reason: "authority_unavailable" })
      } else {
        const response = await operation
        expect(decodeBundledServerRequestResponse(method, response), method).toMatchObject({ _tag: "Decoded" })
      }
    }
    expect(arbiter.receipts()).toHaveLength(11)
    arbiter.close()
  })

  test("commits the first valid proposal across subscribers and marks the loser visible", async () => {
    const attention: string[] = []
    const arbiter = makeCodexReverseRpcArbiter({ timeoutMs: 1_000 })
    arbiter.subscribe(event => attention.push(event.state))
    const first = arbiter.arbitrate({
      connectionKey: "connection-race",
      generation: 1,
      request: request("item/commandExecution/requestApproval", "race-1"),
      proposers: [async () => { await sleep(20); return { decision: "acceptForSession" } }],
    })
    const second = arbiter.arbitrate({
      connectionKey: "connection-race",
      generation: 1,
      request: request("item/commandExecution/requestApproval", "race-1"),
      proposers: [async () => ({ decision: "accept" })],
    })
    await expect(Promise.all([first, second])).resolves.toEqual([
      { decision: "accept" },
      { decision: "accept" },
    ])
    await sleep(30)
    expect(arbiter.receipts()).toEqual([expect.objectContaining({
      outcome: "accepted",
      lateProposals: 1,
    })])
    expect(attention).toContain("late-proposal")
    arbiter.close()
  })

  test("reopens receipts and turns replayed pending requests into deny-only no-ops", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-reverse-rpc-"))
    roots.push(root)
    const journalPath = join(root, "receipts.json")
    const first = makeCodexReverseRpcArbiter({ journalPath })
    await expect(first.arbitrate({
      connectionKey: "connection-replay",
      generation: 1,
      request: request("item/fileChange/requestApproval", "replay-1"),
      proposers: [async () => ({ decision: "accept" })],
    })).resolves.toEqual({ decision: "accept" })
    first.close()

    let proposed = false
    const reopened = makeCodexReverseRpcArbiter({ journalPath })
    await expect(reopened.arbitrate({
      connectionKey: "connection-replay",
      generation: 2,
      request: request("item/fileChange/requestApproval", "replay-1"),
      proposers: [async () => { proposed = true; return { decision: "accept" } }],
    })).resolves.toEqual({ decision: "decline" })
    expect(proposed).toBe(false)
    expect(reopened.receipts()).toEqual([expect.objectContaining({ outcome: "replay_noop" })])
    reopened.close()
  })

  test("times out and shutdown-cancels unresolved proposals without orphaned attention", async () => {
    const timed = makeCodexReverseRpcArbiter({ timeoutMs: 5 })
    await expect(timed.arbitrate({
      connectionKey: "connection-timeout",
      generation: 1,
      request: request("item/tool/requestUserInput", "timeout-1"),
      proposers: [async () => new Promise(() => undefined)],
    })).resolves.toEqual({ answers: {} })
    expect(timed.receipts()).toEqual([expect.objectContaining({ outcome: "timeout" })])
    timed.close()

    const cancelled = makeCodexReverseRpcArbiter({ timeoutMs: 60_000 })
    const pending = cancelled.arbitrate({
      connectionKey: "connection-cancel",
      generation: 1,
      request: request("item/tool/call", "cancel-1"),
      proposers: [async () => new Promise(() => undefined)],
    })
    expect(cancelled.pending()).toHaveLength(1)
    cancelled.close()
    await expect(pending).resolves.toEqual({ contentItems: [], success: false })
    expect(cancelled.pending()).toHaveLength(0)
    expect(cancelled.receipts()).toEqual([expect.objectContaining({ outcome: "cancelled" })])
  })

  test("never journals credential, token, attestation, or proposal payloads", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-reverse-secret-"))
    roots.push(root)
    const journalPath = join(root, "receipts.json")
    const arbiter = makeCodexReverseRpcArbiter({ journalPath })
    await expect(arbiter.arbitrate({
      connectionKey: "private-connection",
      generation: 1,
      request: request("account/chatgptAuthTokens/refresh", "token-refresh-1"),
      proposers: [async () => ({
        accessToken: "super-secret-access-token",
        chatgptAccountId: "private-account",
      })],
    })).resolves.toEqual({
      accessToken: "super-secret-access-token",
      chatgptAccountId: "private-account",
    })
    const disk = readFileSync(journalPath, "utf8")
    expect(disk).not.toContain("super-secret-access-token")
    expect(disk).not.toContain("private-account")
    expect(disk).not.toContain("token-refresh-1")
    arbiter.close()
  })

  test("exports only generated-valid safe denials", () => {
    for (const method of methods.filter(method => method !== "account/chatgptAuthTokens/refresh" && method !== "attestation/generate")) {
      expect(decodeBundledServerRequestResponse(method, denyCodexReverseRpc(method)), method)
        .toMatchObject({ _tag: "Decoded" })
    }
  })
})
