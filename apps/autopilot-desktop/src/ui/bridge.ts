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

import type { Message } from "./message.js"
import type {
  AppleFmReadinessResponse,
  AppleFmSessionStartResponse,
  BuiltInAgentReadinessResponse,
  BuiltInAgentStartResponse,
  InferenceGatewayReadinessResponse,
  InstallReadinessResponse,
  ChooseIdentityParams,
  ChooseIdentityResponse,
  IdentityChoiceStateResponse,
  KhalaTurnResponse,
  ManagedAccountMutationResponse,
  ManagedAccountsResponse,
  OnboardingStatusResponse,
  PublicActivityTimelineResponse,
  PromiseSurfacingInput,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
  ShellTurnResponse,
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
  VerseTurnResponse,
} from "../shared/rpc.js"

// The webview→Bun request surface (mirrors DesktopRPCSchema["bun"]["requests"]).
export type DesktopRequests = {
  openExternal(p: { url: string }): Promise<{ ok: boolean }>
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
  inferenceGatewayReadiness(
    p: Record<string, never>,
  ): Promise<InferenceGatewayReadinessResponse>
  // HUD H5 (#5503): one zero-base shell turn against the live inference gateway.
  // Bun owns the agent token; the webview sends only the prompt and receives the
  // plain Autopilot text (or an honest configure/error message).
  shellTurn(p: { prompt: string }): Promise<ShellTurnResponse>
  // Verse/Tassadar first-paint chat turn.
  verseTurn(p: { prompt: string }): Promise<VerseTurnResponse>
  // EPIC #6017: one streamed Khala cockpit turn from the in-world Verse textbox.
  // Bun owns the agent token; the webview sends the prompt + model + turnId and
  // receives the terminal answer + public-safe receipt. Live token deltas arrive
  // separately on the `khalaToken` Bun→webview push (correlated by turnId).
  khalaTurn(p: {
    prompt: string
    model?: "openagents/khala"
    turnId?: string
  }): Promise<KhalaTurnResponse>
  installReadiness(
    p: Record<string, never>,
  ): Promise<InstallReadinessResponse>
  onboardingStatus(
    p: Record<string, never>,
  ): Promise<OnboardingStatusResponse>
  identityChoiceState(
    p: Record<string, never>,
  ): Promise<IdentityChoiceStateResponse>
  chooseIdentity(p: ChooseIdentityParams): Promise<ChooseIdentityResponse>
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
  listPublicActivityTimeline(
    p: Record<string, never>,
  ): Promise<PublicActivityTimelineResponse>
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
    useDefaultWorktree?: boolean
    // #5471: managed-worktree selector (mutually exclusive with worktreePath).
    repoRef?: {
      provider: "github"
      visibility: "public"
      fullName: string
      branch: string
      commitSha: string
    }
    // CS-A1: per-session provider account.
    accountRef?: string
  }): Promise<{ ok: boolean; sessionRef: string; error?: string }>
  // #5471: resolve a managed-worktree request (GitHub owner/name + base ref) to
  // a concrete repoRef the node can materialize. Bun runs `git ls-remote`; the
  // webview never runs git. No new control verb — the resolved repoRef rides the
  // existing session.spawn.
  resolveManagedWorktree(p: {
    fullName: string
    baseRef: string
    branch: string
  }): Promise<
    | {
        ok: true
        repoRef: {
          provider: "github"
          visibility: "public"
          fullName: string
          branch: string
          commitSha: string
        }
      }
    | { ok: false; error: string }
  >
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
