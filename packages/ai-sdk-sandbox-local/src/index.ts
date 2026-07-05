import { spawn as spawnChild, type ChildProcessByStdio } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, resolve, sep } from "node:path"
import type { Readable } from "node:stream"
import { fileURLToPath } from "node:url"
import type {
  HarnessV1NetworkPolicy,
  HarnessV1NetworkSandboxSession,
  HarnessV1SandboxProvider,
} from "@ai-sdk/harness"
import {
  extractLines,
  type Experimental_SandboxProcess,
  type Experimental_SandboxSession,
} from "@ai-sdk/provider-utils"

const DEFAULT_PROVIDER_ID = "openagents-local-sandbox"

export type LocalAiSdkSandboxAccountHomes = Readonly<{
  home: string
  codexHome: string
  claudeConfigDir: string
}>

export type LocalAiSdkSandboxProviderOptions = Readonly<{
  providerId?: string
  rootDirectory?: string
  defaultPorts?: ReadonlyArray<number>
  defaultNetworkPolicy?: HarnessV1NetworkPolicy
  env?: Readonly<Record<string, string>>
  accountHomes?: Partial<LocalAiSdkSandboxAccountHomes>
}>

export function createLocalAiSdkSandboxProvider(
  options: LocalAiSdkSandboxProviderOptions = {},
): HarnessV1SandboxProvider {
  return new LocalAiSdkSandboxProvider(options)
}

export class LocalAiSdkSandboxProvider implements HarnessV1SandboxProvider {
  readonly specificationVersion = "harness-sandbox-v1" as const
  readonly providerId: string
  private readonly options: LocalAiSdkSandboxProviderOptions
  private readonly sessions = new Map<string, LocalAiSdkNetworkSandboxSession>()

  constructor(options: LocalAiSdkSandboxProviderOptions = {}) {
    this.providerId = options.providerId ?? DEFAULT_PROVIDER_ID
    this.options = options
  }

  createSession = async (options: {
    sessionId?: string
    abortSignal?: AbortSignal
    identity?: string
    onFirstCreate?: (
      session: Experimental_SandboxSession,
      opts: { abortSignal?: AbortSignal },
    ) => Promise<void>
  } = {}): Promise<HarnessV1NetworkSandboxSession> => {
    options.abortSignal?.throwIfAborted()
    const sessionId = options.sessionId ?? `local-${randomUUID()}`
    const existing = this.sessions.get(sessionId)
    if (existing !== undefined) return existing

    const workspaceRoot =
      this.options.rootDirectory === undefined
        ? await mkdtemp(resolve(tmpdir(), "openagents-ai-sdk-sandbox-"))
        : resolve(
            this.options.rootDirectory,
            stableWorkspaceName({
              sessionId,
              ...(options.identity === undefined
                ? {}
                : { identity: options.identity }),
            }),
          )
    await mkdir(workspaceRoot, { recursive: true })

    const homes = resolveLocalAccountHomes({
      workspaceRoot,
      ...(this.options.accountHomes === undefined
        ? {}
        : { accountHomes: this.options.accountHomes }),
    })
    await Promise.all([
      mkdir(homes.home, { recursive: true }),
      mkdir(homes.codexHome, { recursive: true }),
      mkdir(homes.claudeConfigDir, { recursive: true }),
    ])

    const session = new LocalAiSdkNetworkSandboxSession({
      accountHomes: homes,
      defaultNetworkPolicy: this.options.defaultNetworkPolicy ?? {
        mode: "allow-all",
      },
      env: this.options.env ?? {},
      id: sessionId,
      ownsWorkspaceRoot: true,
      ports: this.options.defaultPorts ?? [],
      providerId: this.providerId,
      workspaceRoot,
    })
    this.sessions.set(sessionId, session)

    if (options.onFirstCreate !== undefined) {
      await options.onFirstCreate(
        session.restricted(),
        optionalAbortSignal(options.abortSignal),
      )
    }

    return session
  }

  resumeSession = async (options: {
    sessionId: string
    abortSignal?: AbortSignal
  }): Promise<HarnessV1NetworkSandboxSession> => {
    options.abortSignal?.throwIfAborted()
    const existing = this.sessions.get(options.sessionId)
    if (existing !== undefined) return existing
    if (this.options.rootDirectory === undefined) {
      throw new Error(
        "Local AI SDK sandbox resume requires a provider rootDirectory or an in-memory live session.",
      )
    }
    const workspaceRoot = resolve(this.options.rootDirectory, options.sessionId)
    const homes = resolveLocalAccountHomes({
      workspaceRoot,
      ...(this.options.accountHomes === undefined
        ? {}
        : { accountHomes: this.options.accountHomes }),
    })
    const session = new LocalAiSdkNetworkSandboxSession({
      accountHomes: homes,
      defaultNetworkPolicy: this.options.defaultNetworkPolicy ?? {
        mode: "allow-all",
      },
      env: this.options.env ?? {},
      id: options.sessionId,
      ownsWorkspaceRoot: true,
      ports: this.options.defaultPorts ?? [],
      providerId: this.providerId,
      workspaceRoot,
    })
    this.sessions.set(options.sessionId, session)
    return session
  }
}

export class LocalAiSdkNetworkSandboxSession
  implements HarnessV1NetworkSandboxSession
{
  readonly id: string
  readonly defaultWorkingDirectory: string
  private readonly accountHomes: LocalAiSdkSandboxAccountHomes
  private readonly env: Readonly<Record<string, string>>
  private readonly ownsWorkspaceRoot: boolean
  private readonly processes = new Set<LocalSandboxProcess>()
  private readonly providerId: string
  private networkPolicy: HarnessV1NetworkPolicy
  private exposedPorts: ReadonlyArray<number>
  private stopped = false

  constructor(input: Readonly<{
    accountHomes: LocalAiSdkSandboxAccountHomes
    defaultNetworkPolicy: HarnessV1NetworkPolicy
    env: Readonly<Record<string, string>>
    id: string
    ownsWorkspaceRoot: boolean
    ports: ReadonlyArray<number>
    providerId: string
    workspaceRoot: string
  }>) {
    this.accountHomes = input.accountHomes
    this.defaultWorkingDirectory = input.workspaceRoot
    this.env = input.env
    this.exposedPorts = [...input.ports]
    this.id = input.id
    this.networkPolicy = cloneNetworkPolicy(input.defaultNetworkPolicy)
    this.ownsWorkspaceRoot = input.ownsWorkspaceRoot
    this.providerId = input.providerId
  }

  get description(): string {
    return [
      `OpenAgents local AI SDK sandbox (${this.providerId}:${this.id}).`,
      `Workspace root: ${this.defaultWorkingDirectory}`,
      "Owner-local fixture only; this is not production containment.",
      `CODEX_HOME: ${this.accountHomes.codexHome}`,
      `CLAUDE_CONFIG_DIR: ${this.accountHomes.claudeConfigDir}`,
    ].join("\n")
  }

  get ports(): ReadonlyArray<number> {
    return [...this.exposedPorts]
  }

  restricted(): Experimental_SandboxSession {
    const session = this
    return {
      get description() {
        return session.description
      },
      readFile: (options) => session.readFile(options),
      readBinaryFile: (options) => session.readBinaryFile(options),
      readTextFile: (options) => session.readTextFile(options),
      writeFile: (options) => session.writeFile(options),
      writeBinaryFile: (options) => session.writeBinaryFile(options),
      writeTextFile: (options) => session.writeTextFile(options),
      spawn: (options) => session.spawn(options),
      run: (options) => session.run(options),
    }
  }

  getPortUrl = async (options: {
    port: number
    protocol?: "http" | "https" | "ws"
  }): Promise<string> => {
    this.assertRunning()
    if (!this.exposedPorts.includes(options.port)) {
      throw new Error(
        `Port ${options.port} is not exposed on local sandbox ${this.id}.`,
      )
    }
    const protocol = options.protocol === "ws" ? "ws" : "http"
    return `${protocol}://127.0.0.1:${options.port}/`
  }

  setNetworkPolicy = async (policy: HarnessV1NetworkPolicy): Promise<void> => {
    this.assertRunning()
    this.networkPolicy = cloneNetworkPolicy(policy)
  }

  setPorts = async (
    ports: ReadonlyArray<number>,
    options?: { abortSignal?: AbortSignal },
  ): Promise<void> => {
    options?.abortSignal?.throwIfAborted()
    this.assertRunning()
    this.exposedPorts = [...ports]
  }

  stop = async (): Promise<void> => {
    if (this.stopped) return
    this.stopped = true
    await Promise.all([...this.processes].map((process) => process.kill()))
  }

  destroy = async (): Promise<void> => {
    await this.stop()
    if (this.ownsWorkspaceRoot) {
      await rm(this.defaultWorkingDirectory, { force: true, recursive: true })
    }
  }

  async readFile(options: {
    path: string
    abortSignal?: AbortSignal
  }): Promise<ReadableStream<Uint8Array> | null> {
    const bytes = await this.readBinaryFile(options)
    if (bytes === null) return null
    return bytesToStream(bytes)
  }

  async readBinaryFile(options: {
    path: string
    abortSignal?: AbortSignal
  }): Promise<Uint8Array | null> {
    options.abortSignal?.throwIfAborted()
    this.assertRunning()
    const hostPath = this.resolvePath(options.path)
    try {
      return new Uint8Array(await readFile(hostPath))
    } catch (error) {
      if (isNodeFileNotFound(error)) return null
      throw error
    }
  }

  async readTextFile(options: {
    path: string
    encoding?: string
    startLine?: number
    endLine?: number
    abortSignal?: AbortSignal
  }): Promise<string | null> {
    const bytes = await this.readBinaryFile({
      path: options.path,
      ...optionalAbortSignal(options.abortSignal),
    })
    if (bytes === null) return null
    const text = Buffer.from(bytes).toString(
      (options.encoding ?? "utf-8") as BufferEncoding,
    )
    return extractLines({
      text,
      ...(options.startLine === undefined
        ? {}
        : { startLine: options.startLine }),
      ...(options.endLine === undefined ? {} : { endLine: options.endLine }),
    })
  }

  async writeFile(options: {
    path: string
    content: ReadableStream<Uint8Array>
    abortSignal?: AbortSignal
  }): Promise<void> {
    const content = await collectReadableStream(options.content)
    await this.writeBinaryFile({
      path: options.path,
      content,
      ...optionalAbortSignal(options.abortSignal),
    })
  }

  async writeBinaryFile(options: {
    path: string
    content: Uint8Array
    abortSignal?: AbortSignal
  }): Promise<void> {
    options.abortSignal?.throwIfAborted()
    this.assertRunning()
    const hostPath = this.resolvePath(options.path)
    await mkdir(dirname(hostPath), { recursive: true })
    await writeFile(hostPath, options.content)
  }

  async writeTextFile(options: {
    path: string
    content: string
    encoding?: string
    abortSignal?: AbortSignal
  }): Promise<void> {
    await this.writeBinaryFile({
      path: options.path,
      content: new Uint8Array(
        Buffer.from(options.content, (options.encoding ?? "utf-8") as BufferEncoding),
      ),
      ...optionalAbortSignal(options.abortSignal),
    })
  }

  async spawn(options: {
    command: string
    workingDirectory?: string
    env?: Record<string, string>
    abortSignal?: AbortSignal
  }): Promise<Experimental_SandboxProcess> {
    options.abortSignal?.throwIfAborted()
    this.assertRunning()
    const child = spawnChild("/bin/bash", ["-lc", options.command], {
      cwd:
        options.workingDirectory === undefined
          ? this.defaultWorkingDirectory
          : this.resolvePath(options.workingDirectory),
      detached: true,
      env: this.commandEnv(options.env),
      stdio: ["ignore", "pipe", "pipe"],
    })
    const process = new LocalSandboxProcess(child, options.abortSignal)
    this.processes.add(process)
    void process.wait().finally(() => {
      this.processes.delete(process)
    })
    return process
  }

  async run(options: {
    command: string
    workingDirectory?: string
    env?: Record<string, string>
    abortSignal?: AbortSignal
  }): Promise<{ exitCode: number; stdout: string; stderr: string }> {
    const process = await this.spawn(options)
    const [stdout, stderr, result] = await Promise.all([
      streamToText(process.stdout),
      streamToText(process.stderr),
      process.wait(),
    ])
    return {
      exitCode: result.exitCode,
      stderr,
      stdout,
    }
  }

  private commandEnv(env?: Record<string, string>): NodeJS.ProcessEnv {
    return {
      ...stringProcessEnv(process.env),
      ...this.env,
      ...(env ?? {}),
      CLAUDE_CONFIG_DIR: this.accountHomes.claudeConfigDir,
      CODEX_HOME: this.accountHomes.codexHome,
      HOME: this.accountHomes.home,
      OPENAGENTS_SANDBOX_NETWORK_POLICY: JSON.stringify(this.networkPolicy),
    }
  }

  private resolvePath(path: string): string {
    if (path.includes("\0")) {
      throw new Error("Sandbox paths cannot contain NUL bytes.")
    }
    const hostPath = path.startsWith(sep)
      ? resolve(path)
      : resolve(this.defaultWorkingDirectory, path)
    if (!isPathInside(this.defaultWorkingDirectory, hostPath)) {
      throw new Error(
        `Path ${path} escapes local sandbox workspace ${this.defaultWorkingDirectory}.`,
      )
    }
    return hostPath
  }

  private assertRunning(): void {
    if (this.stopped) {
      throw new Error(`Local sandbox ${this.id} is stopped.`)
    }
  }
}

class LocalSandboxProcess implements Experimental_SandboxProcess {
  readonly pid?: number
  readonly stderr: ReadableStream<Uint8Array>
  readonly stdout: ReadableStream<Uint8Array>
  private readonly child: ChildProcessByStdio<null, Readable, Readable>
  private readonly done: Promise<{ exitCode: number }>
  private killed = false

  constructor(
    child: ChildProcessByStdio<null, Readable, Readable>,
    abortSignal?: AbortSignal,
  ) {
    this.child = child
    if (child.pid !== undefined) this.pid = child.pid
    this.stderr = nodeReadableToWeb(child.stderr)
    this.stdout = nodeReadableToWeb(child.stdout)
    this.done = new Promise((resolveDone, rejectDone) => {
      child.once("error", rejectDone)
      child.once("close", (code, signal) => {
        if (abortSignal?.aborted) {
          rejectDone(abortSignal.reason ?? new DOMException("Aborted", "AbortError"))
          return
        }
        resolveDone({ exitCode: code ?? (signal === null ? 0 : 1) })
      })
    })
    if (abortSignal !== undefined) {
      abortSignal.addEventListener(
        "abort",
        () => {
          void this.kill()
        },
        { once: true },
      )
    }
  }

  wait(): Promise<{ exitCode: number }> {
    return this.done
  }

  async kill(): Promise<void> {
    if (this.killed) return
    this.killed = true
    if (this.child.pid !== undefined) {
      try {
        process.kill(-this.child.pid, "SIGTERM")
      } catch {
        this.child.kill("SIGTERM")
      }
    } else {
      this.child.kill("SIGTERM")
    }
    await Promise.race([
      this.done.catch(() => undefined),
      new Promise((resolveTimeout) => setTimeout(resolveTimeout, 500)),
    ])
    if (!this.child.killed && this.child.pid !== undefined) {
      try {
        process.kill(-this.child.pid, "SIGKILL")
      } catch {
        this.child.kill("SIGKILL")
      }
    }
  }
}

function stableWorkspaceName(input: Readonly<{
  identity?: string
  sessionId: string
}>): string {
  if (input.identity === undefined) return input.sessionId
  return createHash("sha256")
    .update(`${input.sessionId}:${input.identity}`)
    .digest("hex")
    .slice(0, 32)
}

function resolveLocalAccountHomes(input: Readonly<{
  accountHomes?: Partial<LocalAiSdkSandboxAccountHomes>
  workspaceRoot: string
}>): LocalAiSdkSandboxAccountHomes {
  const openagentsHome = resolve(
    input.accountHomes?.home ?? resolve(input.workspaceRoot, ".openagents-home"),
  )
  return {
    claudeConfigDir: resolve(
      input.accountHomes?.claudeConfigDir ??
        resolve(openagentsHome, "claude-code"),
    ),
    codexHome: resolve(
      input.accountHomes?.codexHome ?? resolve(openagentsHome, "codex"),
    ),
    home: openagentsHome,
  }
}

function cloneNetworkPolicy(
  policy: HarnessV1NetworkPolicy,
): HarnessV1NetworkPolicy {
  switch (policy.mode) {
    case "allow-all":
      return { mode: "allow-all" }
    case "deny-all":
      return { mode: "deny-all" }
    case "custom":
      return {
        mode: "custom",
        ...(policy.allowedHosts === undefined
          ? {}
          : { allowedHosts: [...policy.allowedHosts] }),
        ...(policy.allowedCIDRs === undefined
          ? {}
          : { allowedCIDRs: [...policy.allowedCIDRs] }),
        ...(policy.deniedCIDRs === undefined
          ? {}
          : { deniedCIDRs: [...policy.deniedCIDRs] }),
      } as HarnessV1NetworkPolicy
  }
}

function isPathInside(root: string, candidate: string): boolean {
  const resolvedRoot = resolve(root)
  const resolvedCandidate = resolve(candidate)
  return (
    resolvedCandidate === resolvedRoot ||
    resolvedCandidate.startsWith(`${resolvedRoot}${sep}`)
  )
}

function optionalAbortSignal(
  abortSignal: AbortSignal | undefined,
): { abortSignal?: AbortSignal } {
  return abortSignal === undefined ? {} : { abortSignal }
}

function stringProcessEnv(env: NodeJS.ProcessEnv): Record<string, string> {
  const result: Record<string, string> = {}
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) result[key] = value
  }
  return result
}

function isNodeFileNotFound(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: unknown }).code === "ENOENT"
  )
}

function bytesToStream(bytes: Uint8Array): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(bytes)
      controller.close()
    },
  })
}

async function collectReadableStream(
  stream: ReadableStream<Uint8Array>,
): Promise<Uint8Array> {
  const reader = stream.getReader()
  const chunks: Uint8Array[] = []
  let byteLength = 0
  while (true) {
    const next = await reader.read()
    if (next.done) break
    chunks.push(next.value)
    byteLength += next.value.byteLength
  }
  const result = new Uint8Array(byteLength)
  let offset = 0
  for (const chunk of chunks) {
    result.set(chunk, offset)
    offset += chunk.byteLength
  }
  return result
}

function nodeReadableToWeb(
  stream: NodeJS.ReadableStream,
): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      stream.on("data", (chunk: Buffer | Uint8Array | string) => {
        controller.enqueue(toBytes(chunk))
      })
      stream.once("end", () => controller.close())
      stream.once("error", (error) => controller.error(error))
    },
    cancel() {
      if ("destroy" in stream && typeof stream.destroy === "function") {
        stream.destroy()
      }
    },
  })
}

async function streamToText(stream: ReadableStream<Uint8Array>): Promise<string> {
  const bytes = await collectReadableStream(stream)
  return Buffer.from(bytes).toString("utf-8")
}

function toBytes(chunk: Buffer | Uint8Array | string): Uint8Array {
  if (typeof chunk === "string") return new TextEncoder().encode(chunk)
  return new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength)
}

export const localAiSdkSandboxProviderModuleUrl = fileURLToPath(import.meta.url)
