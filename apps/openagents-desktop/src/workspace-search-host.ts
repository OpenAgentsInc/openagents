import { Worker } from "node:worker_threads"

import {
  decodeWorkspaceSearchPage,
  type DesktopWorkspaceSearchPage,
} from "./workspace-contract.ts"

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
  dispose: () => void
}>

type WorkerInput = Readonly<{
  root: string
  grantRef: string
  request: WorkspaceSearchRequest
}>

type ActiveSearch = Readonly<{
  worker: Worker
  settle: (page: DesktopWorkspaceSearchPage, terminate: boolean) => void
}>

const unavailable = (message: string): DesktopWorkspaceSearchPage => ({
  state: "unavailable",
  message,
})

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
): WorkspaceSearchHost => {
  let disposed = false
  let sequence = 0
  const active = new Map<number, ActiveSearch>()

  const settledTask = (page: DesktopWorkspaceSearchPage): WorkspaceSearchTask => {
    const taskRef = `workspace.search.task.${++sequence}`
    return { taskRef, result: Promise.resolve(page), cancel: () => undefined }
  }

  const start = (request: WorkspaceSearchRequest): WorkspaceSearchTask => {
    if (disposed) return settledTask(unavailable("The selected workspace has been disposed."))
    const id = ++sequence
    const taskRef = `workspace.search.task.${id}`
    let worker: Worker
    try {
      worker = makeWorker(workerUrl, { root, grantRef, request })
    } catch {
      return { taskRef, result: Promise.resolve(unavailable("Workspace search could not start.")), cancel: () => undefined }
    }

    let settled = false
    let resolveResult!: (page: DesktopWorkspaceSearchPage) => void
    const result = new Promise<DesktopWorkspaceSearchPage>(resolve => {
      resolveResult = resolve
    })
    const settle = (page: DesktopWorkspaceSearchPage, terminate: boolean): void => {
      if (settled) return
      settled = true
      active.delete(id)
      resolveResult(page)
      if (terminate) void worker.terminate()
    }
    active.set(id, { worker, settle })
    worker.on("message", (message: Readonly<{ ok?: unknown; result?: unknown }>) => {
      const page = message.ok === true ? decodeWorkspaceSearchPage(message.result) : null
      settle(page ?? unavailable("Workspace search did not return a valid result."), false)
    })
    worker.on("error", () => settle(unavailable("Workspace search stopped unexpectedly."), false))
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

  return {
    start,
    cancelAll,
    activeCount: () => active.size,
    dispose: () => {
      if (disposed) return
      disposed = true
      for (const task of [...active.values()]) {
        task.settle(unavailable("The selected workspace has been disposed."), true)
      }
    },
  }
}
