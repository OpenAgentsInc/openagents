import { Worker } from "node:worker_threads"

export type CodexHistoryRequest =
  | Readonly<{ kind: "list"; sessionsRoot: string; limit?: number }>
  | Readonly<{ kind: "detail"; sessionsRoot: string; id: string; messageLimit?: number }>
  | Readonly<{ kind: "history_catalog"; sessionsRoot: string; claudeRoot?: string | null }>
  | Readonly<{ kind: "history_page"; sessionsRoot: string; threadRef: string; offset?: number; limit?: number; claudeRoot?: string | null }>
  | Readonly<{ kind: "history_search"; sessionsRoot: string; claudeRoot?: string | null; query: string; limit?: number }>

export type CodexHistoryHost = Readonly<{
  run: (request: CodexHistoryRequest) => Promise<unknown>
  dispose: () => void
}>

/** Process-owned worker perimeter for all local Codex rollout reads. */
export const makeCodexHistoryHost = (
  workerUrl: URL,
  makeWorker: (url: URL) => Worker = url => new Worker(url),
): CodexHistoryHost => {
  let disposed = false
  let worker: Worker | null = null
  let requestId = 0
  const pending = new Map<number, (value: unknown) => void>()

  const settlePending = (): void => {
    for (const settle of pending.values()) settle(null)
    pending.clear()
  }

  const openWorker = (): Worker => {
    if (worker !== null) return worker
    const opened = makeWorker(workerUrl)
    worker = opened
    opened.on("message", (message: { id?: unknown; ok?: unknown; result?: unknown }) => {
      if (typeof message.id !== "number") return
      const settle = pending.get(message.id)
      if (settle === undefined) return
      pending.delete(message.id)
      settle(message.ok === true ? message.result : null)
    })
    opened.on("error", () => {
      if (worker === opened) worker = null
      settlePending()
    })
    return opened
  }

  return {
    run: request => disposed ? Promise.resolve(null) : new Promise(resolve => {
      const id = ++requestId
      pending.set(id, resolve)
      openWorker().postMessage({ id, request })
    }),
    dispose: () => {
      if (disposed) return
      disposed = true
      const active = worker
      worker = null
      settlePending()
      active?.terminate()
    },
  }
}
