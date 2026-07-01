import { mkdtemp, realpath, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import { KhalaToolRuntimeError } from "./index.js"
import { makeKhalaToolRuntimeService, type KhalaToolRuntimeServiceShape } from "./runtime.js"
import type {
  KhalaProcessEvent,
  KhalaProcessExecInput,
  KhalaProcessExecResult,
  KhalaProcessOutputChannel,
  KhalaProcessService,
  KhalaProcessSessionResult,
  KhalaProcessSessionStartInput,
} from "./index.js"

export type MacosSeatbeltProcessServiceOptions = Readonly<{
  deniedReadPaths?: ReadonlyArray<string>
  fallback?: KhalaProcessService
  runtime?: KhalaToolRuntimeServiceShape
  sandboxExecPath?: string
}>

const sandboxNote = "macOS Seatbelt sandbox enforced by sandbox-exec; writes are limited to the workspace."

export function createMacosSeatbeltKhalaProcessService(
  options: MacosSeatbeltProcessServiceOptions = {},
): KhalaProcessService {
  const sessions = new Map<string, MacosSeatbeltProcessSession>()
  const sandboxExecPath = options.sandboxExecPath ?? "/usr/bin/sandbox-exec"
  const fallback = options.fallback
  const runtime = options.runtime ?? makeKhalaToolRuntimeService()

  return {
    execCommand: input =>
      Effect.gen(function* () {
        if (process.platform !== "darwin") {
          if (fallback !== undefined) return yield* fallback.execCommand(input)
          return yield* processRuntimeError("process_sandbox_unavailable", "macOS Seatbelt sandbox is only available on Darwin")
        }
        return yield* runSandboxedCommand(input, sandboxExecPath, options.deniedReadPaths ?? [], runtime)
      }),
    marker: "khala.process_service",
    startSession: input =>
      Effect.gen(function* () {
        if (process.platform !== "darwin") {
          if (fallback !== undefined) return yield* fallback.startSession(input)
          return yield* processRuntimeError("process_sandbox_unavailable", "macOS Seatbelt sandbox is only available on Darwin")
        }
        return yield* startSandboxedSession(input, sandboxExecPath, options.deniedReadPaths ?? [], sessions, runtime)
      }),
    writeStdin: input =>
      Effect.gen(function* () {
        const session = sessions.get(input.sessionId)
        if (session === undefined) {
          if (fallback !== undefined) return yield* fallback.writeStdin(input)
          return yield* processRuntimeError("process_session_unknown", `unknown process session: ${input.sessionId}`)
        }
        if (session.khalaSessionId !== input.khalaSessionId) {
          return yield* processRuntimeError("process_session_mismatch", "process session does not belong to the active Khala session")
        }
        if (input.chars !== undefined && input.chars.length > 0) {
          if (session.exitCode !== null) {
            return yield* processRuntimeError("process_session_closed", `process session is closed: ${input.sessionId}`)
          }
          if (input.chars === "\u0003") {
            session.cancelled = true
            killSandboxedProcessGroup(session.proc)
          } else {
            const stdin = session.proc.stdin as unknown as BunFileSink | undefined
            if (stdin === undefined) {
              return yield* processRuntimeError("process_session_stdin_closed", `process session stdin is closed: ${input.sessionId}`)
            }
            stdin.write(input.chars)
            stdin.flush()
          }
          session.events.push({ channel: "stdin", text: input.chars, timestampMs: yield* runtime.currentTimeMillis })
        }
        yield* runtime.sleep(input.yieldTimeMs)
        return sessionSnapshot(session, (yield* runtime.currentTimeMillis) - session.createdAtMs)
      }),
  }
}

function runSandboxedCommand(
  input: KhalaProcessExecInput,
  sandboxExecPath: string,
  deniedReadPaths: ReadonlyArray<string>,
  runtime: KhalaToolRuntimeServiceShape,
): Effect.Effect<KhalaProcessExecResult, KhalaToolRuntimeError> {
  return Effect.scoped(Effect.gen(function* () {
    const started = yield* runtime.currentTimeMillis
    let timedOut = false
    let cancelled = false
    const sandbox = yield* Effect.acquireRelease(
      Effect.tryPromise({
        catch: error => processRuntimeFailure("process_sandbox_profile_failed", error),
        try: () => makeSandboxArgs(input, sandboxExecPath, deniedReadPaths),
      }),
      sandbox => Effect.promise(() => sandbox.cleanup()).pipe(Effect.orDie),
    )
    const proc = yield* Effect.acquireRelease(
      Effect.sync(() => Bun.spawn([...sandbox.prefix, ...commandArgs(input)], {
        cwd: input.cwd,
        detached: true,
        stderr: "pipe",
        stdout: "pipe",
      })),
      proc => Effect.sync(() => killSandboxedProcessGroup(proc)),
    )
    const kill = (reason: "cancelled" | "timedOut"): void => {
      if (reason === "cancelled") cancelled = true
      else timedOut = true
      killSandboxedProcessGroup(proc)
    }
    yield* Effect.forkScoped(runtime.sleep(input.timeoutMs).pipe(Effect.tap(() => Effect.sync(() => kill("timedOut")))))
    const cancel = input.cancelAfterMs === undefined
      ? undefined
      : yield* Effect.forkScoped(runtime.sleep(input.cancelAfterMs).pipe(Effect.tap(() => Effect.sync(() => kill("cancelled")))))
    void cancel
    const events: KhalaProcessEvent[] = []
    const [stdout, stderr, exitCode] = yield* Effect.tryPromise({
      catch: error => processRuntimeFailure("process_exec_failed", error),
      try: () => Promise.all([
        readProcessStream(proc.stdout, "stdout", input.maxCaptureBytes, events, () => killSandboxedProcessGroup(proc), runtime),
        readProcessStream(proc.stderr, "stderr", input.maxCaptureBytes, events, () => killSandboxedProcessGroup(proc), runtime),
        proc.exited.catch(() => null),
      ]),
    })
    return {
      cancelled,
      durationMs: (yield* runtime.currentTimeMillis) - started,
      events: events.sort((a, b) => a.timestampMs - b.timestampMs),
      exitCode,
      sandbox: {
        enforced: true,
        kind: "external",
        note: sandboxNote,
      },
      signal: null,
      stderr: stderr.text,
      stderrTruncated: stderr.truncated,
      stdout: stdout.text,
      stdoutTruncated: stdout.truncated,
      timedOut,
    }
  }))
}

function startSandboxedSession(
  input: KhalaProcessSessionStartInput,
  sandboxExecPath: string,
  deniedReadPaths: ReadonlyArray<string>,
  sessions: Map<string, MacosSeatbeltProcessSession>,
  runtime: KhalaToolRuntimeServiceShape,
): Effect.Effect<KhalaProcessSessionResult, KhalaToolRuntimeError> {
  return Effect.gen(function* () {
  const started = yield* runtime.currentTimeMillis
  const sandbox = yield* Effect.tryPromise({
    catch: error => processRuntimeFailure("process_sandbox_profile_failed", error),
    try: () => makeSandboxArgs(input, sandboxExecPath, deniedReadPaths),
  })
  const proc = Bun.spawn([...sandbox.prefix, ...commandArgs(input)], {
    cwd: input.cwd,
    detached: true,
    stderr: "pipe",
    stdin: "pipe",
    stdout: "pipe",
  })
  const sessionId = yield* runtime.eventId("khala.proc")
  const session: MacosSeatbeltProcessSession = {
    cancelled: false,
    cleanup: sandbox.cleanup,
    command: input.command,
    createdAtMs: started,
    events: [],
    exitCode: null,
    khalaSessionId: input.khalaSessionId,
    maxCaptureBytes: input.maxCaptureBytes,
    proc,
    sessionId,
    stderr: "",
    stderrTruncated: false,
    stdout: "",
    stdoutTruncated: false,
    timedOut: false,
  }
  sessions.set(sessionId, session)
  void pumpSessionStream(session, proc.stdout, "stdout", runtime)
  void pumpSessionStream(session, proc.stderr, "stderr", runtime)
  void proc.exited.then(exitCode => {
    session.exitCode = exitCode
    void session.cleanup()
  }).catch(() => {
    session.exitCode = null
    void session.cleanup()
  })
  yield* Effect.forkDetach(runtime.sleep(input.timeoutMs).pipe(Effect.tap(() => Effect.sync(() => {
    if (session.exitCode === null) {
      session.timedOut = true
      killSandboxedProcessGroup(proc)
    }
  }))))
  yield* runtime.sleep(input.yieldTimeMs)
  return sessionSnapshot(session, (yield* runtime.currentTimeMillis) - started)
  })
}

async function makeSandboxArgs(
  input: KhalaProcessExecInput,
  sandboxExecPath: string,
  deniedReadPaths: ReadonlyArray<string>,
): Promise<Readonly<{ cleanup: () => Promise<void>; prefix: ReadonlyArray<string> }>> {
  const workspaceRoot = await realpath(input.workspaceRoot ?? input.cwd)
  const deniedReads = await Promise.all(deniedReadPaths.map(path => realpath(path)))
  const dir = await mkdtemp(join(tmpdir(), "khala-seatbelt-"))
  const profilePath = join(dir, "profile.sb")
  await writeFile(profilePath, seatbeltProfile(workspaceRoot, deniedReads), "utf8")
  return {
    cleanup: () => rm(dir, { force: true, recursive: true }),
    prefix: [sandboxExecPath, "-f", profilePath],
  }
}

export function seatbeltProfile(workspaceRoot: string, deniedReadPaths: ReadonlyArray<string> = []): string {
  const denied = deniedReadPaths.map(path => `(deny file-read* (subpath ${quoteSeatbeltString(path)}))`)
  return [
    "(version 1)",
    "(deny default)",
    "(allow process*)",
    "(allow signal (target same-sandbox))",
    "(allow sysctl-read)",
    "(allow mach-lookup)",
    "(allow file-read*)",
    ...denied,
    `(allow file-write* (subpath ${quoteSeatbeltString(workspaceRoot)}))`,
    '(allow file-write* (literal "/dev/null"))',
    "",
  ].join("\n")
}

function commandArgs(input: KhalaProcessExecInput): ReadonlyArray<string> {
  return input.argv !== undefined && input.argv.length > 0
    ? [...input.argv]
    : [input.shell ?? process.env.SHELL ?? "/bin/sh", "-lc", input.command]
}

function quoteSeatbeltString(value: string): string {
  return JSON.stringify(value)
}

function processRuntimeError(code: string, reason: string): Effect.Effect<never, KhalaToolRuntimeError> {
  return Effect.fail(new KhalaToolRuntimeError({ code, reason }))
}

function processRuntimeFailure(code: string, error: unknown): KhalaToolRuntimeError {
  return new KhalaToolRuntimeError({
    code,
    reason: error instanceof Error ? error.message : String(error),
  })
}

type MacosSeatbeltProcessSession = {
  cancelled: boolean
  cleanup: () => Promise<void>
  command: string
  createdAtMs: number
  events: KhalaProcessEvent[]
  exitCode: number | null
  khalaSessionId: string
  maxCaptureBytes: number
  proc: ReturnType<typeof Bun.spawn>
  sessionId: string
  stderr: string
  stderrTruncated: boolean
  stdout: string
  stdoutTruncated: boolean
  timedOut: boolean
}

type BunFileSink = Readonly<{
  flush: () => void
  write: (value: string | Uint8Array) => unknown
}>

function sessionSnapshot(session: MacosSeatbeltProcessSession, durationMs: number): KhalaProcessSessionResult {
  return {
    cancelled: session.cancelled,
    durationMs,
    events: [...session.events].sort((a, b) => a.timestampMs - b.timestampMs),
    exitCode: session.exitCode,
    sandbox: {
      enforced: true,
      kind: "external",
      note: sandboxNote,
    },
    sessionId: session.sessionId,
    signal: null,
    stderr: session.stderr,
    stderrTruncated: session.stderrTruncated,
    stdout: session.stdout,
    stdoutTruncated: session.stdoutTruncated,
    timedOut: session.timedOut,
  }
}

async function pumpSessionStream(
  session: MacosSeatbeltProcessSession,
  stream: ReadableStream<Uint8Array>,
  channel: "stdout" | "stderr",
  runtime: KhalaToolRuntimeServiceShape = makeKhalaToolRuntimeService(),
): Promise<void> {
  const reader = stream.getReader()
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = Buffer.from(read.value).toString("utf8")
    session.events.push({ channel, text: chunk, timestampMs: await Effect.runPromise(runtime.currentTimeMillis) })
    if (channel === "stdout") {
      if (Buffer.byteLength(session.stdout + chunk, "utf8") > session.maxCaptureBytes) {
        session.stdoutTruncated = true
        session.stdout = tailByBytes(`${session.stdout}${chunk}`, session.maxCaptureBytes)
      } else {
        session.stdout += chunk
      }
    } else if (Buffer.byteLength(session.stderr + chunk, "utf8") > session.maxCaptureBytes) {
      session.stderrTruncated = true
      session.stderr = tailByBytes(`${session.stderr}${chunk}`, session.maxCaptureBytes)
    } else {
      session.stderr += chunk
    }
  }
}

async function readProcessStream(
  stream: ReadableStream<Uint8Array>,
  channel: KhalaProcessOutputChannel,
  maxBytes: number,
  events: KhalaProcessEvent[],
  onTruncate: () => void,
  runtime: KhalaToolRuntimeServiceShape = makeKhalaToolRuntimeService(),
): Promise<Readonly<{ text: string; truncated: boolean }>> {
  const reader = stream.getReader()
  const chunks: Buffer[] = []
  let bytes = 0
  let truncated = false
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = read.value
    const remaining = maxBytes - bytes
    if (remaining <= 0 || chunk.byteLength > remaining) {
      if (remaining > 0) {
        const kept = chunk.slice(0, remaining)
        chunks.push(Buffer.from(kept))
        events.push({ channel, text: Buffer.from(kept).toString("utf8"), timestampMs: await Effect.runPromise(runtime.currentTimeMillis) })
      }
      truncated = true
      onTruncate()
      break
    }
    bytes += chunk.byteLength
    chunks.push(Buffer.from(chunk))
    events.push({ channel, text: Buffer.from(chunk).toString("utf8"), timestampMs: await Effect.runPromise(runtime.currentTimeMillis) })
  }
  return {
    text: Buffer.concat(chunks).toString("utf8"),
    truncated,
  }
}

function tailByBytes(text: string, maxBytes: number): string {
  const bytes = Buffer.from(text, "utf8")
  if (bytes.byteLength <= maxBytes) return text
  return bytes.subarray(bytes.byteLength - maxBytes).toString("utf8")
}

function killSandboxedProcessGroup(proc: ReturnType<typeof Bun.spawn>, signal: NodeJS.Signals = "SIGTERM"): void {
  try {
    process.kill(-proc.pid, signal)
  } catch {
    proc.kill(signal)
  }
}
