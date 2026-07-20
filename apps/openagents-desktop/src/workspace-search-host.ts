import { Worker } from "node:worker_threads"

import {
  decodeWorkspaceSearchPage,
  type DesktopWorkspaceSearchPage,
} from "./workspace-contract.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./ide/portable-mutation-authority.ts"

export type WorkspaceSearchRequest = Readonly<{
  query: string
  mode: "path" | "content"
  offset?: number
  limit?: number
  epoch: number
}>

export type WorkspaceSearchTask = Readonly<{
  taskRef: string
  result: Promise<DesktopWorkspaceSearchPage>
  cancel: () => void
}>

export type WorkspaceSearchHost = Readonly<{
  start: (request: WorkspaceSearchRequest) => WorkspaceSearchTask
  cancelAll: () => void
  activeCount: () => number
  quiesce: () => Promise<WorkspaceSearchQuiesceResult>
  dispose: () => Promise<WorkspaceSearchQuiesceResult>
}>

export type WorkspaceSearchQuiesceResult =
  | Readonly<{ state: "quiesced" }>
  | Readonly<{ state: "timed_out"; pendingTaskRefs: ReadonlyArray<string> }>

export type WorkspaceSearchHostOptions = Readonly<{
  mutationAuthority?: IdePortableMutationAuthority
  monitorAuthority?: (check: () => void) => Readonly<{ close: () => void }>
  quiesceTimeoutMs?: number
}>

type WorkerInput = Readonly<{
  root: string
  grantRef: string
  request: WorkspaceSearchRequest
}>

type ActiveSearch = Readonly<{
  taskRef: string
  worker: Worker
  settle: (page: DesktopWorkspaceSearchPage, terminate: boolean) => void
}>

const unavailable = (message: string): DesktopWorkspaceSearchPage => ({
  state: "unavailable",
  message,
})

const defaultAuthorityMonitor = (check: () => void): Readonly<{ close: () => void }> => {
  const timer = setInterval(check, 25)
  timer.unref()
  return { close: () => clearInterval(timer) }
}

/**
 * WorkContext-owned perimeter for bounded filesystem searches. Each search has
 * its own worker so cancellation interrupts synchronous traversal instead of
 * merely ignoring a late result on Electron main.
 */
export const makeWorkspaceSearchHost = (
  root: string,
  grantRef: string,
  workerUrl: URL,
  makeWorker: (url: URL, input: WorkerInput) => Worker =
    (url, input) => new Worker(url, { workerData: input }),
  options: WorkspaceSearchHostOptions = {},
): WorkspaceSearchHost => {
  let quiesced = false
  let sequence = 0
  let quiescePromise: Promise<WorkspaceSearchQuiesceResult> | null = null
  const active = new Map<number, ActiveSearch>()
  const terminating = new Map<string, Promise<void>>()
  const timeoutMs = Math.max(10, Math.min(options.quiesceTimeoutMs ?? 5_000, 30_000))

  const capturePermit = (): IdePortableMutationPermit | null => {
    const authority = options.mutationAuthority
    if (authority === undefined) return null
    const authorization = authority.authorize(grantRef)
    return authorization._tag === "Permitted" ? authorization.permit : null
  }
  const permitIsCurrent = (permit: IdePortableMutationPermit | null): boolean =>
    options.mutationAuthority === undefined || (
      permit !== null && options.mutationAuthority.reauthorize(permit)
    )

  const terminateWorker = (taskRef: string, worker: Worker): Promise<void> => {
    const current = terminating.get(taskRef)
    if (current !== undefined) return current
    const termination = worker.terminate()
      .then(() => undefined, () => undefined)
      .finally(() => {
        if (terminating.get(taskRef) === termination) terminating.delete(taskRef)
      })
    terminating.set(taskRef, termination)
    return termination
  }

  const settledTask = (page: DesktopWorkspaceSearchPage): WorkspaceSearchTask => {
    const taskRef = `workspace.search.task.${++sequence}`
    return { taskRef, result: Promise.resolve(page), cancel: () => undefined }
  }

  const start = (request: WorkspaceSearchRequest): WorkspaceSearchTask => {
    if (quiesced) return settledTask(unavailable("Workspace search is quiesced on this host."))
    const id = ++sequence
    const taskRef = `workspace.search.task.${id}`
    const permit = capturePermit()
    if (!permitIsCurrent(permit)) {
      return { taskRef, result: Promise.resolve(unavailable("The current workspace placement does not permit this search.")), cancel: () => undefined }
    }
    let worker: Worker
    try {
      worker = makeWorker(workerUrl, { root, grantRef, request })
    } catch {
      return { taskRef, result: Promise.resolve(unavailable("Workspace search could not start.")), cancel: () => undefined }
    }

    let settled = false
    let monitor: Readonly<{ close: () => void }> | null = null
    let resolveResult!: (page: DesktopWorkspaceSearchPage) => void
    const result = new Promise<DesktopWorkspaceSearchPage>(resolve => {
      resolveResult = resolve
    })
    const settle = (page: DesktopWorkspaceSearchPage, terminate: boolean): void => {
      if (settled) return
      settled = true
      active.delete(id)
      monitor?.close()
      monitor = null
      resolveResult(page)
      if (terminate) void terminateWorker(taskRef, worker)
    }
    active.set(id, { taskRef, worker, settle })
    if (!permitIsCurrent(permit)) {
      settle(unavailable("The workspace search authority changed during worker startup."), true)
    } else if (permit !== null) {
      monitor = (options.monitorAuthority ?? defaultAuthorityMonitor)(() => {
        if (!permitIsCurrent(permit)) {
          settle(unavailable("The workspace search authority was revoked."), true)
        }
      })
    }
    worker.on("message", (message: Readonly<{ ok?: unknown; result?: unknown }>) => {
      if (!permitIsCurrent(permit)) {
        settle(unavailable("The workspace search authority was revoked before its result was admitted."), true)
        return
      }
      const page = message.ok === true ? decodeWorkspaceSearchPage(message.result) : null
      const pageMatchesTask = page !== null && (
        page.state !== "available" || (
          page.grantRef === grantRef &&
          page.query === request.query &&
          page.mode === request.mode &&
          page.cache.epoch === request.epoch
        )
      )
      settle(pageMatchesTask ? page : unavailable("Workspace search did not return a valid result."), true)
    })
    worker.on("error", () => settle(unavailable("Workspace search stopped unexpectedly."), true))
    worker.on("exit", () => settle(unavailable("Workspace search stopped before completing."), false))

    return {
      taskRef,
      result,
      cancel: () => settle(unavailable("Workspace search was cancelled."), true),
    }
  }

  const cancelAll = (): void => {
    for (const task of [...active.values()]) {
      task.settle(unavailable("Workspace search was cancelled."), true)
    }
  }

  const quiesce = (): Promise<WorkspaceSearchQuiesceResult> => {
    quiesced = true
    if (quiescePromise !== null) return quiescePromise
    const pending = [...active.values()]
    for (const task of pending) {
      task.settle(unavailable("Workspace search was quiesced."), true)
    }
    quiescePromise = (async () => {
      const safePoint = Promise.all([...terminating.values()]).then(() => "quiesced" as const)
      let timer: ReturnType<typeof setTimeout> | null = null
      const timeout = new Promise<"timed_out">(resolve => {
        timer = setTimeout(() => resolve("timed_out"), timeoutMs)
      })
      const state = await Promise.race([safePoint, timeout])
      if (timer !== null) clearTimeout(timer)
      return state === "quiesced"
        ? { state }
        : { state, pendingTaskRefs: [...terminating.keys()].sort() }
    })()
    return quiescePromise
  }

  return {
    start,
    cancelAll,
    activeCount: () => active.size,
    quiesce,
    dispose: quiesce,
  }
}
