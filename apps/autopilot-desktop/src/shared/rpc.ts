import type {
  NotificationCenterView,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

export type SessionEventRow = {
  readonly eventIndex: number
  readonly phase: string
  readonly state: string
  readonly observedAt: string
  readonly detail: string
}

export type AccountRow = {
  readonly provider: string
  readonly homeState: string
  readonly ready: boolean
}

export type SessionArtifactStats = {
  readonly kind: string
  readonly outcome: string | null
  readonly editedFileCount: number | null
  readonly commandCount: number | null
  readonly totalTokens: number | null
}

// CL-26 "Deploy to Cloud": read-only projection of the node's last deploy.
export type DeployStatusRow = {
  readonly state: "queued" | "building" | "deployed" | "failed" | "unknown"
  readonly url: string | null
  readonly deployedAt: string | null
  readonly message: string
}

export type DeployResultRow = {
  readonly accepted: boolean
  readonly reason: string
  readonly errors: string[]
}

export type NodeStateMessage = {
  readonly ok: boolean
  readonly schema: string
  readonly sessions: SessionSummary[]
  // CL-5: bounded recent-events tail per session for the live detail timeline.
  readonly events?: Record<string, SessionEventRow[]>
  // CL-18/CL-20: read-only provider/account readiness.
  readonly accounts?: AccountRow[]
  // CL-19: retained artifact stats per terminal session.
  readonly artifacts?: Record<string, SessionArtifactStats>
  // CL-26: read-only projection of the node's last "Deploy to Cloud".
  readonly deploy?: DeployStatusRow
}

export type DesktopRPCSchema = {
  readonly bun: {
    // CL-26: the webview asks the Bun side to trigger a deploy of the node's
    // own cloud service. The node fail-safe-gates execution behind
    // OA_DEPLOY_ENABLE=1, so an un-enabled node returns accepted:false.
    readonly requests: {
      readonly deployCloud: {
        readonly params: { target: "cloudrun" | "workers"; ref: string; env?: "production" | "preview" }
        readonly response: DeployResultRow
      }
    }
    readonly messages: Record<string, never>
  }
  readonly webview: {
    readonly requests: Record<string, never>
    readonly messages: {
      readonly nodeState: NodeStateMessage
      // CL-30: in-app notification center (unread count + recent items),
      // derived from newly notify-worthy sessions on the Bun side.
      readonly notifications: NotificationCenterView
    }
  }
}
