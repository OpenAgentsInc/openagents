import type { StreamCursor } from "@openagentsinc/autopilot-control-protocol"

export type ControlStreamClientConfig = {
  baseUrl: string
  tokenRef: string
  initialCursor?: StreamCursor
}

export type ControlStreamClient = {
  readonly config: ControlStreamClientConfig
  connect(): Promise<void>
  close(): void
}

export function createControlStreamClient(config: ControlStreamClientConfig): ControlStreamClient {
  return {
    config,
    async connect() {
      // TODO: implement WS/SSE transport and cursor resume from the shared package.
      throw new Error("ControlStreamClient.connect is not implemented yet")
    },
    close() {
      // Placeholder until the transport owns a live subscription.
    },
  }
}
