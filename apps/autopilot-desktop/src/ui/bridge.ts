// CL-53: the module bridge between the Electrobun RPC surface and the Foldkit
// runtime.
//
// The webview talks to the Bun main process over the typed `DesktopRPCSchema`
// (Electroview). Two directions cross this bridge:
//
//   * outbound — Commands (update.ts) call `rpc.request.<verb>(...)`, which is a
//     Promise. We stash `rpc.request` here so the pure Command definitions can
//     reach it without importing electrobun.
//   * inbound — the Electroview message handlers (`nodeState` / `notifications`)
//     push messages into the runtime via an emitter the persistent subscription
//     stream registers here (Effect `Stream.async`).
//
// Keeping both halves in one tiny module means update.ts / subscriptions.ts stay
// free of electrobun imports and remain unit-testable.

import type { Message } from "./message"
import type {
  TrainingDashboardSummaryResponse,
  TrainingPlanResponse,
  TrainingPromiseGatesResponse,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowLeaseResponse,
} from "../shared/rpc"

// The webview→Bun request surface (mirrors DesktopRPCSchema["bun"]["requests"]).
export type DesktopRequests = {
  deployCloud(p: {
    target: "cloudrun" | "workers"
    ref: string
    env?: "production" | "preview"
  }): Promise<{ accepted: boolean; reason: string; errors: string[] }>
  submitIntent(p: {
    title: string
    body: string
  }): Promise<{ ok: boolean; status: string; error?: string }>
  listTrainingRuns(p: Record<string, never>): Promise<TrainingRunsResponse>
  listTrainingDashboard(
    p: Record<string, never>,
  ): Promise<TrainingDashboardSummaryResponse>
  listTrainingPromiseGates(
    p: Record<string, never>,
  ): Promise<TrainingPromiseGatesResponse>
  planTrainingRunWindow(
    p: Record<string, never>,
  ): Promise<TrainingPlanResponse>
  activateTrainingWindow(p: {
    windowRef: string
  }): Promise<TrainingWindowActionResponse>
  reconcileTrainingWindow(p: {
    windowRef: string
  }): Promise<TrainingWindowActionResponse>
  claimTrainingWindowLease(
    p: Record<string, never>,
  ): Promise<TrainingWindowLeaseResponse>
  resolveApproval(p: {
    approvalRef: string
    decision: "approve" | "deny"
  }): Promise<{ applied: boolean; duplicate: boolean; decision: string }>
  setCoordinatorPaused(p: { paused: boolean }): Promise<{ paused: boolean }>
  cancelSession(p: { sessionRef: string }): Promise<{ ok: boolean; state: string }>
  spawnSession(p: {
    adapter: "codex" | "claude_agent"
    objective: string
    verify?: string[]
  }): Promise<{ ok: boolean; sessionRef: string; error?: string }>
}

let request: DesktopRequests | null = null
let emit: ((message: Message) => void) | null = null

// Set by main.ts once the Electroview rpc exists.
export const setRequest = (next: DesktopRequests): void => {
  request = next
}

// Commands read this. Throws if called before wiring (a programming error).
export const getRequest = (): DesktopRequests => {
  if (request === null) {
    throw new Error("desktop RPC request bridge not initialized")
  }
  return request
}

// Registered by the persistent subscription stream (subscriptions.ts). The
// Electroview message handlers (main.ts) call `pushInbound` to feed the runtime.
export const setEmit = (next: ((message: Message) => void) | null): void => {
  emit = next
}

export const pushInbound = (message: Message): void => {
  emit?.(message)
}
