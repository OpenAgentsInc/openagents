import type { CodingStatusResult } from "./coding-status"
import type { KhalaFleetSnapshotResult } from "./khala-fleet-manager"
import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    codingStatus(): Promise<CodingStatusResult>
    createPylon(): Promise<CreatePylonResult>
    khalaFleetSnapshot(): Promise<KhalaFleetSnapshotResult>
    pylonStatus(): Promise<PylonStatusResult>
  }
}
