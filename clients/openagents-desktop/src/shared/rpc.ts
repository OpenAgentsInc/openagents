import type {
  CodexAccountResetResult,
  CodexAccountStatusResult,
} from "./account-status"
import type { CodingStatusResult } from "./coding-status"
import type { CreatePylonResult, PylonStatusResult } from "./pylon-status"

export const OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type OpenAgentsDesktopRPCSchema = {
  requests: {
    codexAccountReset(accountRef: string): Promise<CodexAccountResetResult>
    codexAccountStatus(): Promise<CodexAccountStatusResult>
    codingStatus(): Promise<CodingStatusResult>
    createPylon(): Promise<CreatePylonResult>
    pylonStatus(): Promise<PylonStatusResult>
  }
}
