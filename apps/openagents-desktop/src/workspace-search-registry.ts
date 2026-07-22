import type {
  DesktopWorkspaceSearchBridgeRequest,
  DesktopWorkspaceSearchCancelResult,
  DesktopWorkspaceSearchResponse,
} from "./workspace-contract.ts"
import type { DesktopWorkspaceService } from "./workspace-service.ts"
import type {
  WorkspaceSearchQuiesceResult,
  WorkspaceSearchTask,
} from "./workspace-search-host.ts"

type ActiveRequest = Readonly<{
  requestRef: string
  workspace: DesktopWorkspaceService
  grantRef: string
  task: WorkspaceSearchTask
}>

export type WorkspaceSearchRegistry = Readonly<{
  start: (ownerRef: string, request: DesktopWorkspaceSearchBridgeRequest) => Promise<DesktopWorkspaceSearchResponse>
  cancel: (ownerRef: string, requestRef: string) => DesktopWorkspaceSearchCancelResult
  closeOwner: (ownerRef: string) => void
  activeCount: () => number
  quiesce: () => Promise<WorkspaceSearchQuiesceResult>
  dispose: () => Promise<WorkspaceSearchQuiesceResult>
}>

const unavailable = (requestRef: string, message: string): DesktopWorkspaceSearchResponse => ({
  requestRef,
  page: { state: "unavailable", message },
})

/** Binds each in-flight search to one exact renderer webContents owner. */
export const makeWorkspaceSearchRegistry = (
  currentWorkspace: () => DesktopWorkspaceService | null,
  quiesceTimeoutMs = 5_000,
): WorkspaceSearchRegistry => {
  let quiesced = false
  let quiescePromise: Promise<WorkspaceSearchQuiesceResult> | null = null
  const active = new Map<string, ActiveRequest>()

  const closeOwner = (ownerRef: string): void => {
    const current = active.get(ownerRef)
    if (current === undefined) return
    active.delete(ownerRef)
    current.task.cancel()
  }

  const quiesce = (): Promise<WorkspaceSearchQuiesceResult> => {
    quiesced = true
    if (quiescePromise !== null) return quiescePromise
    const pending = [...active.values()]
    for (const ownerRef of [...active.keys()]) closeOwner(ownerRef)
    quiescePromise = (async () => {
      const awaiting = new Set(pending.map(request => request.task.taskRef))
      const safePoint = Promise.all(pending.map(async request => {
        try {
          await request.task.result
        } finally {
          awaiting.delete(request.task.taskRef)
        }
      })).then(() => "quiesced" as const)
      let timer: ReturnType<typeof setTimeout> | null = null
      const timeout = new Promise<"timed_out">(resolve => {
        timer = setTimeout(
          () => resolve("timed_out"),
          Math.max(10, Math.min(quiesceTimeoutMs, 30_000)),
        )
      })
      const state = await Promise.race([safePoint, timeout])
      if (timer !== null) clearTimeout(timer)
      return state === "quiesced"
        ? { state }
        : { state, pendingTaskRefs: [...awaiting].sort() }
    })()
    return quiescePromise
  }

  return {
    start: async (ownerRef, request) => {
      if (quiesced) return unavailable(request.requestRef, "Workspace search is quiesced on this host.")
      closeOwner(ownerRef)
      const workspace = currentWorkspace()
      if (workspace === null) {
        return unavailable(request.requestRef, "Choose a workspace folder before searching.")
      }
      const task = workspace.search({
        query: request.query,
        mode: request.mode,
        ...(request.offset === undefined ? {} : { offset: request.offset }),
        ...(request.limit === undefined ? {} : { limit: request.limit }),
      })
      const owned = { requestRef: request.requestRef, workspace, grantRef: workspace.grantRef, task }
      active.set(ownerRef, owned)
      const page = await task.result
      const stillOwned = active.get(ownerRef) === owned
      if (stillOwned) active.delete(ownerRef)
      const current = currentWorkspace()
      if (
        page.state === "available" && (
          !stillOwned ||
          current !== owned.workspace ||
          current?.grantRef !== owned.grantRef
        )
      ) {
        return unavailable(request.requestRef, "The workspace changed before the search result was admitted.")
      }
      return { requestRef: request.requestRef, page }
    },
    cancel: (ownerRef, requestRef) => {
      const current = active.get(ownerRef)
      if (current === undefined || current.requestRef !== requestRef) {
        return { requestRef, cancelled: false }
      }
      active.delete(ownerRef)
      current.task.cancel()
      return { requestRef, cancelled: true }
    },
    closeOwner,
    activeCount: () => active.size,
    quiesce,
    dispose: quiesce,
  }
}
