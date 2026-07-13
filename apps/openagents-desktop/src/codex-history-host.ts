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

export type CodexHistoryProcess = Readonly<{
  postMessage: (value: unknown) => void
  onMessage: (listener: (value: unknown) => void) => void
  onExit: (listener: () => void) => void
  terminate: () => void
}>

/** Process-owned utility-process perimeter for all local Codex rollout reads. */
export const makeCodexHistoryHost = (
  makeProcess: () => CodexHistoryProcess,
): CodexHistoryHost => {
  let disposed = false
  let process: CodexHistoryProcess | null = null
  let requestId = 0
  const pending = new Map<number, (value: unknown) => void>()

  const settlePending = (): void => {
    for (const settle of pending.values()) settle(null)
    pending.clear()
  }

  const openProcess = (): CodexHistoryProcess => {
    if (process !== null) return process
    const opened = makeProcess()
    process = opened
    opened.onMessage(value => {
      const message = value as { id?: unknown; ok?: unknown; result?: unknown }
      if (typeof message.id !== "number") return
      const settle = pending.get(message.id)
      if (settle === undefined) return
      pending.delete(message.id)
      settle(message.ok === true ? message.result : null)
    })
    opened.onExit(() => {
      if (process === opened) process = null
      settlePending()
    })
    return opened
  }

  return {
    run: request => disposed ? Promise.resolve(null) : new Promise(resolve => {
      const id = ++requestId
      pending.set(id, resolve)
      openProcess().postMessage({ id, request })
    }),
    dispose: () => {
      if (disposed) return
      disposed = true
      const active = process
      process = null
      settlePending()
      active?.terminate()
    },
  }
}
