/** Runs one bounded workspace search outside Electron's main process. */
import { parentPort, workerData } from "node:worker_threads"

import {
  decodeWorkspaceSearchRequest,
  type DesktopWorkspaceSearchPage,
} from "./workspace-contract.ts"
import { searchWorkspace } from "./workspace-service.ts"

type WorkerInput = Readonly<{
  root?: unknown
  grantRef?: unknown
  request?: unknown
}>

const fail = (): void => {
  parentPort?.postMessage({ ok: false, result: null })
}

try {
  const input = workerData as WorkerInput
  const request = decodeWorkspaceSearchRequest(input.request)
  const rawEpoch = (input.request as { epoch?: unknown } | null)?.epoch
  const epoch = typeof rawEpoch === "number" && Number.isFinite(rawEpoch)
    ? Math.max(0, Math.trunc(rawEpoch))
    : null
  if (typeof input.root !== "string" || typeof input.grantRef !== "string" || request === null || epoch === null) {
    fail()
  } else {
    const result: DesktopWorkspaceSearchPage = searchWorkspace({
      root: input.root,
      grantRef: input.grantRef,
      ...request,
      epoch,
    })
    parentPort?.postMessage({ ok: true, result })
  }
} catch {
  fail()
}
