import { mkdtemp, realpath, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
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
  sandboxExecPath?: string
}>

const sandboxNote = "macOS Seatbelt sandbox enforced by sandbox-exec; writes are limited to the workspace."

export function createMacosSeatbeltKhalaProcessService(
  options: MacosSeatbeltProcessServiceOptions = {},
): KhalaProcessService {
  const sessions = new Map<string, MacosSeatbeltProcessSession>()
  const sandboxExecPath = options.sandboxExecPath ?? "/usr/bin/sandbox-exec"
  const fallback = options.fallback

  return {
    execCommand: input =>
      Effect.promise(async () => {
        if (process.platform !== "darwin") {
          if (fallback !== undefined) return await Effect.runPromise(fallback.execCommand(input))
          throw new Error("macOS Seatbelt sandbox is only available on Darwin")
        }
        return runSandboxedCommand(input, sandboxExecPath, options.deniedReadPaths ?? [])
      }),
    marker: "khala.process_service",
    startSession: input =>
      Effect.promise(async () => {
        if (process.platform !== "darwin") {
          if (fallback !== undefined) return await Effect.runPromise(fallback.startSession(input))
          throw new Error("macOS Seatbelt sandbox is only available on Darwin")
        }
        return startSandboxedSession(input, sandboxExecPath, options.deniedReadPaths ?? [], sessions)
      }),
    writeStdin: input =>
      Effect.promise(async () => {
        const session = sessions.get(input.sessionId)
        if (session === undefined) {
          if (fallback !== undefined) return await Effect.runPromise(fallback.writeStdin(input))
          throw new Error(`unknown process session: ${input.sessionId}`)
        }
        if (session.khalaSessionId !== input.khalaSessionId) {
          throw new Error("process session does not belong to the active Khala session")
        }
        if (input.chars !== undefined && input.chars.length > 0) {
          if (session.exitCode !== null) throw new Error(`process session is closed: ${input.sessionId}`)
          if (input.chars === "\u0003") {
            session.cancelled = true
            session.proc.kill()
          } else {
            const stdin = session.proc.stdin as unknown as BunFileSink | undefined
            if (stdin === undefined) throw new Error(`process session stdin is closed: ${input.sessionId}`)
            stdin.write(input.chars)
            stdin.flush()
          }
          session.events.push({ channel: "stdin", text: input.chars, timestampMs: Date.now() })
        }
        await sleep(input.yieldTimeMs)
        return sessionSnapshot(session, Date.now() - session.createdAtMs)
      }),
  }
}

async function runSandboxedCommand(
  input: KhalaProcessExecInput,
  sandboxExecPath: string,
  deniedReadPaths: ReadonlyArray<string>,
): Promise<KhalaProcessExecResult> {
  const started = Date.now()
  let timedOut = false
  let cancelled = false
  const sandbox = await makeSandboxArgs(input, sandboxExecPath, deniedReadPaths)
  try {
    const proc = Bun.spawn([...sandbox.prefix, ...commandArgs(input)], {
      cwd: input.cwd,
      stderr: "pipe",
      stdout: "pipe",
    })
    const kill = (reason: "cancelled" | "timedOut"): void => {
      if (reason === "cancelled") cancelled = true
      else timedOut = true
      proc.kill()
    }
    const timeout = setTimeout(() => kill("timedOut"), input.timeoutMs)
    const cancel = input.cancelAfterMs === undefined
      ? undefined
      : setTimeout(() => kill("cancelled"), input.cancelAfterMs)
    const events: KhalaProcessEvent[] = []
    const [stdout, stderr, exitCode] = await Promise.all([
      readProcessStream(proc.stdout, "stdout", input.maxCaptureBytes, events, () => proc.kill()),
      readProcessStream(proc.stderr, "stderr", input.maxCaptureBytes, events, () => proc.kill()),
      proc.exited.catch(() => null),
    ])
    clearTimeout(timeout)
    if (cancel !== undefined) clearTimeout(cancel)
    return {
      cancelled,
      durationMs: Date.now() - started,
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
  } finally {
    await sandbox.cleanup()
  }
}

async function startSandboxedSession(
  input: KhalaProcessSessionStartInput,
  sandboxExecPath: string,
  deniedReadPaths: ReadonlyArray<string>,
  sessions: Map<string, MacosSeatbeltProcessSession>,
): Promise<KhalaProcessSessionResult> {
  const started = Date.now()
  const sandbox = await makeSandboxArgs(input, sandboxExecPath, deniedReadPaths)
  const proc = Bun.spawn([...sandbox.prefix, ...commandArgs(input)], {
    cwd: input.cwd,
    stderr: "pipe",
    stdin: "pipe",
    stdout: "pipe",
  })
  const sessionId = `khala.proc.${Date.now().toString(36)}.${Math.random().toString(36).slice(2)}`
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
  void pumpSessionStream(session, proc.stdout, "stdout")
  void pumpSessionStream(session, proc.stderr, "stderr")
  void proc.exited.then(exitCode => {
    session.exitCode = exitCode
    void session.cleanup()
  }).catch(() => {
    session.exitCode = null
    void session.cleanup()
  })
  setTimeout(() => {
    if (session.exitCode === null) {
      session.timedOut = true
      proc.kill()
    }
  }, input.timeoutMs)
  await sleep(input.yieldTimeMs)
  return sessionSnapshot(session, Date.now() - started)
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
): Promise<void> {
  const reader = stream.getReader()
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = Buffer.from(read.value).toString("utf8")
    session.events.push({ channel, text: chunk, timestampMs: Date.now() })
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
        events.push({ channel, text: Buffer.from(kept).toString("utf8"), timestampMs: Date.now() })
      }
      truncated = true
      onTruncate()
      break
    }
    bytes += chunk.byteLength
    chunks.push(Buffer.from(chunk))
    events.push({ channel, text: Buffer.from(chunk).toString("utf8"), timestampMs: Date.now() })
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

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}
