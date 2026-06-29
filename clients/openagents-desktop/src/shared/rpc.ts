import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    createPylon(): Promise<CreatePylonResult>
    pylonStatus(): Promise<PylonStatusResult>
  }
}
