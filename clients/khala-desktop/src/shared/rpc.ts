import type { AppleFmSidecarPublicStatus } from "../bun/apple-fm-sidecar.js"
import type { KhalaDesktopDashboardResult } from "./operator-dashboard.js"

export const KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

export type KhalaDesktopRPCSchema = {
  requests: {
    operatorDashboard(): Promise<KhalaDesktopDashboardResult>
    appleFmSidecarStatus(): Promise<AppleFmSidecarPublicStatus>
    openExternal(input: { readonly url: string }): Promise<{ readonly ok: true }>
  }
}
