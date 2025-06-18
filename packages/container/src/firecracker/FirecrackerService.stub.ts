import { Context, Effect, Layer } from "effect"
import type { FirecrackerError, VMAlreadyExistsError, VMNotFoundError } from "./errors.js"
import { FirecrackerBinaryNotFoundError } from "./errors.js"
import type { CreateVMRequest, CreateVMRequest, StopVMRequest, StopVMRequest } from "./MicroVMConfig.js"
import { VMState } from "./MicroVMConfig.js"

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

export const FirecrackerServiceLive = Layer.succeed(
  FirecrackerService,
  {
    createVM: (_request: CreateVMRequest) =>
      Effect.succeed(
        new VMState({
          id: _request.id,
          status: "running",
          pid: 12345,
          startedAt: new Date()
        })
      ),

    stopVM: (_request: StopVMRequest) => Effect.void,

    getVM: (id: string) =>
      Effect.succeed(
        new VMState({
          id,
          status: "running",
          pid: 12345,
          startedAt: new Date()
        })
      ),

    listVMs: () => Effect.succeed([]),

    getBinaryPath: () => Effect.fail(new FirecrackerBinaryNotFoundError({ path: "/usr/bin/firecracker" }))
  }
)
