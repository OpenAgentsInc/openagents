import { PlatformError } from "@effect/platform/Error"

export interface ServerOptions {
  readonly command: string
  readonly args?: ReadonlyArray<string>
  readonly cwd?: string
  readonly env: Record<string, string>
  readonly port?: number
  readonly readyPattern?: RegExp
  readonly timeout?: number
}

export interface ServerProcess {
  readonly _tag: "ServerProcess"
  readonly pid: number
  readonly port: number
  readonly url: string
  readonly logs: ReadonlyArray<string>
}

export interface ServerState {
  readonly status: "starting" | "ready" | "stopping" | "stopped" | "error"
  readonly process?: ServerProcess
  readonly error?: Error
  readonly startedAt?: Date
  readonly readyAt?: Date
  readonly stoppedAt?: Date
}

export class ServerError extends PlatformError {
  readonly _tag = "ServerError"
  
  constructor(
    readonly message: string,
    readonly cause?: unknown
  ) {
    super(message)
  }
}

export class ServerTimeoutError extends PlatformError {
  readonly _tag = "ServerTimeoutError"
  
  constructor(
    readonly timeout: number,
    readonly cause?: unknown
  ) {
    super(`Server failed to start within ${timeout}ms`)
  }
}

export class ServerPortError extends PlatformError {
  readonly _tag = "ServerPortError"
  
  constructor(
    readonly port: number,
    readonly cause?: unknown
  ) {
    super(`Port ${port} is already in use`)
  }
}