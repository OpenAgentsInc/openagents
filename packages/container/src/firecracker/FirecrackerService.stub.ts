import { Context, Effect, Layer } from "effect"
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

export const FirecrackerServiceLive = Layer.succeed(
  FirecrackerService,
  {
    createVM: (request: CreateVMRequest) => 
      Effect.succeed(new VMState({
        id: request.id,
        status: "running",
        pid: 12345,
        startedAt: new Date(),
      })),
      
    stopVM: (request: StopVMRequest) => 
      Effect.void,
      
    getVM: (id: string) => 
      Effect.succeed(new VMState({
        id,
        status: "running",
        pid: 12345,
        startedAt: new Date(),
      })),
      
    listVMs: () => 
      Effect.succeed([]),
      
    getBinaryPath: () => 
      Effect.fail(new FirecrackerBinaryNotFoundError({ path: "/usr/bin/firecracker" }))
  }
)