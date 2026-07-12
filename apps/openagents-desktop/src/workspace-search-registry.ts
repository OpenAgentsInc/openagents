import type {
  DesktopWorkspaceSearchBridgeRequest,
  DesktopWorkspaceSearchCancelResult,
  DesktopWorkspaceSearchResponse,
} from "./workspace-contract.ts"
import type { DesktopWorkspaceService } from "./workspace-service.ts"
import type { WorkspaceSearchTask } from "./workspace-search-host.ts"

type ActiveRequest = Readonly<{
  requestRef: string
  task: WorkspaceSearchTask
}>

export type WorkspaceSearchRegistry = Readonly<{
  start: (ownerRef: string, request: DesktopWorkspaceSearchBridgeRequest) => Promise<DesktopWorkspaceSearchResponse>
  cancel: (ownerRef: string, requestRef: string) => DesktopWorkspaceSearchCancelResult
  closeOwner: (ownerRef: string) => void
  activeCount: () => number
  dispose: () => void
}>

const unavailable = (requestRef: string, message: string): DesktopWorkspaceSearchResponse => ({
  requestRef,
  page: { state: "unavailable", message },
})

/** Binds each in-flight search to one exact renderer webContents owner. */
export const makeWorkspaceSearchRegistry = (
  currentWorkspace: () => DesktopWorkspaceService | null,
): WorkspaceSearchRegistry => {
  let disposed = false
  const active = new Map<string, ActiveRequest>()

  const closeOwner = (ownerRef: string): void => {
    const current = active.get(ownerRef)
    if (current === undefined) return
    active.delete(ownerRef)
    current.task.cancel()
  }

  return {
    start: async (ownerRef, request) => {
      if (disposed) return unavailable(request.requestRef, "Workspace search is unavailable.")
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
      const owned = { requestRef: request.requestRef, task }
      active.set(ownerRef, owned)
      const page = await task.result
      if (active.get(ownerRef) === owned) active.delete(ownerRef)
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
    dispose: () => {
      if (disposed) return
      disposed = true
      for (const ownerRef of [...active.keys()]) closeOwner(ownerRef)
    },
  }
}
