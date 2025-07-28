import { Effect, Schedule, Duration } from "effect"
import { invoke } from "@tauri-apps/api/core"
import { IPCError } from "./errors"

export interface CommandResult<T> {
  success: boolean
  data?: T
  error?: string
}

// Base command wrapper with automatic retry and error handling
export const createCommand = <TArgs, TResult>(name: string) => ({
  name,
  invoke: (args: TArgs) =>
    Effect.tryPromise({
      try: () => invoke<CommandResult<TResult>>(name, args as any),
      catch: (error) => new IPCError({ 
        command: name, 
        args, 
        cause: error 
      })
    }).pipe(
      Effect.flatMap((result) => 
        result.success && result.data !== undefined
          ? Effect.succeed(result.data)
          : Effect.fail(new IPCError({
              command: name,
              args,
              cause: result.error || "Unknown error"
            }))
      ),
      Effect.tap(() => Effect.logDebug(`IPC command succeeded: ${name}`)),
      Effect.tapError((error) => Effect.logError(`IPC command failed: ${name}`, error))
    ),
  
  // Variant with retry policy
  invokeWithRetry: (args: TArgs, retryPolicy?: Schedule.Schedule<unknown, unknown, unknown>) =>
    createCommand<TArgs, TResult>(name).invoke(args).pipe(
      Effect.retry(retryPolicy || Schedule.exponential(Duration.millis(100)).pipe(
        Schedule.jittered,
        Schedule.either(Schedule.spaced(Duration.seconds(1))),
        Schedule.whileInput((error: IPCError) => 
          // Only retry on network errors or timeouts
          error.cause?.toString().includes("network") ||
          error.cause?.toString().includes("timeout")
        ),
        Schedule.compose(Schedule.elapsed),
        Schedule.whileOutput((elapsed) => elapsed < Duration.minutes(1))
      ))
    )
})

// Helper for commands without arguments
export const createSimpleCommand = <TResult>(name: string) =>
  createCommand<Record<string, never>, TResult>(name)