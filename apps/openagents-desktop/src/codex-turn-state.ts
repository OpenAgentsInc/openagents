import { createHash, randomUUID } from "node:crypto"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"
import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"
import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"

type Row = Readonly<Record<string, unknown>>
const object = (value: unknown): Row | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as Row : null
const string = (value: unknown): string | null => typeof value === "string" ? value : null

/** Generated programmatic effect classes: no handwritten notification inventory. */
export const codexTurnEffectByMethod = Object.fromEntries(
  bundledCodex01441ProtocolManifest.members
    .filter(member => member.direction === "server-notification")
    .map(member => [member.method,
      member.method === "turn/started" ? "turn_started"
        : member.method === "turn/completed" ? "turn_terminal"
          : member.method === "item/started" ? "item_started"
            : member.method === "item/completed" ? "item_terminal"
              : member.method.startsWith("item/") ? "item_delta"
                : member.method.startsWith("turn/") ? "turn_observation"
                  : member.method.startsWith("thread/") ? "thread_observation"
                    : "observation"]),
) as Readonly<Record<string, string>>

export type CodexNativeItemState = Readonly<{ id: string; type: string; status: string; payload: unknown; deltas: ReadonlyArray<Readonly<{ method: string; payload: unknown }>> }>
export type CodexTurnStateSnapshot = Readonly<{
  revision: number
  threadId: string | null
  activeTurnId: string | null
  activeKind: "regular" | "review" | null
  terminalTurnIds: ReadonlyArray<string>
  interrupt: Readonly<{ turnId: string; admittedAt: string }> | null
  items: Readonly<Record<string, CodexNativeItemState>>
  observations: ReadonlyArray<Readonly<{ method: string; payload: unknown }>>
}>
export type CodexTurnAdmissionReceipt = Readonly<{ intentHash: string; method: "turn/steer"; threadIdHash: string; expectedTurnIdHash: string; clientUserMessageIdHash: string; outcome: "accepted" | "rejected"; observedAt: string }>

export type CodexTurnState = Readonly<{
  snapshot: () => CodexTurnStateSnapshot
  apply: (notification: CodexAppServerNotification) => CodexTurnStateSnapshot
  bindStartedTurn: (threadId: string, turnId: string, kind?: "regular" | "review") => void
  admitInterrupt: (threadId: string, turnId: string) => boolean
  authorizeSteer: (threadId: string, expectedTurnId: string, clientUserMessageId?: string) => Readonly<{ accepted: boolean; clientUserMessageId: string }>
  settleSteer: (threadId: string, expectedTurnId: string, clientUserMessageId: string, accepted: boolean) => void
  startReview: (lease: CodexAppServerLease, input: Readonly<{ threadId: string; delivery: "inline" | "detached"; target: unknown }>) => Promise<Readonly<{ turnId: string | null; reviewThreadId: string | null }>>
  receipts: () => ReadonlyArray<CodexTurnAdmissionReceipt>
}>

const initial = (): CodexTurnStateSnapshot => ({ revision: 0, threadId: null, activeTurnId: null, activeKind: null, terminalTurnIds: [], interrupt: null, items: {}, observations: [] })
const readReceipts = (path: string | undefined): CodexTurnAdmissionReceipt[] => {
  if (path === undefined) return []
  try { const parsed = JSON.parse(readFileSync(path, "utf8")); return Array.isArray(parsed.receipts) ? parsed.receipts : [] } catch { return [] }
}

export const makeCodexTurnState = (options: Readonly<{ receiptPath?: string; now?: () => Date }> = {}): CodexTurnState => {
  let state = initial()
  const receipts = readReceipts(options.receiptPath)
  const now = () => options.now?.() ?? new Date()
  const publish = (patch: Partial<CodexTurnStateSnapshot>) => { state = { ...state, ...patch, revision: state.revision + 1 }; return state }
  const persist = () => {
    if (options.receiptPath === undefined) return
    mkdirSync(dirname(options.receiptPath), { recursive: true, mode: 0o700 })
    const temporary = `${options.receiptPath}.tmp`
    writeFileSync(temporary, `${JSON.stringify({ schema: "openagents.desktop.codex_turn_admission_receipts.v1", receipts: receipts.slice(-2_048) }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, options.receiptPath)
  }
  const hash = (value: string) => createHash("sha256").update(value).digest("hex")
  const settleSteer = (threadId: string, expectedTurnId: string, clientUserMessageId: string, accepted: boolean) => {
    receipts.push({ intentHash: hash(`${threadId}\0${expectedTurnId}\0${clientUserMessageId}`), method: "turn/steer", threadIdHash: hash(threadId), expectedTurnIdHash: hash(expectedTurnId), clientUserMessageIdHash: hash(clientUserMessageId), outcome: accepted ? "accepted" : "rejected", observedAt: now().toISOString() })
    persist()
  }
  return {
    snapshot: () => state,
    bindStartedTurn: (threadId, turnId, kind = "regular") => { if (!state.terminalTurnIds.includes(turnId)) publish({ threadId, activeTurnId: turnId, activeKind: kind, interrupt: null }) },
    apply: notification => {
      const method = string(notification.message.method) ?? "unknown"
      const params = object(notification.message.params) ?? {}
      const threadId = string(params.threadId) ?? string(object(params.thread)?.id) ?? state.threadId
      const turn = object(params.turn)
      const turnId = string(params.turnId) ?? string(turn?.id)
      const effect = codexTurnEffectByMethod[method] ?? "observation"
      if (effect === "turn_started" && threadId !== null && turnId !== null) {
        if (!state.terminalTurnIds.includes(turnId)) publish({ threadId, activeTurnId: turnId, activeKind: state.activeKind ?? "regular", interrupt: null })
        return state
      }
      if (effect === "turn_terminal" && turnId !== null) {
        const terminal = [...new Set([...state.terminalTurnIds, turnId])]
        publish({ terminalTurnIds: terminal, ...(state.activeTurnId === turnId ? { activeTurnId: null, activeKind: null, interrupt: null } : {}) })
        return state
      }
      const item = object(params.item)
      const itemId = string(params.itemId) ?? string(item?.id)
      if (itemId !== null && (effect === "item_started" || effect === "item_terminal")) {
        const previous = state.items[itemId]
        const next: CodexNativeItemState = { id: itemId, type: string(item?.type) ?? previous?.type ?? "unknown", status: effect === "item_terminal" ? string(item?.status) ?? "completed" : string(item?.status) ?? "started", payload: item ?? params, deltas: previous?.deltas ?? [] }
        publish({ items: { ...state.items, [itemId]: next } }); return state
      }
      if (itemId !== null && effect === "item_delta") {
        const previous = state.items[itemId] ?? { id: itemId, type: method.split("/")[1] ?? "unknown", status: "started", payload: null, deltas: [] }
        publish({ items: { ...state.items, [itemId]: { ...previous, deltas: [...previous.deltas, { method, payload: params }] } } }); return state
      }
      publish({ observations: [...state.observations, { method, payload: params }].slice(-2_048) })
      return state
    },
    admitInterrupt: (threadId, turnId) => {
      if (state.threadId !== threadId || state.activeTurnId !== turnId) return false
      publish({ interrupt: { turnId, admittedAt: now().toISOString() } })
      return true
    },
    authorizeSteer: (threadId, expectedTurnId, proposedId) => {
      const clientUserMessageId = proposedId ?? `steer.${randomUUID()}`
      return { accepted: state.threadId === threadId && state.activeTurnId === expectedTurnId && state.activeKind === "regular", clientUserMessageId }
    },
    settleSteer,
    startReview: async (lease, input) => {
      const response = object(await lease.request("review/start", input))
      const turnId = string(object(response?.turn)?.id)
      const reviewThreadId = string(response?.reviewThreadId) ?? (input.delivery === "inline" ? input.threadId : null)
      if (turnId !== null && reviewThreadId !== null) publish({ threadId: reviewThreadId, activeTurnId: turnId, activeKind: "review", interrupt: null })
      return { turnId, reviewThreadId }
    },
    receipts: () => [...receipts],
  }
}
