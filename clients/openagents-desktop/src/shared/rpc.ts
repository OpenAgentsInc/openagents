import type { CodingStatusResult } from "./coding-status"
import type {
  DesktopKhalaDispatchPlanInput,
  DesktopKhalaDispatchPlanResult,
} from "./khala-dispatch"
import type { KhalaFleetSnapshotResult } from "./khala-fleet-manager"
import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    codingStatus(): Promise<CodingStatusResult>
    createPylon(): Promise<CreatePylonResult>
    khalaDispatchPlan(
      input: DesktopKhalaDispatchPlanInput,
    ): Promise<DesktopKhalaDispatchPlanResult>
    khalaFleetSnapshot(): Promise<KhalaFleetSnapshotResult>
    pylonStatus(): Promise<PylonStatusResult>
  }
}
