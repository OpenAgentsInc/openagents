import type { CodexLocalRuntime } from "./codex-local-runtime.ts"
import type { DesktopThread } from "./chat-contract.ts"
import type { CodexHistoryCatalog, CodexHistorySearchResponse } from "./codex-history-contract.ts"
import type { CodexModel } from "./fable-local-contract.ts"
import type { LocalTurnJournal, LocalTurnKey, LocalTurnRecord } from "./local-turn-journal.ts"
import type { makeThreadStore } from "./thread-store.ts"

type ThreadStore = ReturnType<typeof makeThreadStore>

/**
 * Translate a provider-native Codex history ref back to the Desktop-local
 * thread that owns it. Sidebar history is keyed by provider session ids,
 * while the mutable composer/store is keyed by Desktop thread ids.
 */
export const localThreadRefForProviderSession = (
  records: ReadonlyArray<LocalTurnRecord>,
  providerSessionRef: string,
): string | null => {
  const matches = records.filter(record =>
    record.lane === "codex-local" && record.providerSessionRef === providerSessionRef)
  return [...matches].sort((left, right) =>
    right.updatedAt.localeCompare(left.updatedAt) || right.turnRef.localeCompare(left.turnRef))[0]?.threadRef ?? null
}

/**
 * Provider sessions already represented by a currently retained Desktop-local
 * thread are one conversation identity, not a second history conversation.
 * Keep the mapping in main: neither provider refs nor the private turn journal
 * need to enter renderer state merely to deduplicate the sidebar.
 */
export const providerSessionRefsForLocalThreads = (
  records: ReadonlyArray<LocalTurnRecord>,
  localThreadRefs: ReadonlySet<string>,
): ReadonlySet<string> => new Set(records.flatMap(record =>
  record.lane === "codex-local" &&
  record.providerSessionRef !== null &&
  localThreadRefs.has(record.threadRef)
    ? [record.providerSessionRef]
    : [],
))

export const filterLocallyOwnedCodexHistoryCatalog = (
  catalog: CodexHistoryCatalog,
  records: ReadonlyArray<LocalTurnRecord>,
  localThreadRefs: ReadonlySet<string>,
): CodexHistoryCatalog => {
  const locallyOwnedProviderRefs = providerSessionRefsForLocalThreads(records, localThreadRefs)
  return locallyOwnedProviderRefs.size === 0
    ? catalog
    : { ...catalog, roots: catalog.roots.filter(root => !locallyOwnedProviderRefs.has(root.threadRef)) }
}

export const filterLocallyOwnedCodexHistorySearch = (
  response: CodexHistorySearchResponse,
  records: ReadonlyArray<LocalTurnRecord>,
  localThreadRefs: ReadonlySet<string>,
): CodexHistorySearchResponse => {
  const locallyOwnedProviderRefs = providerSessionRefsForLocalThreads(records, localThreadRefs)
  return locallyOwnedProviderRefs.size === 0
    ? response
    : { ...response, results: response.results.filter(result => !locallyOwnedProviderRefs.has(result.rootThreadRef)) }
}

const isRecoverableCodexModel = (model: string | null): model is CodexModel =>
  model === "gpt-5.5" || model === "gpt-5.6-sol"

export type LocalTurnRecoveryOutcome = Readonly<{
  key: LocalTurnKey
  state: "completed" | "interrupted"
}>

const timestamp = (): string =>
  new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })

const recoveryMeta = (
  state: "recovering" | "interrupted" | "completed",
  generation: number,
  disposition?: "resumed_after_restart" | "interrupted_by_restart",
) => ({
  recovery: {
    state,
    generation,
    ...(disposition === undefined ? {} : { disposition }),
  },
})

const upsertAssistant = (
  store: ThreadStore,
  record: LocalTurnRecord,
  state: "recovering" | "interrupted" | "completed",
  disposition?: "resumed_after_restart" | "interrupted_by_restart",
): void => {
  for (const segment of record.assistantSegments) {
    store.upsert(record.threadRef, {
      key: segment.key,
      role: "assistant",
      text: segment.text,
      timestamp: timestamp(),
      meta: {
        lane: record.lane,
        turnRef: record.turnRef,
        ...(record.model === null ? {} : { model: record.model }),
        ...(record.accountRef === null ? {} : { accountRef: record.accountRef }),
        ...recoveryMeta(state, record.recoveryGeneration, disposition),
      },
    })
  }
}

const upsertRecoveryNotice = (
  store: ThreadStore,
  record: LocalTurnRecord,
  state: "recovering" | "interrupted" | "completed",
  text: string,
  disposition?: "resumed_after_restart" | "interrupted_by_restart",
): void => {
  store.upsert(record.threadRef, {
    key: `${record.turnRef}-recovery`,
    role: "system",
    text,
    timestamp: timestamp(),
    meta: {
      lane: record.lane,
      turnRef: record.turnRef,
      ...recoveryMeta(state, record.recoveryGeneration, disposition),
    },
  })
}

const mergeText = (prefix: string, finalText: string): string => {
  if (prefix === "") return finalText
  if (finalText === "" || prefix.endsWith(finalText)) return prefix
  if (finalText.startsWith(prefix)) return finalText
  const max = Math.min(prefix.length, finalText.length)
  for (let length = max; length > 0; length -= 1) {
    if (prefix.endsWith(finalText.slice(0, length))) return prefix + finalText.slice(length)
  }
  return `${prefix}\n\n${finalText}`
}

const interrupt = (
  journal: LocalTurnJournal,
  store: ThreadStore,
  record: LocalTurnRecord,
): LocalTurnRecoveryOutcome => {
  const terminal = journal.terminal(record, "interrupted_by_restart", "interrupted_by_restart") ?? record
  upsertAssistant(store, terminal, "interrupted", "interrupted_by_restart")
  upsertRecoveryNotice(
    store,
    terminal,
    "interrupted",
    "Turn interrupted by an application restart. Retry explicitly to continue.",
    "interrupted_by_restart",
  )
  return { key: record, state: "interrupted" }
}

export const reconcileLocalTurns = async (input: Readonly<{
  journal: LocalTurnJournal
  store: ThreadStore
  codex: Pick<CodexLocalRuntime, "runTurn">
  /** Healthy production path: app-server durable state decides recovery; no synthetic continuation prompt. */
  codexState?: (threadId: string) => Promise<"completed" | "running" | "interrupted" | "unknown">
  onThread?: (thread: DesktopThread) => void
}>): Promise<ReadonlyArray<LocalTurnRecoveryOutcome>> => {
  const publish = (threadRef: string): void => {
    const thread = input.store.open(threadRef)
    if (thread !== null) input.onThread?.(thread)
  }
  const outcomes: LocalTurnRecoveryOutcome[] = []
  for (const candidate of input.journal.nonterminal()) {
    const key: LocalTurnKey = candidate
    // Provider lane SPI (L1 #8899): only the codex-local lane owns durable
    // provider-session replay; every other lane — fable-local and any SPI
    // lane that never declared replay — fails CLOSED to an honest
    // interrupted_by_restart disposition instead of guessing a resume path.
    if (candidate.lane !== "codex-local" || candidate.accountRef === null ||
      candidate.providerSessionRef === null || !isRecoverableCodexModel(candidate.model)) {
      outcomes.push(interrupt(input.journal, input.store, candidate))
      publish(candidate.threadRef)
      continue
    }
    const claimed = input.journal.beginRecovery(key)
    if (claimed === null || claimed.phase !== "recovering") {
      outcomes.push(interrupt(input.journal, input.store, claimed ?? candidate))
      publish(candidate.threadRef)
      continue
    }
    if (claimed.accountRef === null || claimed.providerSessionRef === null ||
      !isRecoverableCodexModel(claimed.model)) {
      outcomes.push(interrupt(input.journal, input.store, claimed))
      publish(claimed.threadRef)
      continue
    }
    const accountRef = claimed.accountRef
    const providerSessionRef = claimed.providerSessionRef
    const model = claimed.model
    const recoverySegmentKey = `${claimed.turnRef}-assistant-${claimed.assistantSegments.length}`
    upsertAssistant(input.store, claimed, "recovering")
    upsertRecoveryNotice(
      input.store,
      claimed,
      "recovering",
      "Recovering the interrupted Codex turn on its recorded account and thread…",
    )
    publish(claimed.threadRef)
    if (input.codexState !== undefined) {
      const nativeState = await input.codexState(providerSessionRef).catch(() => "unknown" as const)
      if (nativeState === "completed") {
        const completed = input.journal.terminal(key, "completed", "resumed_after_restart") ?? claimed
        upsertAssistant(input.store, completed, "completed", "resumed_after_restart")
        upsertRecoveryNotice(input.store, completed, "completed", "Codex app-server confirmed the turn completed before restart.", "resumed_after_restart")
        publish(completed.threadRef)
        outcomes.push({ key, state: "completed" })
      } else {
        outcomes.push(interrupt(input.journal, input.store, claimed))
        upsertRecoveryNotice(input.store, claimed, "interrupted", nativeState === "running"
          ? "Codex app-server restored durable thread state, but transient output has no replay cursor. Retry explicitly."
          : "Codex app-server did not confirm completion. Retry explicitly; no continuation was fabricated.", "interrupted_by_restart")
        publish(claimed.threadRef)
      }
      continue
    }
    const result = await input.codex.runTurn({
      turnRef: claimed.turnRef,
      threadRef: claimed.threadRef,
      history: [],
      message: "Continue the response interrupted by the Desktop host restart. Do not repeat text already produced.",
      accountRef,
      model,
      recovery: { threadId: providerSessionRef, accountRef },
      emit: event => {
        if (event.kind !== "text_delta") return
        const current = input.journal.appendAssistantText(key, event.text, recoverySegmentKey)
        if (current !== null) {
          upsertAssistant(input.store, current, "recovering")
          publish(current.threadRef)
        }
      },
    })
    if (!result.ok) {
      outcomes.push(interrupt(input.journal, input.store, claimed))
      publish(claimed.threadRef)
      continue
    }
    const current = input.journal.get(key) ?? claimed
    const merged = mergeText(current.assistantText, result.text)
    if (merged !== current.assistantText) {
      input.journal.appendAssistantText(key, merged.slice(current.assistantText.length), recoverySegmentKey)
    }
    const completed = input.journal.terminal(key, "completed", "resumed_after_restart") ?? current
    upsertAssistant(input.store, completed, "completed", "resumed_after_restart")
    upsertRecoveryNotice(
      input.store,
      completed,
      "completed",
      "Recovered after application restart.",
      "resumed_after_restart",
    )
    publish(completed.threadRef)
    outcomes.push({ key, state: "completed" })
  }
  return outcomes
}
