// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { execFile, spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { Duration, Effect, Scope } from "effect"
import type { WorkerAuthStatus } from "../shared/api.ts"
import {
  encodeAnswerLine,
  encodeInterruptLine,
  encodeSteerLine,
  encodeStopLine,
  encodeTaskLine,
  parseWorkerLine,
  type TaskWireOptions,
  type WorkerEvent
} from "../shared/wire.ts"

export const DEFAULT_OA_WORKER_BIN =
  "/Applications/OpenAgents.app/Contents/Resources/oa-worker"

export const resolveWorkerBin = (): string =>
  process.env["OA_WORKER_BIN"] && process.env["OA_WORKER_BIN"].length > 0
    ? process.env["OA_WORKER_BIN"]
    : DEFAULT_OA_WORKER_BIN

export class WorkerProcessError {
  readonly _tag = "WorkerProcessError"
  readonly message: string
  constructor(message: string) {
    this.message = message
  }
}

export interface OaWorkerHandle {
  readonly pid: number | null
  readonly sendTask: (opts: TaskWireOptions) => Effect.Effect<void, WorkerProcessError>
  readonly sendAnswer: (opts: {
    readonly answer: boolean
    readonly ticket: number
    readonly why?: string | undefined
    readonly option?: string | undefined
  }) => Effect.Effect<void, WorkerProcessError>
  readonly sendSteer: (opts: {
    readonly id: string
    readonly steer: string
    readonly turn?: number | undefined
  }) => Effect.Effect<void, WorkerProcessError>
  readonly sendStop: () => Effect.Effect<void, WorkerProcessError>
  readonly sendInterrupt: (resubmit?: boolean) => Effect.Effect<void, WorkerProcessError>
  readonly stderrTail: () => string
}

export const probeWorkerAuth = (
  binary = resolveWorkerBin()
): Effect.Effect<WorkerAuthStatus> =>
  Effect.callback<WorkerAuthStatus>((resume) => {
    const child = execFile(
      binary,
      ["--auth"],
      {
        timeout: 10_000,
        maxBuffer: 256 * 1024,
        env: { ...process.env, SSH_CONNECTION: "1" }
      },
      (err, stdout, stderr) => {
        if (err) {
          resume(
            Effect.succeed({
              binary,
              source: "error",
              ready: false,
              label: (stderr || err.message || "oa-worker --auth failed").trim().slice(0, 300)
            })
          )
          return
        }
        try {
          const parsed = JSON.parse(stdout.trim()) as Record<string, unknown>
          resume(
            Effect.succeed({
              binary,
              source: typeof parsed["source"] === "string" ? parsed["source"] : "unknown",
              ready: parsed["ready"] === true,
              label: typeof parsed["label"] === "string" ? parsed["label"] : ""
            })
          )
        } catch {
          resume(
            Effect.succeed({
              binary,
              source: "parse_error",
              ready: false,
              label: stdout.trim().slice(0, 300)
            })
          )
        }
      }
    )
    return Effect.sync(() => {
      child.kill("SIGTERM")
    })
  })

const writeLine = (
  child: ChildProcessWithoutNullStreams,
  line: string
): Effect.Effect<void, WorkerProcessError> =>
  Effect.callback((resume) => {
    if (child.killed || child.exitCode !== null || child.stdin.destroyed || !child.stdin.writable) {
      resume(Effect.fail(new WorkerProcessError("oa-worker stdin is closed")))
      return
    }
    child.stdin.write(line, "utf8", (err) => {
      if (err) {
        resume(Effect.fail(new WorkerProcessError(`oa-worker write failed: ${err.message}`)))
      } else {
        resume(Effect.void)
      }
    })
  })

const stopChildProcess = (child: ChildProcessWithoutNullStreams): Effect.Effect<void> =>
  Effect.callback<void>((resume) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resume(Effect.void)
      return
    }
    let settled = false
    const finish = () => {
      if (settled) return
      settled = true
      clearTimeout(termTimer)
      clearTimeout(killTimer)
      resume(Effect.void)
    }
    child.once("exit", finish)
    try {
      if (!child.stdin.destroyed && child.stdin.writable) {
        child.stdin.write(encodeStopLine(), "utf8")
        child.stdin.end()
      }
    } catch {
      // ignore write errors during shutdown
    }
    const termTimer = setTimeout(() => {
      if (child.exitCode === null && child.signalCode === null) {
        try {
          child.kill("SIGTERM")
        } catch {
          // ignore
        }
      }
    }, 300)
    const killTimer = setTimeout(() => {
      if (child.exitCode === null && child.signalCode === null) {
        try {
          child.kill("SIGKILL")
        } catch {
          // ignore
        }
      }
      finish()
    }, 2500)
  })

export const spawnOaWorker = (opts: {
  readonly binary?: string | undefined
  readonly cwd: string
  readonly permissionMode?: string | undefined
  readonly onEvent: (event: WorkerEvent) => void
  readonly onExit: (code: number | null, signal: NodeJS.Signals | null, stderr: string) => void
}): Effect.Effect<OaWorkerHandle, WorkerProcessError, Scope.Scope> =>
  Effect.acquireRelease(
    Effect.gen(function*() {
      const binary = opts.binary ?? resolveWorkerBin()
      let stderrBuf = ""
      let stdoutBuf = ""
      let readySettled = false
      let resolveReady: (() => void) | null = null
      let rejectReady: ((err: WorkerProcessError) => void) | null = null

      const readyPromise = new Promise<void>((resolve, reject) => {
        resolveReady = resolve
        rejectReady = reject
      })

      const permissionMode =
        opts.permissionMode ?? process.env["OA_AGENT_PERMISSION_MODE"] ?? "default"

      const child = spawn(binary, [], {
        cwd: opts.cwd,
        stdio: ["pipe", "pipe", "pipe"],
        env: {
          ...process.env,
          SSH_CONNECTION: "1",
          OA_AGENT_PERMISSION_MODE: permissionMode
        }
      })

      child.stderr.setEncoding("utf8")
      child.stderr.on("data", (chunk: string) => {
        stderrBuf = (stderrBuf + chunk).slice(-8192)
      })

      child.stdout.setEncoding("utf8")
      child.stdout.on("data", (chunk: string) => {
        stdoutBuf += chunk
        let idx = stdoutBuf.indexOf("\n")
        while (idx !== -1) {
          const line = stdoutBuf.slice(0, idx)
          stdoutBuf = stdoutBuf.slice(idx + 1)
          const ev = parseWorkerLine(line)
          if (ev !== null) {
            if (ev.kind === "ready" && !readySettled) {
              readySettled = true
              resolveReady?.()
            }
            opts.onEvent(ev)
          }
          idx = stdoutBuf.indexOf("\n")
        }
      })

      child.on("error", (err) => {
        if (!readySettled) {
          readySettled = true
          rejectReady?.(new WorkerProcessError(`Failed to spawn ${binary}: ${err.message}`))
        }
        opts.onExit(null, null, `${stderrBuf}\n${err.message}`.trim())
      })

      child.on("exit", (code, signal) => {
        if (!readySettled) {
          readySettled = true
          rejectReady?.(
            new WorkerProcessError(
              `oa-worker exited before ready (code=${String(code)}, signal=${String(signal)}): ${stderrBuf.trim()}`
            )
          )
        }
        opts.onExit(code, signal, stderrBuf.trim())
      })

      yield* Effect.tryPromise({
        try: () => readyPromise,
        catch: (e) =>
          e instanceof WorkerProcessError
            ? e
            : new WorkerProcessError(`oa-worker startup error: ${String(e)}`)
      }).pipe(
        Effect.timeoutOrElse({
          duration: Duration.seconds(15),
          orElse: () =>
            Effect.andThen(
              stopChildProcess(child),
              Effect.fail(new WorkerProcessError("Timed out waiting 15s for oa-worker ready frame"))
            )
        })
      )

      const handle: OaWorkerHandle = {
        pid: child.pid ?? null,
        sendTask: (taskOpts) => writeLine(child, encodeTaskLine(taskOpts)),
        sendAnswer: (ans) => writeLine(child, encodeAnswerLine(ans)),
        sendSteer: (steer) => writeLine(child, encodeSteerLine(steer)),
        sendStop: () => writeLine(child, encodeStopLine()),
        sendInterrupt: (resubmit = false) => writeLine(child, encodeInterruptLine(resubmit)),
        stderrTail: () => stderrBuf.trim()
      }

      return { child, handle }
    }),
    ({ child }) => stopChildProcess(child)
  ).pipe(Effect.map(({ handle }) => handle))
