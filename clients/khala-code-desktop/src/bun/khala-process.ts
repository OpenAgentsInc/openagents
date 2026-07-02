import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { Readable } from "node:stream"

import { Context, Data, Effect, Layer, Sink, Stream } from "effect"
import * as PlatformError from "effect/PlatformError"
import type * as Scope from "effect/Scope"
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process"

export class KhalaProcessSpawnFailure extends Data.TaggedError("KhalaProcessSpawnFailure")<{
  readonly cause: unknown
  readonly command: string
  readonly message: string
}> {}

export class KhalaProcessStreamFailure extends Data.TaggedError("KhalaProcessStreamFailure")<{
  readonly cause: unknown
  readonly command: string
  readonly stream: "stdin" | "stdout" | "stderr" | "all"
  readonly message: string
}> {}

export class KhalaProcessNonZeroExit extends Data.TaggedError("KhalaProcessNonZeroExit")<{
  readonly command: string
  readonly exitCode: number
  readonly message: string
}> {}

export type KhalaProcessFailure =
  | KhalaProcessSpawnFailure
  | KhalaProcessStreamFailure
  | KhalaProcessNonZeroExit

export type KhalaProcessSpawnOptions = Readonly<{
  readonly cwd?: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly extendEnv?: boolean
  readonly forceKillAfter?: ChildProcess.KillOptions["forceKillAfter"]
  readonly killSignal?: ChildProcess.Signal
}>

export type KhalaProcessHandle = Readonly<{
  readonly command: string
  readonly exit: Effect.Effect<number, KhalaProcessFailure>
  readonly kill: (
    options?: ChildProcess.KillOptions,
  ) => Effect.Effect<void, KhalaProcessSpawnFailure>
  readonly pid: number
  readonly stderr: Stream.Stream<Uint8Array, KhalaProcessFailure>
  readonly stdin: Sink.Sink<void, Uint8Array, never, KhalaProcessFailure>
  readonly stdout: Stream.Stream<Uint8Array, KhalaProcessFailure>
}>

export type KhalaProcessServiceShape = Readonly<{
  readonly spawn: (
    command: string,
    args?: readonly string[],
    options?: KhalaProcessSpawnOptions,
  ) => Effect.Effect<KhalaProcessHandle, KhalaProcessSpawnFailure, Scope.Scope>
}>

export type KhalaProcessTextResult = Readonly<{
  readonly exitCode: number
  readonly stderr: string
  readonly stdout: string
}>

const platformToSpawnFailure = (command: string, cause: unknown): KhalaProcessSpawnFailure =>
  new KhalaProcessSpawnFailure({
    cause,
    command,
    message: cause instanceof Error ? cause.message : String(cause),
  })

const platformToStreamFailure = (
  command: string,
  stream: KhalaProcessStreamFailure["stream"],
  cause: unknown,
): KhalaProcessStreamFailure =>
  new KhalaProcessStreamFailure({
    cause,
    command,
    stream,
    message: cause instanceof Error ? cause.message : String(cause),
  })

const platformError = (method: string, cause: unknown): PlatformError.PlatformError =>
  PlatformError.systemError({
    _tag: "Unknown",
    cause,
    description: cause instanceof Error ? cause.message : String(cause),
    method,
    module: "KhalaProcess",
  })

const streamFromNodeReadable = (
  command: string,
  channel: "stdout" | "stderr",
  readable: NodeJS.ReadableStream,
): Stream.Stream<Uint8Array, KhalaProcessFailure> =>
  Stream.fromReadableStream({
    evaluate: () => Readable.toWeb(readable as Readable) as unknown as ReadableStream<Uint8Array>,
    onError: cause => platformToStreamFailure(command, channel, cause),
    releaseLockOnEnd: true,
  })

const writeStdin = (
  command: string,
  proc: ChildProcessWithoutNullStreams,
  chunk: Uint8Array,
): Effect.Effect<void, KhalaProcessStreamFailure> =>
  Effect.callback<void, KhalaProcessStreamFailure>((resume) => {
    proc.stdin.write(chunk, error => {
      if (error === null || error === undefined) {
        resume(Effect.void)
        return
      }
      resume(Effect.fail(platformToStreamFailure(command, "stdin", error)))
    })
  })

const stdinSink = (
  command: string,
  proc: ChildProcessWithoutNullStreams,
): Sink.Sink<void, Uint8Array, never, KhalaProcessFailure> =>
  Sink.forEach((chunk: Uint8Array) => writeStdin(command, proc, chunk)).pipe(
    Sink.ensuring(Effect.sync(() => {
      if (!proc.stdin.destroyed) proc.stdin.end()
    })),
  )

const waitForExit = (
  command: string,
  proc: ChildProcessWithoutNullStreams,
): Effect.Effect<number, KhalaProcessSpawnFailure> =>
  Effect.callback<number, KhalaProcessSpawnFailure>((resume) => {
    const onExit = (code: number | null, signal: NodeJS.Signals | null) => {
      cleanup()
      resume(Effect.succeed(code ?? (signal === null ? 0 : 128)))
    }
    const onError = (cause: Error) => {
      cleanup()
      resume(Effect.fail(platformToSpawnFailure(command, cause)))
    }
    const cleanup = () => {
      proc.off("exit", onExit)
      proc.off("error", onError)
    }
    proc.once("exit", onExit)
    proc.once("error", onError)
  })

const killNodeProcess = (
  proc: ChildProcessWithoutNullStreams,
  signal: ChildProcess.Signal = "SIGTERM",
): void => {
  if (proc.killed || proc.exitCode !== null || proc.signalCode !== null) return
  proc.kill(signal)
}

const makeNodeSpawnerService = (): ChildProcessSpawner.ChildProcessSpawner["Service"] =>
  ChildProcessSpawner.make(command => {
    if (command._tag !== "StandardCommand") {
      return Effect.fail(PlatformError.badArgument({
        description: "Piped commands are not supported by KhalaProcess live spawner yet.",
        method: "spawn",
        module: "KhalaProcess",
      }))
    }

    return Effect.acquireRelease(
      Effect.try({
        try: () => {
          const env = command.options.extendEnv === false
            ? command.options.env
            : { ...process.env, ...command.options.env }
          const proc = spawn(command.command, [...command.args], {
            cwd: command.options.cwd,
            detached: command.options.detached,
            env,
            shell: command.options.shell,
            stdio: ["pipe", "pipe", "pipe"],
          })
          return { command: command.command, proc }
        },
        catch: cause => platformError("spawn", cause),
      }),
      ({ proc }) => Effect.sync(() => killNodeProcess(proc, command.options.killSignal ?? "SIGTERM")),
    ).pipe(
      Effect.map(({ command: commandName, proc }) => ChildProcessSpawner.makeHandle({
        all: Stream.merge(
          streamFromNodeReadable(commandName, "stdout", proc.stdout),
          streamFromNodeReadable(commandName, "stderr", proc.stderr),
        ).pipe(Stream.mapError(cause => platformError("all", cause))),
        exitCode: waitForExit(commandName, proc).pipe(
          Effect.map(code => ChildProcessSpawner.ExitCode(code)),
          Effect.mapError(cause => platformError("exitCode", cause)),
        ),
        getInputFd: () => Sink.drain,
        getOutputFd: () => Stream.empty,
        isRunning: Effect.sync(() => proc.exitCode === null && proc.signalCode === null),
        kill: options => Effect.try({
          try: () => killNodeProcess(proc, options?.killSignal ?? "SIGTERM"),
          catch: cause => platformError("kill", cause),
        }),
        pid: ChildProcessSpawner.ProcessId(proc.pid ?? -1),
        stderr: streamFromNodeReadable(commandName, "stderr", proc.stderr).pipe(
          Stream.mapError(cause => platformError("stderr", cause)),
        ),
        stdin: stdinSink(commandName, proc).pipe(
          Sink.mapError(cause => platformError("stdin", cause)),
        ),
        stdout: streamFromNodeReadable(commandName, "stdout", proc.stdout).pipe(
          Stream.mapError(cause => platformError("stdout", cause)),
        ),
        unref: Effect.sync(() => {
          proc.unref()
          return Effect.sync(() => proc.ref())
        }),
      })),
    )
  })

export const makeKhalaProcessService = (): KhalaProcessServiceShape => {
  const spawner = makeNodeSpawnerService()
  return {
    spawn: (command, args = [], options = {}) => {
      const childCommand = ChildProcess.make(command, [...args], {
        cwd: options.cwd,
        env: options.env === undefined ? undefined : { ...options.env },
        extendEnv: options.extendEnv ?? true,
        forceKillAfter: options.forceKillAfter,
        killSignal: options.killSignal ?? "SIGTERM",
        stderr: { stream: "pipe" },
        stdin: { stream: "pipe" },
        stdout: { stream: "pipe" },
      })

      return Effect.acquireRelease(
        childCommand.pipe(
          Effect.provideService(ChildProcessSpawner.ChildProcessSpawner, spawner),
          Effect.mapError(cause => platformToSpawnFailure(command, cause)),
        ),
        handle => handle.kill({
          forceKillAfter: options.forceKillAfter,
          killSignal: options.killSignal ?? "SIGTERM",
        }).pipe(Effect.ignore),
      ).pipe(
        Effect.map(handle => ({
          command,
          exit: handle.exitCode.pipe(
            Effect.map(code => Number(code)),
            Effect.flatMap(exitCode =>
              exitCode === 0
                ? Effect.succeed(exitCode)
                : Effect.fail(new KhalaProcessNonZeroExit({
                  command,
                  exitCode,
                  message: `${command} exited with code ${exitCode}`,
                }))
            ),
            Effect.mapError(cause =>
              cause instanceof KhalaProcessNonZeroExit
                ? cause
                : platformToSpawnFailure(command, cause)
            ),
          ),
          kill: killOptions => handle.kill(killOptions).pipe(
            Effect.mapError(cause => platformToSpawnFailure(command, cause)),
          ),
          pid: Number(handle.pid),
          stderr: handle.stderr.pipe(
            Stream.mapError(cause => platformToStreamFailure(command, "stderr", cause)),
          ),
          stdin: handle.stdin.pipe(
            Sink.mapError(cause => platformToStreamFailure(command, "stdin", cause)),
          ),
          stdout: handle.stdout.pipe(
            Stream.mapError(cause => platformToStreamFailure(command, "stdout", cause)),
          ),
        })),
      )
    },
  }
}

export class KhalaProcess extends Context.Service<KhalaProcess, KhalaProcessServiceShape>()(
  "KhalaProcess",
  { make: Effect.sync(makeKhalaProcessService) },
) {}

export const KhalaProcessLive = Layer.succeed(KhalaProcess, makeKhalaProcessService())

export const spawnKhalaProcess = (
  command: string,
  args: readonly string[] = [],
  options: KhalaProcessSpawnOptions = {},
): Effect.Effect<KhalaProcessHandle, KhalaProcessSpawnFailure, Scope.Scope> =>
  makeKhalaProcessService().spawn(command, args, options)

const streamText = (
  stream: Stream.Stream<Uint8Array, KhalaProcessFailure>,
): Effect.Effect<string, KhalaProcessFailure> =>
  stream.pipe(Stream.decodeText(), Stream.mkString)

const exitCodeResult = (
  handle: KhalaProcessHandle,
): Effect.Effect<number, KhalaProcessFailure> =>
  handle.exit.pipe(
    Effect.match({
      onFailure: failure => {
        if (failure instanceof KhalaProcessNonZeroExit) return failure.exitCode
        throw failure
      },
      onSuccess: exitCode => exitCode,
    }),
  )

export const collectKhalaProcessText = (
  process: Effect.Effect<KhalaProcessHandle, KhalaProcessSpawnFailure, Scope.Scope>,
): Promise<KhalaProcessTextResult> =>
  Effect.runPromise(
    Effect.scoped(
      Effect.gen(function*() {
        const handle = yield* process
        return yield* Effect.all({
          exitCode: exitCodeResult(handle),
          stderr: streamText(handle.stderr),
          stdout: streamText(handle.stdout),
        }, { concurrency: "unbounded" })
      }),
    ),
  )
