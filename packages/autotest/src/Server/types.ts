export interface ServerOptions {
  readonly command: string
  readonly args?: ReadonlyArray<string>
  readonly cwd?: string
  readonly env: Record<string, string>
  readonly port: number
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

export class ServerError extends Error {
  readonly _tag = "ServerError" as const
  
  constructor(
    message: string,
    readonly cause?: unknown
  ) {
    super(message)
    this.name = "ServerError"
  }
}

export class ServerTimeoutError extends Error {
  readonly _tag = "ServerTimeoutError" as const
  
  constructor(
    readonly timeout: number,
    readonly cause?: unknown
  ) {
    super(`Server failed to start within ${timeout}ms`)
    this.name = "ServerTimeoutError"
  }
}

export class ServerPortError extends Error {
  readonly _tag = "ServerPortError" as const
  
  constructor(
    readonly port: number,
    readonly cause?: unknown
  ) {
    super(`Port ${port} is already in use`)
    this.name = "ServerPortError"
  }
}