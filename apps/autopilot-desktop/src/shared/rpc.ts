import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

export type NodeStateMessage = {
  readonly ok: boolean
  readonly schema: string
  readonly sessions: SessionSummary[]
}

export type DesktopRPCSchema = {
  readonly bun: {
    readonly requests: Record<string, never>
    readonly messages: Record<string, never>
  }
  readonly webview: {
    readonly requests: Record<string, never>
    readonly messages: {
      readonly nodeState: NodeStateMessage
    }
  }
}
