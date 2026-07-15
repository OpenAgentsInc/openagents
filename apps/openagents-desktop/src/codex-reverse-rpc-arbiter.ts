import { createHash } from "node:crypto"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"

import { decodeBundledServerRequestResponse } from "@openagentsinc/codex-app-server-protocol/decode"
import type { CodexAppServerRequest } from "./codex-app-server-client.ts"

export const CODEX_REVERSE_RPC_JOURNAL_SCHEMA = "openagents.desktop.codex_reverse_rpc.v1" as const

export type CodexReverseRpcOutcome = "accepted" | "denied" | "error" | "timeout" | "cancelled" | "replay_noop"

export type CodexReverseRpcReceipt = Readonly<{
  key: string
  method: string
  requestIdHash: string
  state: "committed"
  outcome: CodexReverseRpcOutcome
  decidedAt: string
  lateProposals: number
}>

export type CodexReverseRpcAttention = Readonly<{
  key: string
  method: string
  state: "pending" | "committed" | "late-proposal"
  deadlineAt: string
}>

export class CodexReverseRpcError extends Error {
  readonly _tag = "CodexReverseRpcError"
  override readonly name = "CodexReverseRpcError"
  constructor(
    readonly code: number,
    readonly reason: "authority_unavailable" | "invalid_proposal",
    message: string,
  ) { super(message) }
}

type Proposer = (request: CodexAppServerRequest) => Promise<unknown> | unknown
type Pending = {
  key: string
  method: string
  request: CodexAppServerRequest
  deadlineAt: string
  promise: Promise<unknown>
  settle: (value: unknown, outcome: CodexReverseRpcOutcome) => void
  fail: (error: Error, outcome: CodexReverseRpcOutcome) => void
  add: (proposers: ReadonlyArray<Proposer>) => void
  timer: ReturnType<typeof setTimeout>
  settled: boolean
}

export type CodexReverseRpcArbiter = Readonly<{
  arbitrate: (input: Readonly<{
    connectionKey: string
    generation: number
    request: CodexAppServerRequest
    proposers: ReadonlyArray<Proposer>
  }>) => Promise<unknown>
  receipts: () => ReadonlyArray<CodexReverseRpcReceipt>
  pending: () => ReadonlyArray<CodexReverseRpcAttention>
  subscribe: (listener: (attention: CodexReverseRpcAttention) => void) => () => void
  close: () => void
}>

const object = (value: unknown): Readonly<Record<string, unknown>> | null =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Readonly<Record<string, unknown>>
    : null

const stableRequestKey = (input: Readonly<{
  connectionKey: string
  generation: number
  request: CodexAppServerRequest
}>): string => {
  const params = object(input.request.params)
  const semantic = ["threadId", "turnId", "itemId", "callId", "requestId"]
    .flatMap(name => typeof params?.[name] === "string" ? [[name, params[name]]] : [])
  const durable = typeof input.request.id === "string" || semantic.length > 0
  return createHash("sha256").update(JSON.stringify([
    input.connectionKey,
    durable ? null : input.generation,
    input.request.method,
    input.request.id,
    semantic,
  ])).digest("hex")
}

export const denyCodexReverseRpc = (method: string): unknown => {
  switch (method) {
    case "item/commandExecution/requestApproval":
    case "item/fileChange/requestApproval":
      return { decision: "decline" }
    case "applyPatchApproval":
    case "execCommandApproval":
      return { decision: "denied" }
    case "item/tool/requestUserInput":
      return { answers: {} }
    case "mcpServer/elicitation/request":
      return { action: "decline", content: null, _meta: null }
    case "item/permissions/requestApproval":
      return { permissions: {} }
    case "item/tool/call":
      return { contentItems: [], success: false }
    case "account/chatgptAuthTokens/refresh":
    case "attestation/generate":
      throw new CodexReverseRpcError(-32_003, "authority_unavailable", `${method} authority is unavailable`)
    case "currentTime/read":
      return { currentTimeAt: Math.floor(Date.now() / 1_000) }
    default:
      throw new CodexReverseRpcError(-32_601, "invalid_proposal", `Unsupported reverse RPC: ${method}`)
  }
}

const readReceipts = (journalPath: string | undefined): CodexReverseRpcReceipt[] => {
  if (journalPath === undefined) return []
  try {
    const parsed = JSON.parse(readFileSync(journalPath, "utf8")) as {
      schema?: unknown
      receipts?: unknown
    }
    return parsed.schema === CODEX_REVERSE_RPC_JOURNAL_SCHEMA && Array.isArray(parsed.receipts)
      ? parsed.receipts.filter(receipt => receipt !== null && typeof receipt === "object") as CodexReverseRpcReceipt[]
      : []
  } catch {
    return []
  }
}

export const makeCodexReverseRpcArbiter = (options: Readonly<{
  journalPath?: string
  timeoutMs?: number
  maxReceipts?: number
  now?: () => Date
}> = {}): CodexReverseRpcArbiter => {
  const timeoutMs = Math.max(1, Math.floor(options.timeoutMs ?? 120_000))
  const maxReceipts = Math.max(1, Math.floor(options.maxReceipts ?? 4_096))
  const receiptMap = new Map(readReceipts(options.journalPath).slice(-maxReceipts).map(receipt => [receipt.key, receipt]))
  const operations = new Map<string, Pending>()
  const listeners = new Set<(attention: CodexReverseRpcAttention) => void>()
  let closed = false

  const publish = (attention: CodexReverseRpcAttention): void => {
    for (const listener of listeners) {
      try { listener(attention) } catch { /* isolate attention observers */ }
    }
  }
  const persist = (): void => {
    if (options.journalPath === undefined) return
    mkdirSync(dirname(options.journalPath), { recursive: true })
    const temporary = `${options.journalPath}.tmp`
    writeFileSync(temporary, `${JSON.stringify({
      schema: CODEX_REVERSE_RPC_JOURNAL_SCHEMA,
      receipts: [...receiptMap.values()].slice(-maxReceipts),
    }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, options.journalPath)
  }
  const record = (pending: Pending, outcome: CodexReverseRpcOutcome): void => {
    const receipt: CodexReverseRpcReceipt = {
      key: pending.key,
      method: pending.method,
      requestIdHash: createHash("sha256").update(String(pending.request.id)).digest("hex"),
      state: "committed",
      outcome,
      decidedAt: (options.now?.() ?? new Date()).toISOString(),
      lateProposals: receiptMap.get(pending.key)?.lateProposals ?? 0,
    }
    receiptMap.delete(pending.key)
    receiptMap.set(pending.key, receipt)
    while (receiptMap.size > maxReceipts) receiptMap.delete(receiptMap.keys().next().value!)
    persist()
    publish({ key: pending.key, method: pending.method, state: "committed", deadlineAt: pending.deadlineAt })
  }
  const late = (pending: Pending): void => {
    const existing = receiptMap.get(pending.key)
    if (existing !== undefined) {
      receiptMap.set(pending.key, { ...existing, lateProposals: existing.lateProposals + 1 })
      persist()
    }
    publish({ key: pending.key, method: pending.method, state: "late-proposal", deadlineAt: pending.deadlineAt })
  }

  const arbitrate: CodexReverseRpcArbiter["arbitrate"] = async input => {
    if (closed) return denyCodexReverseRpc(input.request.method)
    const key = stableRequestKey(input)
    const previous = receiptMap.get(key)
    if (previous !== undefined) {
      const replay: CodexReverseRpcReceipt = {
        ...previous,
        outcome: "replay_noop",
        lateProposals: previous.lateProposals + input.proposers.length,
      }
      receiptMap.set(key, replay)
      persist()
      publish({ key, method: input.request.method, state: "late-proposal", deadlineAt: previous.decidedAt })
      return denyCodexReverseRpc(input.request.method)
    }
    const existing = operations.get(key)
    if (existing !== undefined) {
      existing.add(input.proposers)
      return existing.promise
    }

    const deadlineAt = new Date((options.now?.() ?? new Date()).getTime() + timeoutMs).toISOString()
    let resolvePromise!: (value: unknown) => void
    let rejectPromise!: (error: Error) => void
    const promise = new Promise<unknown>((resolve, reject) => {
      resolvePromise = resolve
      rejectPromise = reject
    })
    let remaining = 0
    const pending = {} as Pending
    const commit = (value: unknown, outcome: CodexReverseRpcOutcome): void => {
      if (pending.settled) { late(pending); return }
      pending.settled = true
      clearTimeout(pending.timer)
      operations.delete(key)
      record(pending, outcome)
      resolvePromise(value)
    }
    const fail = (error: Error, outcome: CodexReverseRpcOutcome): void => {
      if (pending.settled) { late(pending); return }
      pending.settled = true
      clearTimeout(pending.timer)
      operations.delete(key)
      record(pending, outcome)
      rejectPromise(error)
    }
    const add = (proposers: ReadonlyArray<Proposer>): void => {
      if (pending.settled) { for (const _ of proposers) late(pending); return }
      remaining += proposers.length
      for (const proposer of proposers) {
        void Promise.resolve().then(() => proposer(input.request)).then(value => {
          const decoded = decodeBundledServerRequestResponse(input.request.method, value)
          if (decoded._tag === "Decoded") commit(decoded.payload, "accepted")
          else if (--remaining === 0 && !pending.settled) {
            try { commit(denyCodexReverseRpc(input.request.method), "denied") } catch (error) {
              fail(error instanceof Error ? error : new Error("reverse RPC authority unavailable"), "error")
            }
          }
        }, () => {
          if (--remaining === 0 && !pending.settled) {
            try { commit(denyCodexReverseRpc(input.request.method), "denied") } catch (error) {
              fail(error instanceof Error ? error : new Error("reverse RPC authority unavailable"), "error")
            }
          }
        })
      }
    }
    Object.assign(pending, {
      key,
      method: input.request.method,
      request: input.request,
      deadlineAt,
      promise,
      settle: commit,
      fail,
      add,
      settled: false,
      timer: setTimeout(() => {
        try { commit(denyCodexReverseRpc(input.request.method), "timeout") } catch (error) {
          fail(error instanceof Error ? error : new Error("reverse RPC authority unavailable"), "error")
        }
      }, timeoutMs),
    } satisfies Pending)
    operations.set(key, pending)
    publish({ key, method: input.request.method, state: "pending", deadlineAt })

    const centralMethod = input.request.method === "currentTime/read" ||
      input.request.method === "item/permissions/requestApproval" ||
      input.request.method === "mcpServer/elicitation/request" ||
      input.request.method === "account/chatgptAuthTokens/refresh" ||
      input.request.method === "attestation/generate"
    const centrallyResolved = centralMethod && input.proposers.length === 0
    if (centrallyResolved || input.proposers.length === 0) {
      try {
        const fallback = denyCodexReverseRpc(input.request.method)
        const decoded = decodeBundledServerRequestResponse(input.request.method, fallback)
        if (decoded._tag === "DecodeFailure") throw new CodexReverseRpcError(-32_602, "invalid_proposal", decoded.detail)
        commit(decoded.payload, input.request.method === "currentTime/read" ? "accepted" : "denied")
      } catch (error) {
        fail(error instanceof Error ? error : new Error("reverse RPC authority unavailable"), "error")
      }
    } else add(input.proposers)
    return promise
  }

  return {
    arbitrate,
    receipts: () => [...receiptMap.values()],
    pending: () => [...operations.values()].map(operation => ({
      key: operation.key,
      method: operation.method,
      state: "pending",
      deadlineAt: operation.deadlineAt,
    })),
    subscribe: listener => {
      if (closed) throw new Error("Codex reverse RPC arbiter is closed")
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    close: () => {
      if (closed) return
      closed = true
      for (const pending of [...operations.values()]) {
        try { pending.settle(denyCodexReverseRpc(pending.method), "cancelled") } catch (error) {
          pending.fail(error instanceof Error ? error : new Error("reverse RPC authority unavailable"), "error")
        }
      }
      listeners.clear()
    },
  }
}
