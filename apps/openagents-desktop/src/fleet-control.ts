/**
 * Main-process-only local Pylon dispatch adapter. It borrows the useful shape
 * of the retired Khala Code control service without importing that app: the
 * control token stays on disk/in the Electron host, the renderer supplies one
 * bounded objective, and only a public-safe outcome returns.
 */
import { existsSync, readFileSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"

import {
  type FleetStageRequest,
  type FleetStageResult,
  unavailableFleetStageResult,
} from "./fleet-contract.ts"

const controlBaseUrl = (): string =>
  (process.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716").replace(/\/+$/u, "")

const pylonHomes = (): ReadonlyArray<string> =>
  [...new Set([
    process.env.PYLON_HOME?.trim(),
    join(homedir(), ".openagents", "pylon"),
    join(homedir(), ".pylon"),
  ].filter((value): value is string => typeof value === "string" && value.length > 0))]

const readControlToken = (): string | null => {
  for (const home of pylonHomes()) {
    const path = join(home, "control-token")
    if (!existsSync(path)) continue
    const token = readFileSync(path, "utf8").trim()
    if (token.length > 0) return token
  }
  return null
}

type CommandResponse = Readonly<{
  ok?: unknown
  result?: Readonly<{ status?: unknown }>
}>

export type SubmitFleetBriefDependencies = Readonly<{
  fetch?: typeof fetch
  readToken?: () => string | null
  baseUrl?: string
}>

export const submitFleetBrief = async (
  request: FleetStageRequest,
  dependencies: SubmitFleetBriefDependencies = {},
): Promise<FleetStageResult> => {
  const token = (dependencies.readToken ?? readControlToken)()
  if (token === null) return unavailableFleetStageResult()

  try {
    const response = await (dependencies.fetch ?? fetch)(
      `${dependencies.baseUrl ?? controlBaseUrl()}/command`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          type: "intent.submit",
          title: "Desktop fleet deployment brief",
          body: request.objective,
          submittedByClientRef: "openagents-desktop",
        }),
      },
    )
    if (!response.ok) {
      return {
        state: response.status === 401 || response.status === 403 ? "rejected" : "unavailable",
        message: response.status === 401 || response.status === 403
          ? "Local Pylon rejected the request. No fleet work was dispatched."
          : "Local Pylon is unavailable. No fleet work was dispatched.",
        intentStatus: null,
      }
    }
    const body = await response.json() as CommandResponse
    if (body.ok !== true) {
      return {
        state: "rejected",
        message: "Local Pylon declined the fleet brief. No FleetRun receipt was returned.",
        intentStatus: null,
      }
    }
    const intentStatus = typeof body.result?.status === "string"
      ? body.result.status.slice(0, 120)
      : "received"
    return {
      state: "accepted",
      message: "Local Pylon accepted the fleet brief. Watch for an authority-backed FleetRun receipt before treating work as deployed.",
      intentStatus,
    }
  } catch {
    return unavailableFleetStageResult()
  }
}
