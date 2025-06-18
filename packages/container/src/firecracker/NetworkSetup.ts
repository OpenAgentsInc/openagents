import { Command, CommandExecutor } from "@effect/platform"
import { NodeCommandExecutor } from "@effect/platform-node"
import { Context, Effect, Layer } from "effect"
import { NetworkSetupError } from "./errors.js"

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

export const NetworkSetupServiceLive = Layer.effect(
  NetworkSetupService,
  Effect.gen(function*() {
    const executor = yield* CommandExecutor.CommandExecutor

    const generateMacAddress = () => {
      // Generate a locally administered MAC address
      const mac = [
        0x02, // Locally administered
        Math.floor(Math.random() * 256),
        Math.floor(Math.random() * 256),
        Math.floor(Math.random() * 256),
        Math.floor(Math.random() * 256),
        Math.floor(Math.random() * 256)
      ]
      return mac.map((b) => b.toString(16).padStart(2, "0")).join(":")
    }

    const createTapInterface = (name: string) =>
      Effect.gen(function*() {
        // Check if running as root or with CAP_NET_ADMIN
        const hasPerms = yield* checkPermissions()
        if (!hasPerms) {
          return yield* Effect.fail(
            new NetworkSetupError({
              message: "Insufficient permissions. Run as root or with CAP_NET_ADMIN capability",
              interface: name
            })
          )
        }

        // Create TAP interface
        const createCmd = Command.make("ip", "tuntap", "add", name, "mode", "tap")
        yield* executor.start(createCmd).pipe(
          Effect.flatMap(() => Effect.void),
          Effect.catchTag("ExitCode", (error: any) =>
            Effect.fail(
              new NetworkSetupError({
                message: `Failed to create TAP interface: ${error.message}`,
                interface: name
              })
            ))
        )

        // Bring interface up
        const upCmd = Command.make("ip", "link", "set", name, "up")
        yield* executor.exec(upCmd).pipe(
          Effect.catchTag("ExitCode", (error: any) =>
            Effect.fail(
              new NetworkSetupError({
                message: `Failed to bring up TAP interface: ${error.message}`,
                interface: name
              })
            ))
        )

        const macAddress = generateMacAddress()

        return {
          name,
          macAddress
        }
      })

    const deleteTapInterface = (name: string) =>
      Effect.gen(function*() {
        const deleteCmd = Command.make("ip", "link", "delete", name)
        yield* executor.exec(deleteCmd).pipe(
          Effect.catchTag("ExitCode", () => Effect.void) // Ignore if already deleted
        )
      })

    const setupBridge = (config: NetworkConfig) =>
      Effect.gen(function*() {
        if (!config.bridgeName) {
          return
        }

        // Add TAP to bridge
        const bridgeCmd = Command.make("ip", "link", "set", config.tapDevice, "master", config.bridgeName)
        yield* executor.exec(bridgeCmd).pipe(
          Effect.catchTag("ExitCode", (error: any) =>
            Effect.fail(
              new NetworkSetupError({
                message: `Failed to add TAP to bridge: ${error.message}`,
                interface: config.tapDevice
              })
            ))
        )
      })

    const checkPermissions = () =>
      Effect.gen(function*() {
        // Try to check capabilities
        const capCmd = Command.make("capsh", "--print")
        const result = yield* executor.exec(capCmd).pipe(
          Effect.map((output: any) => output.stdout.includes("cap_net_admin")),
          Effect.catchAll(() => Effect.succeed(false))
        )

        if (result) return true

        // Check if running as root
        const idCmd = Command.make("id", "-u")
        const uid = yield* executor.exec(idCmd).pipe(
          Effect.map((output: any) => parseInt(output.stdout.trim())),
          Effect.catchAll(() => Effect.succeed(-1))
        )

        return uid === 0
      })

    return {
      createTapInterface,
      deleteTapInterface,
      setupBridge,
      checkPermissions
    } as const
  }).pipe(
    Effect.provide(NodeCommandExecutor.layer)
  )
)
