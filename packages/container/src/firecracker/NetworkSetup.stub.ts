import { Context, Effect, Layer } from "effect"
import type { NetworkSetupError } from "./errors.js"

export interface NetworkConfig {
  readonly tapDevice: string
  readonly bridgeName?: string
  readonly ipAddress?: string
  readonly gateway?: string
}

export interface TapInterface {
  readonly name: string
  readonly macAddress: string
}

export class NetworkSetupService extends Context.Tag("@openagentsinc/container/NetworkSetupService")<
  NetworkSetupService,
  {
    readonly createTapInterface: (name: string) => Effect.Effect<TapInterface, NetworkSetupError>
    readonly deleteTapInterface: (name: string) => Effect.Effect<void, NetworkSetupError>
    readonly setupBridge: (config: NetworkConfig) => Effect.Effect<void, NetworkSetupError>
    readonly checkPermissions: () => Effect.Effect<boolean>
  }
>() {}

export const NetworkSetupServiceLive = Layer.succeed(
  NetworkSetupService,
  {
    createTapInterface: (name: string) =>
      Effect.succeed({
        name,
        macAddress: "02:00:00:00:00:01"
      }),

    deleteTapInterface: (_name: string) => Effect.void,

    setupBridge: (_config: NetworkConfig) => Effect.void,

    checkPermissions: () => Effect.succeed(false)
  }
)
