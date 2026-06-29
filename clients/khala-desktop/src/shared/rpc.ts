import type { KhalaAppleFmReadiness } from "./apple-fm-readiness.js"
import type { KhalaDesktopDashboardResult } from "./operator-dashboard.js"

export const KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type KhalaDesktopRPCSchema = {
  requests: {
    appleFmReadiness(): Promise<KhalaAppleFmReadiness>
    operatorDashboard(): Promise<KhalaDesktopDashboardResult>
    openExternal(input: { readonly url: string }): Promise<{ readonly ok: true }>
  }
}
