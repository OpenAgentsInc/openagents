import { Context, Effect, Layer } from "effect"
import { Command, CommandExecutor } from "@effect/platform"
import { NodeCommandExecutor, NodeFileSystem } from "@effect/platform-node"
import * as FS from "@effect/platform/FileSystem"
import * as path from "node:path"
import * as os from "node:os"
import { 
  CreateVMRequest, 
  StopVMRequest, 
  VMState 
} from "./MicroVMConfig.js"
import {
  FirecrackerBinaryNotFoundError,
  FirecrackerError,
  VMAlreadyExistsError,
  VMNotFoundError,
} from "./errors.js"

export class FirecrackerService extends Context.Tag("@openagentsinc/container/FirecrackerService")<
  FirecrackerService,
  {
    readonly createVM: (request: CreateVMRequest) => Effect.Effect<VMState, FirecrackerError | VMAlreadyExistsError>
    readonly stopVM: (request: StopVMRequest) => Effect.Effect<void, FirecrackerError | VMNotFoundError>
    readonly getVM: (id: string) => Effect.Effect<VMState, VMNotFoundError>
    readonly listVMs: () => Effect.Effect<ReadonlyArray<VMState>>
    readonly getBinaryPath: () => Effect.Effect<string, FirecrackerBinaryNotFoundError>
  }
>() {}

export const FirecrackerServiceLive = Layer.effect(
  FirecrackerService,
  Effect.gen(function* () {
    const fs = yield* FS.FileSystem
    const executor = yield* CommandExecutor
    
    // In-memory VM state tracking
    const vms = new Map<string, VMState>()
    
    // Firecracker binary paths to check
    const firecrackerPaths = [
      "/usr/bin/firecracker",
      "/usr/local/bin/firecracker",
      path.join(os.homedir(), ".local", "bin", "firecracker"),
    ]
    
    const getBinaryPath = () =>
      Effect.gen(function* () {
        for (const binaryPath of firecrackerPaths) {
          const exists = yield* fs.exists(binaryPath)
          if (exists) {
            return binaryPath
          }
        }
        return yield* Effect.fail(new FirecrackerBinaryNotFoundError({
          path: firecrackerPaths.join(", ")
        }))
      })
    
    const createVM = (request: CreateVMRequest) =>
      Effect.gen(function* () {
        // Check if VM already exists
        if (vms.has(request.id)) {
          return yield* Effect.fail(new VMAlreadyExistsError({ vmId: request.id }))
        }
        
        // Verify firecracker binary exists
        const firecrackerPath = yield* getBinaryPath()
        
        // Create socket path if not provided
        const socketPath = request.socketPath || path.join(
          os.tmpdir(),
          `firecracker-${request.id}.sock`
        )
        
        // Create configuration file
        const configPath = path.join(os.tmpdir(), `firecracker-${request.id}.json`)
        const configJson = JSON.stringify(request.config, null, 2)
        yield* fs.writeFileString(configPath, configJson)
        
        // Build firecracker command
        const cmd = Command.make(firecrackerPath, "--api-sock", socketPath, "--config-file", configPath)
        
        // Start the VM process
        const process = yield* executor.start(cmd)
        
        // Create initial VM state
        const vmState = new VMState({
          id: request.id,
          status: "starting",
          pid: process.pid,
          startedAt: new Date(),
        })
        
        // Store VM state
        vms.set(request.id, vmState)
        
        // Wait a bit for VM to start (in production, we'd poll the API socket)
        yield* Effect.sleep("500 millis")
        
        // Update status to running
        const runningState = new VMState({
          ...vmState,
          status: "running",
        })
        vms.set(request.id, runningState)
        
        return runningState
      }).pipe(
        Effect.catchTag("SystemError", (error: any) =>
          Effect.fail(new FirecrackerError({ message: error.message }))
        )
      )
    
    const stopVM = (request: StopVMRequest) =>
      Effect.gen(function* () {
        const vm = vms.get(request.id)
        if (!vm) {
          return yield* Effect.fail(new VMNotFoundError({ vmId: request.id }))
        }
        
        if (vm.status === "stopped") {
          return
        }
        
        // Update status
        const stoppingState = new VMState({
          ...vm,
          status: "stopping",
        })
        vms.set(request.id, stoppingState)
        
        // Kill the process if we have a PID
        if (vm.pid) {
          const killCmd = Command.make("kill", request.force ? "-9" : "-TERM", vm.pid.toString())
          yield* executor.exec(killCmd).pipe(
            Effect.catchAll(() => Effect.void) // Ignore errors if process already dead
          )
        }
        
        // Clean up socket and config files
        const socketPath = path.join(os.tmpdir(), `firecracker-${request.id}.sock`)
        const configPath = path.join(os.tmpdir(), `firecracker-${request.id}.json`)
        
        yield* fs.remove(socketPath).pipe(Effect.catchAll(() => Effect.void))
        yield* fs.remove(configPath).pipe(Effect.catchAll(() => Effect.void))
        
        // Update final state
        const stoppedState = new VMState({
          ...stoppingState,
          status: "stopped",
          stoppedAt: new Date(),
        })
        vms.set(request.id, stoppedState)
      })
    
    const getVM = (id: string) =>
      Effect.gen(function* () {
        const vm = vms.get(id)
        if (!vm) {
          return yield* Effect.fail(new VMNotFoundError({ vmId: id }))
        }
        return vm
      })
    
    const listVMs = () => Effect.succeed(Array.from(vms.values()))
    
    return {
      createVM,
      stopVM,
      getVM,
      listVMs,
      getBinaryPath,
    } as const
  }).pipe(
    Effect.provide(NodeFileSystem.layer),
    Effect.provide(NodeCommandExecutor.layer)
  )
)