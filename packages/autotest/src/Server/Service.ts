import { CommandExecutor } from "@effect/platform/CommandExecutor"
import * as Command from "@effect/platform/Command"
import { Context, Effect, Layer, Ref, Schedule, Stream, Fiber } from "effect"
import * as net from "node:net"
import type { ServerOptions, ServerProcess, ServerState } from "./types.js"
import { ServerError, ServerPortError, ServerTimeoutError } from "./errors.js"

export class ServerService extends Context.Tag("@openagentsinc/autotest/ServerService")<
  ServerService,
  {
    readonly start: (options: ServerOptions) => Effect.Effect<ServerProcess, ServerError | ServerTimeoutError | ServerPortError>
    readonly stop: (process: ServerProcess) => Effect.Effect<void, ServerError>
    readonly getState: (process: ServerProcess) => Effect.Effect<ServerState>
    readonly waitForReady: (process: ServerProcess, options?: { timeout?: number; pattern?: RegExp }) => Effect.Effect<void, ServerTimeoutError>
  }
>() {}

// Helper to check if port is available
const isPortAvailable = (port: number): Effect.Effect<boolean, never> =>
  Effect.async<boolean>((resume) => {
    const server = net.createServer()
    server.once("error", () => resume(Effect.succeed(false)))
    server.once("listening", () => {
      server.close()
      resume(Effect.succeed(true))
    })
    server.listen(port)
  })

// Helper to find available port
const findAvailablePort = (startPort: number): Effect.Effect<number, ServerPortError> =>
  Effect.gen(function*() {
    for (let port = startPort; port < startPort + 100; port++) {
      const available = yield* isPortAvailable(port)
      if (available) return port
    }
    return yield* Effect.fail(new ServerPortError(startPort))
  })

export const ServerServiceLive = Layer.effect(
  ServerService,
  Effect.gen(function*() {
    const executor = yield* CommandExecutor

    // Store for process state
    const processStates = yield* Ref.make(new Map<number, ServerState>())
    // Store for daemon fibers managing processes
    const processFibers = yield* Ref.make(new Map<number, any>())

    return ServerService.of({
      start: (options: ServerOptions) =>
        Effect.gen(function*() {
          // Find available port
          const actualPort = yield* findAvailablePort(options.port)

          // Parse command and args
          const [cmd, ...cmdArgs] = options.command.split(" ")
          const allArgs = [...cmdArgs, ...(options.args ?? [])]

          // Create command with environment
          const command = Command.make(cmd, ...allArgs).pipe(
            Command.workingDirectory(options.cwd ?? process.cwd()),
            Command.env({
              ...process.env,
              ...options.env,
              PORT: actualPort.toString()
            }),
            Command.stdout("pipe"),
            Command.stderr("pipe")
          )

          // Use a promise to get the process info from the daemon fiber
          const { promise, resolve, reject } = yield* Effect.sync(() => {
            let resolve: (value: any) => void
            let reject: (error: any) => void
            const promise = new Promise((res, rej) => {
              resolve = res
              reject = rej
            })
            return { promise, resolve: resolve!, reject: reject! }
          })

          // Start process in a daemon fiber that will outlive this effect
          const fiber = yield* Effect.forkDaemon(
            Effect.scoped(
              Effect.gen(function*() {
                try {
                  // Start process
                  const proc = yield* executor.start(command).pipe(
                    Effect.mapError((error) => new ServerError(`Failed to start server: ${error.message}`, error))
                  )

                  // Send process info back
                  yield* Effect.sync(() => resolve(proc))

                  // Keep the fiber alive to maintain the process
                  yield* Effect.never
                } catch (error) {
                  yield* Effect.sync(() => reject(error))
                }
              })
            )
          )

          // Wait for process to start
          const proc = yield* Effect.tryPromise({
            try: () => promise,
            catch: (error) => new ServerError(`Failed to start server: ${error}`, error)
          })

          // Create process info
          const serverProcess: ServerProcess = {
            _tag: "ServerProcess",
            pid: proc.pid,
            port: actualPort,
            url: `http://localhost:${actualPort}`,
            logs: []
          }

          // Initialize state
          yield* Ref.update(processStates, (states) =>
            new Map(states).set(proc.pid, {
              status: "starting",
              process: serverProcess,
              startedAt: new Date()
            })
          )

          // Set up log collection using daemon fibers so they outlive the scope
          const collectLogs = Effect.gen(function*() {
            // Collect stdout
            yield* proc.stdout.pipe(
              Stream.decodeText(),
              Stream.splitLines,
              Stream.tap((line) =>
                Effect.gen(function*() {
                  const trimmedLine = line.trim()
                  if (trimmedLine) {
                    console.log(`[${proc.pid}]`, trimmedLine)
                    
                    // Update state immediately with new log
                    yield* Ref.update(processStates, (states) => {
                      const state = states.get(proc.pid)
                      if (state?.process) {
                        const updatedLogs = [...state.process.logs, trimmedLine]
                        const updatedProcess = { ...state.process, logs: updatedLogs }
                        states.set(proc.pid, { ...state, process: updatedProcess })
                      }
                      return states
                    })
                  }
                })
              ),
              Stream.runDrain
            ).pipe(Effect.forkDaemon)

            // Collect stderr
            yield* proc.stderr.pipe(
              Stream.decodeText(),
              Stream.splitLines,
              Stream.tap((line) =>
                Effect.gen(function*() {
                  const trimmedLine = line.trim()
                  if (trimmedLine) {
                    console.error(`[${proc.pid}] [stderr]`, trimmedLine)
                    
                    // Update state immediately with new log
                    yield* Ref.update(processStates, (states) => {
                      const state = states.get(proc.pid)
                      if (state?.process) {
                        const updatedLogs = [...state.process.logs, `[stderr] ${trimmedLine}`]
                        const updatedProcess = { ...state.process, logs: updatedLogs }
                        states.set(proc.pid, { ...state, process: updatedProcess })
                      }
                      return states
                    })
                  }
                })
              ),
              Stream.runDrain
            ).pipe(Effect.forkDaemon)
          })

          // Start log collection
          yield* collectLogs
          
          // Give the process a moment to start and output initial logs
          yield* Effect.sleep("100 millis")

          // Handle process exit
          yield* proc.exitCode.pipe(
            Effect.tap((code) =>
              Ref.update(processStates, (states) => {
                const state = states.get(proc.pid)
                if (state) {
                  states.set(proc.pid, {
                    ...state,
                    status: code === 0 ? "stopped" : "error",
                    stoppedAt: new Date(),
                    error: code !== 0 ? new Error(`Process exited with code ${code}`) : undefined
                  })
                }
                return states
              })
            ),
            Effect.forkDaemon
          )

          // Store the fiber reference
          yield* Ref.update(processFibers, (fibers) =>
            new Map(fibers).set(proc.pid, fiber)
          )

          return serverProcess
        }),

      stop: (process: ServerProcess) =>
        Effect.gen(function*() {
          // Interrupt the daemon fiber
          const fibers = yield* Ref.get(processFibers)
          const fiber = fibers.get(process.pid)
          if (fiber) {
            yield* Fiber.interrupt(fiber)
            yield* Ref.update(processFibers, (fibers) => {
              const newFibers = new Map(fibers)
              newFibers.delete(process.pid)
              return newFibers
            })
          }

          // Kill the process
          const killCmd = Command.make("kill", "-TERM", process.pid.toString())
          yield* executor.exitCode(killCmd).pipe(
            Effect.mapError((error) => new ServerError(`Failed to stop server: ${error.message}`, error)),
            Effect.orElse(() => Effect.succeed(0))
          )

          // Update state
          yield* Ref.update(processStates, (states) => {
            const state = states.get(process.pid)
            if (state) {
              states.set(process.pid, {
                ...state,
                status: "stopped",
                stoppedAt: new Date()
              })
            }
            return states
          })
        }),

      getState: (process: ServerProcess) =>
        Effect.gen(function*() {
          const states = yield* Ref.get(processStates)
          const state = states.get(process.pid)
          if (!state) {
            return {
              status: "stopped" as const,
              error: new Error("Process not found")
            }
          }
          return state
        }),

      waitForReady: (process: ServerProcess, options) =>
        Effect.gen(function*() {
          const timeout = options?.timeout ?? 30000
          const pattern = options?.pattern ?? /listening|ready|started|running/i

          const checkReady = Effect.gen(function*() {
            const state = yield* Ref.get(processStates)
            const procState = state.get(process.pid)
            
            if (!procState?.process) {
              return false
            }

            // Check if any log matches the ready pattern
            const isReady = procState.process.logs.some(log => pattern.test(log))
            
            if (isReady && procState.status === "starting") {
              // Update state to ready
              yield* Ref.update(processStates, (states) => {
                const current = states.get(process.pid)
                if (current) {
                  states.set(process.pid, {
                    ...current,
                    status: "ready",
                    readyAt: new Date()
                  })
                }
                return states
              })
            }

            return isReady
          })

          // Poll for ready state
          const waitLoop = Effect.gen(function*() {
            while (true) {
              const ready = yield* checkReady
              if (ready) return

              // Check if process died
              const state = yield* Ref.get(processStates)
              const procState = state.get(process.pid)
              if (procState?.status === "error" || procState?.status === "stopped") {
                return yield* Effect.fail(
                  new ServerError(`Server process died before becoming ready`, procState.error)
                )
              }

              yield* Effect.sleep("500 millis")
            }
          })

          yield* waitLoop.pipe(
            Effect.timeout(timeout),
            Effect.mapError(() => new ServerTimeoutError(timeout))
          )
        })
    })
  })
)