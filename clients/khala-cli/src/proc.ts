import { spawn as nodeSpawn } from "node:child_process"

// Cross-runtime process helpers.
//
// The Khala CLI is published to npm and must run for ANY new user, whether they
// launch it with `node` (the common case after `npm i -g`) or `bun`. Bun-only
// globals (`Bun.spawn`, `Bun.stdin`, `Bun.env`, `Bun.argv`) crash under node
// with "Bun is not defined", so every process/stdio touchpoint goes through
// these node:child_process-based helpers, which work identically under both
// runtimes.

export type ProcStdio = "inherit" | "ignore" | "pipe"

export interface SpawnHandle {
  readonly exited: Promise<number>
  readonly stdout: Promise<string>
  readonly stderr: Promise<string>
  unref(): void
}

export interface SpawnProcessOptions {
  readonly cwd?: string | undefined
  readonly env?: Record<string, string | undefined> | undefined
  readonly stdin?: ProcStdio | undefined
  readonly stdout?: ProcStdio | undefined
  readonly stderr?: ProcStdio | undefined
  readonly detached?: boolean | undefined
}

// Spawn a child process. Never throws and never produces an unhandled rejection:
// a failure to spawn (missing binary, etc.) resolves `exited` with a non-zero
// code so callers can branch on it the same way they branch on a real exit code.
export function spawnProcess(
  command: ReadonlyArray<string>,
  options: SpawnProcessOptions = {},
): SpawnHandle {
  const [cmd, ...args] = command
  if (cmd === undefined) {
    return {
      exited: Promise.resolve(127),
      stdout: Promise.resolve(""),
      stderr: Promise.resolve("empty command"),
      unref: () => {},
    }
  }

  const stdio: [ProcStdio, ProcStdio, ProcStdio] = [
    options.stdin ?? "ignore",
    options.stdout ?? "ignore",
    options.stderr ?? "ignore",
  ]

  let child: ReturnType<typeof nodeSpawn>
  try {
    child = nodeSpawn(cmd, args, {
      ...(options.cwd === undefined ? {} : { cwd: options.cwd }),
      ...(options.env === undefined ? {} : { env: options.env as NodeJS.ProcessEnv }),
      ...(options.detached === undefined ? {} : { detached: options.detached }),
      stdio,
    })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return {
      exited: Promise.resolve(127),
      stdout: Promise.resolve(""),
      stderr: Promise.resolve(message),
      unref: () => {},
    }
  }

  const collect = (stream: NodeJS.ReadableStream | null): Promise<string> => {
    if (stream === null) return Promise.resolve("")
    return new Promise<string>((resolve) => {
      let buffer = ""
      stream.setEncoding("utf8")
      stream.on("data", (chunk: string) => {
        buffer += chunk
      })
      stream.on("end", () => resolve(buffer))
      stream.on("error", () => resolve(buffer))
    })
  }

  const stdout = collect(child.stdout)
  const stderr = collect(child.stderr)
  const exited = new Promise<number>((resolve) => {
    child.on("error", () => resolve(127))
    child.on("close", (code) => resolve(code ?? 0))
  })

  return {
    exited,
    stdout,
    stderr,
    unref: () => {
      try {
        child.unref()
      } catch {
        // Some runtimes do not expose unref on every handle; ignore.
      }
    },
  }
}

// Read all of stdin as UTF-8 text. Works under both node and bun.
export async function readStdinText(): Promise<string> {
  const chunks: Array<Buffer> = []
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk as Uint8Array))
  }
  return Buffer.concat(chunks).toString("utf8")
}
