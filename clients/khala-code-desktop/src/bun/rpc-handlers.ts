import type { KhalaAppleFmReadiness } from "../shared/apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "../shared/on-device-decider.js"
import {
  type KhalaCodeDesktopAppInfo,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc.js"
import {
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "./khala-chat-runtime.js"

type ChatEnv = Readonly<Record<string, string | undefined>>
type MaybePromise<T> = T | Promise<T>

export type KhalaCodeDesktopRpcHandlersInput = {
  readonly appleFmReadiness: () => MaybePromise<KhalaAppleFmReadiness>
  readonly env: ChatEnv
  readonly onDeviceDeciderStatus: () => MaybePromise<OnDeviceDeciderSelection>
  readonly workingDirectory: string
}

const appInfo = (): KhalaCodeDesktopAppInfo => ({
  ok: true,
  app: "Khala Code Desktop",
  observedAt: new Date().toISOString(),
})

const runtimeStatus = (input: {
  readonly available: boolean
  readonly capability: KhalaCodeDesktopRuntimeStatus["capability"]
  readonly reason: string
  readonly status: KhalaCodeDesktopRuntimeStatus["status"]
}): KhalaCodeDesktopRuntimeStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  available: input.available,
  capability: input.capability,
  observedAt: new Date().toISOString(),
  reason: input.reason,
  status: input.status,
})

export function createKhalaCodeDesktopRpcRequestHandlers(
  input: KhalaCodeDesktopRpcHandlersInput,
): KhalaCodeDesktopRPCSchema["requests"] {
  return {
    async appInfo() {
      return appInfo()
    },
    async appleFmReadiness() {
      return input.appleFmReadiness()
    },
    async codexAccountsStatus() {
      return runtimeStatus({
        available: false,
        capability: "codex_accounts",
        reason: "Khala Code Desktop does not manage Codex account state yet.",
        status: "not_configured",
      })
    },
    async codingStatus() {
      return runtimeStatus({
        available: true,
        capability: "coding",
        reason: "Khala Code chat and owner-local tools are served by this desktop process.",
        status: "ready",
      })
    },
    async onDeviceDeciderStatus() {
      return input.onDeviceDeciderStatus()
    },
    async pylonStatus() {
      return runtimeStatus({
        available: false,
        capability: "pylon",
        reason: "Khala Code Desktop is not attached to a Pylon node in this simplified app.",
        status: "not_configured",
      })
    },
    async submitChatMessage(request) {
      return runKhalaCodeDesktopChatTurn({
        env: input.env,
        request,
        workingDirectory: input.workingDirectory,
      })
    },
    async tokenAccountingStatus() {
      return runtimeStatus({
        available: false,
        capability: "token_accounting",
        reason: "Token accounting is handled by hosted OpenAgents when a cloud credential is configured.",
        status: "not_configured",
      })
    },
    async toolCatalog() {
      return khalaCodeDesktopToolCatalog()
    },
  }
}
