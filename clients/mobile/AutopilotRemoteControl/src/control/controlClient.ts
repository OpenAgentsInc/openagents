import type { ControlCommand } from "@openagentsinc/autopilot-control-protocol"

export type PylonControlClientConfig = {
  baseUrl: string
  tokenRef: string
}

export type PylonControlClient = {
  readonly config: PylonControlClientConfig
  send(command: ControlCommand): Promise<unknown>
}

export function createPylonControlClient(config: PylonControlClientConfig): PylonControlClient {
  return {
    config,
    async send(_command) {
      // TODO: adapt mobile requests over the typed control protocol.
      throw new Error("PylonControlClient.send is not implemented yet")
    },
  }
}
