import type { AccountStatusResult } from "./account-status"
import type { CodingStatusResult } from "./coding-status"
import type {
  DesktopKhalaDispatchPlanInput,
  DesktopKhalaDispatchPlanResult,
} from "./khala-dispatch"
import type { KhalaFleetSnapshotResult } from "./khala-fleet-manager"
import type {
  OnDeviceDeciderRequest,
  OnDeviceDeciderRunResult,
  OnDeviceDeciderStatus,
} from "./on-device-decider"
import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"
import type {
  AssignmentTokenUsageVerification,
  TokenAccountingReplayResult,
  TokenAccountingStatusResult,
} from "./token-accounting"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    codexAccountsStatus(): Promise<AccountStatusResult>
    codingStatus(): Promise<CodingStatusResult>
    createPylon(): Promise<CreatePylonResult>
    khalaDispatchPlan(
      input: DesktopKhalaDispatchPlanInput,
    ): Promise<DesktopKhalaDispatchPlanResult>
    khalaFleetSnapshot(): Promise<KhalaFleetSnapshotResult>
    onDeviceDecide(
      input: OnDeviceDeciderRequest,
    ): Promise<OnDeviceDeciderRunResult>
    onDeviceDeciderStatus(): Promise<OnDeviceDeciderStatus>
    pylonStatus(): Promise<PylonStatusResult>
    replayTokenFailures(): Promise<TokenAccountingReplayResult>
    tokenAccountingStatus(): Promise<TokenAccountingStatusResult>
    verifyAssignmentTokenUsage(
      assignmentRef: string,
    ): Promise<AssignmentTokenUsageVerification>
  }
}
