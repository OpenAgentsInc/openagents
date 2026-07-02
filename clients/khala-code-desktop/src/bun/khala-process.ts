import { EventEmitter } from "node:events"
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { Readable } from "node:stream"

import { Context, Data, Duration, Effect, Exit, Layer, Scope, Sink, Stream } from "effect"
import * as PlatformError from "effect/PlatformError"
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
  readonly detached?: boolean
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
  readonly unref: Effect.Effect<Effect.Effect<void, KhalaProcessFailure>, KhalaProcessFailure>
  readonly writeStdin: (chunk: string | Uint8Array) => Effect.Effect<void, KhalaProcessFailure>
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

export type KhalaProcessNodeChild = EventEmitter & Readonly<{
  readonly exited: Promise<number>
  readonly kill: (signal?: ChildProcess.Signal) => unknown
  readonly pid: number
  readonly stderr: Readable
  readonly stdin: {
    readonly write: (chunk: string | Uint8Array) => unknown
  }
  readonly stdout: Readable
  readonly unref: () => unknown
}>

const nodeStdinWriters = new WeakMap<object, (chunk: Uint8Array) => Effect.Effect<void, PlatformError.PlatformError>>()

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

const encodeStdinChunk = (chunk: string | Uint8Array): Uint8Array =>
  typeof chunk === "string" ? new TextEncoder().encode(chunk) : chunk

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

const nodeProcessIsRunning = (proc: ChildProcessWithoutNullStreams): boolean =>
  proc.exitCode === null && proc.signalCode === null

const killNodeProcess = (
  proc: ChildProcessWithoutNullStreams,
  options: ChildProcess.KillOptions = {},
): Effect.Effect<void, PlatformError.PlatformError> =>
  Effect.callback<void, PlatformError.PlatformError>((resume) => {
    if (!nodeProcessIsRunning(proc)) {
      resume(Effect.void)
      return
    }

    const signal = options.killSignal ?? "SIGTERM"
    const forceKillAfterMs =
      options.forceKillAfter === undefined
        ? null
        : Math.max(0, Duration.toMillis(options.forceKillAfter))
    let forceKillTimer: ReturnType<typeof setTimeout> | null = null
    let settled = false

    const cleanup = () => {
      if (forceKillTimer !== null) {
        clearTimeout(forceKillTimer)
        forceKillTimer = null
      }
      proc.off("exit", onExit)
      proc.off("error", onError)
    }
    const settle = (effect: Effect.Effect<void, PlatformError.PlatformError>) => {
      if (settled) return
      settled = true
      cleanup()
      resume(effect)
    }
    const onExit = () => settle(Effect.void)
    const onError = (cause: Error) => settle(Effect.fail(platformError("kill", cause)))

    try {
      if (forceKillAfterMs !== null) {
        proc.once("exit", onExit)
        proc.once("error", onError)
      }
      proc.kill(signal)
      if (forceKillAfterMs === null) {
        settle(Effect.void)
        return
      }
      forceKillTimer = setTimeout(() => {
        try {
          if (nodeProcessIsRunning(proc)) proc.kill("SIGKILL")
          settle(Effect.void)
        } catch (cause) {
          settle(Effect.fail(platformError("kill", cause)))
        }
      }, forceKillAfterMs)
      forceKillTimer.unref?.()
    } catch (cause) {
      settle(Effect.fail(platformError("kill", cause)))
    }
  })

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
      ({ proc }) =>
        killNodeProcess(proc, { killSignal: command.options.killSignal }).pipe(Effect.ignore),
    ).pipe(
      Effect.map(({ command: commandName, proc }) => {
        const handle = ChildProcessSpawner.makeHandle({
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
          kill: options => killNodeProcess(proc, options),
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
        })
        nodeStdinWriters.set(handle, chunk =>
          writeStdin(commandName, proc, chunk).pipe(Effect.mapError(cause => platformError("stdin", cause)))
        )
        return handle
      }),
    )
  })

export const makeKhalaProcessService = (): KhalaProcessServiceShape => {
  const spawner = makeNodeSpawnerService()
  return {
    spawn: (command, args = [], options = {}) => {
      const childCommand = ChildProcess.make(command, [...args], {
        cwd: options.cwd,
        detached: options.detached,
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
          unref: handle.unref.pipe(
            Effect.map(reref =>
              reref.pipe(Effect.mapError(cause => platformToSpawnFailure(command, cause)))
            ),
            Effect.mapError(cause => platformToSpawnFailure(command, cause)),
          ),
          writeStdin: chunk => {
            const writer = nodeStdinWriters.get(handle)
            if (writer === undefined) {
              return Effect.fail(platformToStreamFailure(command, "stdin", "stdin writer is unavailable"))
            }
            return writer(encodeStdinChunk(chunk)).pipe(
              Effect.mapError(cause => platformToStreamFailure(command, "stdin", cause)),
            )
          },
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

export const spawnKhalaProcessNodeChild = async (
  command: string,
  args: readonly string[] = [],
  options: KhalaProcessSpawnOptions = {},
): Promise<KhalaProcessNodeChild> => {
  const scope = Effect.runSync(Scope.make())
  const handle = await Effect.runPromise(
    Effect.provideService(spawnKhalaProcess(command, args, options), Scope.Scope, scope),
  )
  let scopeClosed = false
  const closeScope = async (): Promise<void> => {
    if (scopeClosed) return
    scopeClosed = true
    await Effect.runPromise(Scope.close(scope, Exit.void).pipe(Effect.ignore))
  }
  const bufferedChunks = async function*(
    stream: Stream.Stream<Uint8Array, KhalaProcessFailure>,
  ): AsyncIterable<Buffer> {
    for await (const chunk of Stream.toAsyncIterable(stream)) {
      yield Buffer.from(chunk)
    }
  }
  const stdout = Readable.from(bufferedChunks(handle.stdout))
  const stderr = Readable.from(bufferedChunks(handle.stderr))
  const events = new EventEmitter()
  const exited = Effect.runPromise(
    handle.exit.pipe(
      Effect.match({
        onFailure: failure =>
          failure instanceof KhalaProcessNonZeroExit ? failure.exitCode : 127,
        onSuccess: exitCode => exitCode,
      }),
    ),
  )
  void exited.then(
    exitCode => {
      events.emit("close", exitCode, null)
      void closeScope()
    },
    failure => {
      events.emit("error", failure)
      events.emit("close", 127, null)
      void closeScope()
    },
  )

  return Object.assign(events, {
    exited,
    kill: (signal: ChildProcess.Signal = "SIGTERM") => {
      Effect.runFork(handle.kill({ forceKillAfter: "1500 millis", killSignal: signal }).pipe(Effect.ignore))
      return true
    },
    pid: handle.pid,
    stderr,
    stdin: {
      write: (chunk: string | Uint8Array) => {
        Effect.runFork(handle.writeStdin(chunk).pipe(Effect.ignore))
        return true
      },
    },
    stdout,
    unref: () => {
      Effect.runFork(handle.unref.pipe(Effect.ignore))
    },
  }) as KhalaProcessNodeChild
}

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
