// CL-44: the shared context every pane and card receives. The shell owns the
// live store (latest node projection + notifications + navigation) and the
// typed RPC request bridge to the Bun main process; panes/cards are pure
// render-from-context + dispatch-request functions, so each lives in its own
// file with no shared mutable wiring.

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"
import type { DeployResultRow, NodeStateMessage } from "../shared/rpc"

export type PaneId = "nodes" | "sessions" | "decisions" | "spawn" | "settings" | "session-detail"

// The webview→Bun request surface (mirrors DesktopRPCSchema["bun"]["requests"]).
// Panes/cards call these to mutate node state; secrets stay in the Bun process.
export type DesktopRequests = {
  deployCloud(p: { target: "cloudrun" | "workers"; ref: string; env?: "production" | "preview" }): Promise<DeployResultRow>
  submitIntent(p: { title: string; body: string }): Promise<{ ok: boolean; status: string; error?: string }>
  resolveApproval(p: { approvalRef: string; decision: "approve" | "deny" }): Promise<{ applied: boolean; duplicate: boolean; decision: string }>
  setCoordinatorPaused(p: { paused: boolean }): Promise<{ paused: boolean }>
  cancelSession(p: { sessionRef: string }): Promise<{ ok: boolean; state: string }>
  spawnSession(p: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[] }): Promise<{ ok: boolean; sessionRef: string; error?: string }>
}

export type PaneContext = {
  // Latest projections (null until the first poll lands).
  readonly node: NodeStateMessage | null
  readonly notifications: NotificationCenterView | null
  // Which session the session-detail pane is focused on.
  readonly selectedSessionRef: string | null
  // Typed RPC requests to the Bun main process.
  readonly request: DesktopRequests
  // Switch panes (optionally focusing a session for the detail pane).
  navigate(pane: PaneId, sessionRef?: string): void
  // Re-render the active pane now (e.g. after a local optimistic change).
  refresh(): void
}

// A pane or card is just a render function from a context into a container.
export type Render = (container: HTMLElement, ctx: PaneContext) => void
