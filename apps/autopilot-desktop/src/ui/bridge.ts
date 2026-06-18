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
  AppleFmReadinessResponse,
  AppleFmSessionStartResponse,
  BuiltInAgentReadinessResponse,
  BuiltInAgentStartResponse,
  InstallReadinessResponse,
  ManagedAccountMutationResponse,
  ManagedAccountsResponse,
  PromiseSurfacingInput,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
  TrainingBootstrapGrantResponse,
  TrainingDashboardSummaryResponse,
  TrainingEvidenceAdmissionResponse,
  TrainingEvidencePacketBuildResponse,
  TrainingEvidencePacketSummaryResponse,
  TrainingOperatorReadinessResponse,
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
  builtinAgentReadiness(
    p: Record<string, never>,
  ): Promise<BuiltInAgentReadinessResponse>
  startBuiltInAgent(
    p: Record<string, never>,
  ): Promise<BuiltInAgentStartResponse>
  appleFmReadiness(
    p: Record<string, never>,
  ): Promise<AppleFmReadinessResponse>
  startAppleFmSession(
    p: Record<string, never>,
  ): Promise<AppleFmSessionStartResponse>
  installReadiness(
    p: Record<string, never>,
  ): Promise<InstallReadinessResponse>
  promiseSurfacingReadiness(
    p: Record<string, never>,
  ): Promise<PromiseSurfacingReadinessResponse>
  surfacePromiseGap(
    p: PromiseSurfacingInput,
  ): Promise<PromiseSurfacingResponse>
  listTrainingRuns(p: Record<string, never>): Promise<TrainingRunsResponse>
  listTrainingDashboard(
    p: Record<string, never>,
  ): Promise<TrainingDashboardSummaryResponse>
  listTrainingPromiseGates(
    p: Record<string, never>,
  ): Promise<TrainingPromiseGatesResponse>
  listTrainingOperatorReadiness(
    p: Record<string, never>,
  ): Promise<TrainingOperatorReadinessResponse>
  listTrainingEvidencePacketSummary(
    p: Record<string, never>,
  ): Promise<TrainingEvidencePacketSummaryResponse>
  buildTrainingEvidencePacket(p: {
    trainingRunRef: string
  }): Promise<TrainingEvidencePacketBuildResponse>
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
  requestTrainingBootstrapGrant(p: {
    trainingRunRef: string
  }): Promise<TrainingBootstrapGrantResponse>
  admitTrainingRealGradientEvidence(p: {
    trainingRunRef: string
  }): Promise<TrainingEvidenceAdmissionResponse>
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
    // #4998: requested execution lane (auto|local|cloud-gcp|cloud-shc).
    lane?: "auto" | "local" | "cloud-gcp" | "cloud-shc"
    timeoutSeconds?: number
    worktreePath?: string
    // CS-A1: per-session provider account.
    accountRef?: string
  }): Promise<{ ok: boolean; sessionRef: string; error?: string }>
  // CS-A1: spawn a bounded local Apple FM coding session (its own control verb).
  spawnAppleFmSession(p: {
    objective: string
    worktreePath?: string
  }): Promise<AppleFmSessionStartResponse>
  // CS-A1 account management against the node's local dev.accounts config.
  listManagedAccounts(p: Record<string, never>): Promise<ManagedAccountsResponse>
  addManagedAccount(p: {
    ref: string
    provider: "codex" | "claude_agent"
    home: string
    priority?: number
  }): Promise<ManagedAccountMutationResponse>
  removeManagedAccount(p: {
    ref: string
    provider: "codex" | "claude_agent"
  }): Promise<ManagedAccountMutationResponse>
  setManagedAccountPriority(p: {
    ref: string
    provider: "codex" | "claude_agent"
    priority: number
  }): Promise<ManagedAccountMutationResponse>
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
