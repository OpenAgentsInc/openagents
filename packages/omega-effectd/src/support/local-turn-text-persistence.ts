import type { DesktopMessageMeta } from "./chat-contract.ts"
import {
  LOCAL_TURN_TEXT_LIMIT,
  type LocalTurnJournal,
  type LocalTurnKey,
  type LocalTurnRecord,
} from "./local-turn-journal.ts"
import type { makeThreadStore } from "./thread-store.ts"

type ThreadStore = ReturnType<typeof makeThreadStore>

export type LocalTurnTextPersistence = Readonly<{
  append: (text: string) => void
  boundary: () => LocalTurnRecord | null
  complete: (text: string) => LocalTurnRecord | null
  flush: () => LocalTurnRecord | null
  dispose: () => void
}>

/**
 * Coalesces provider deltas into one private atomic journal/thread-store write
 * per cadence window. The renderer still receives every live delta; this host
 * owns only the bounded durable checkpoint used after process loss.
 */
export const makeLocalTurnTextPersistence = (input: Readonly<{
  journal: LocalTurnJournal
  store: ThreadStore
  key: LocalTurnKey
  cadenceMs?: number
  meta: () => DesktopMessageMeta
  now?: () => Date
}>): LocalTurnTextPersistence => {
  const cadenceMs = Math.max(10, Math.min(input.cadenceMs ?? 50, 1_000))
  const now = input.now ?? (() => new Date())
  let pending = ""
  let timer: ReturnType<typeof setTimeout> | null = null
  let segmentIndex = 0
  let segmentKey = input.journal.get(input.key)?.assistantMessageKey ?? `${input.key.turnRef}-assistant`

  const project = (record: LocalTurnRecord | null): LocalTurnRecord | null => {
    if (record === null || record.assistantText === "") return record
    const segment = record.assistantSegments.find(value => value.key === segmentKey)
    if (segment === undefined) return record
    input.store.upsert(record.threadRef, {
      key: segment.key,
      role: "assistant",
      text: segment.text,
      timestamp: now().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      meta: input.meta(),
    })
    return record
  }
  const flush = (): LocalTurnRecord | null => {
    if (timer !== null) clearTimeout(timer)
    timer = null
    const text = pending
    pending = ""
    return text === "" ? input.journal.get(input.key) : project(input.journal.appendAssistantText(input.key, text, segmentKey))
  }

  return {
    append: text => {
      if (text === "") return
      pending = (pending + text).slice(0, LOCAL_TURN_TEXT_LIMIT)
      if (timer === null) timer = setTimeout(flush, cadenceMs)
    },
    boundary: () => {
      const record = flush()
      if (record?.assistantSegments.some(segment => segment.key === segmentKey) === true) {
        segmentKey = `${input.key.turnRef}-assistant-${++segmentIndex}`
      }
      return record
    },
    complete: text => {
      let record = flush()
      if (record !== null && record.assistantText === "" && text !== "") {
        pending = text
        record = flush()
      }
      return record
    },
    flush,
    dispose: () => { flush() },
  }
}
