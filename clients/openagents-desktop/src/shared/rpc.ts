import type { CodingStatusResult } from "./coding-status"
import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"
import type {
  AssignmentTokenUsageVerification,
  TokenAccountingReplayResult,
  TokenAccountingStatusResult,
} from "./token-accounting"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    codingStatus(): Promise<CodingStatusResult>
    createPylon(): Promise<CreatePylonResult>
    pylonStatus(): Promise<PylonStatusResult>
    replayTokenFailures(): Promise<TokenAccountingReplayResult>
    tokenAccountingStatus(): Promise<TokenAccountingStatusResult>
    verifyAssignmentTokenUsage(
      assignmentRef: string,
    ): Promise<AssignmentTokenUsageVerification>
  }
}
