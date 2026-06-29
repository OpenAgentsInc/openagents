import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"

export const KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 30_000
export const KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT = 50021

export type KhalaCodeDesktopMessageRole = "user" | "assistant" | "system"

export type KhalaCodeDesktopMessage = {
  readonly id: string
  readonly role: KhalaCodeDesktopMessageRole
  readonly body: string
}

export type KhalaCodeDesktopAppInfo = {
  readonly ok: true
  readonly app: "Khala Code Desktop"
  readonly observedAt: string
}

export type KhalaCodeDesktopRPCSchema = {
  requests: {
    appInfo(): Promise<KhalaCodeDesktopAppInfo>
    appleFmReadiness(): Promise<KhalaAppleFmReadiness>
  }
}
