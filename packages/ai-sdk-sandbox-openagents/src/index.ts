import { createHash } from "node:crypto"
import { posix } from "node:path"
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

const DEFAULT_PROVIDER_ID = "openagents-sandbox"
const SNAPSHOT_SCHEMA_VERSION = "openagents.sandbox.snapshot_identity.v1"

export type OpenAgentsSandboxLane = "owner_local" | "public_untrusted"

export type OpenAgentsSandboxAccountHomes = Readonly<{
  home: string
  codexHome: string
  claudeConfigDir: string
}>

export type OpenAgentsSandboxSnapshotInputs = Readonly<{
  agentsSetupRef: string
  baseImageRef: string
  repoRef: string
  sandboxProfileRef: string
  toolchainRef: string
  lockfileRefs?: ReadonlyArray<string>
  additionalRefs?: Readonly<Record<string, string>>
}>

export type OpenAgentsSandboxSnapshotIdentity = Readonly<{
  identity: string
  inputs: OpenAgentsSandboxSnapshotInputs & Readonly<{
    bridgeBootstrapRecipeRef: string
    schemaVersion: typeof SNAPSHOT_SCHEMA_VERSION
  }>
}>

export type OpenAgentsSandboxSessionDescriptor = Readonly<{
  id: string
  defaultWorkingDirectory: string
  ports: ReadonlyArray<number>
  fresh?: boolean
}>

export type OpenAgentsSandboxCreateSessionInput = Readonly<{
  accountHomes: OpenAgentsSandboxAccountHomes
  defaultWorkingDirectory: string
  lane: OpenAgentsSandboxLane
  networkPolicy?: HarnessV1NetworkPolicy
  ports: ReadonlyArray<number>
  sessionId: string
  snapshotIdentity: OpenAgentsSandboxSnapshotIdentity
  abortSignal?: AbortSignal
}>

export type OpenAgentsSandboxResumeSessionInput = Readonly<{
  sessionId: string
  abortSignal?: AbortSignal
}>

export type OpenAgentsSandboxFileInput = Readonly<{
  path: string
  abortSignal?: AbortSignal
}>

export type OpenAgentsSandboxWriteFileInput = Readonly<{
  path: string
  content: Uint8Array
  abortSignal?: AbortSignal
}>

export type OpenAgentsSandboxProcessInput = Readonly<{
  command: string
  workingDirectory: string
  env: Readonly<Record<string, string>>
  abortSignal?: AbortSignal
}>

export type OpenAgentsSandboxV1Client = Readonly<{
  createSession: (
    input: OpenAgentsSandboxCreateSessionInput,
  ) => PromiseLike<OpenAgentsSandboxSessionDescriptor>
  resumeSession: (
    input: OpenAgentsSandboxResumeSessionInput,
  ) => PromiseLike<OpenAgentsSandboxSessionDescriptor>
  stopSession: (
    input: Readonly<{ sessionId: string; abortSignal?: AbortSignal }>,
  ) => PromiseLike<void>
  destroySession: (
    input: Readonly<{ sessionId: string; abortSignal?: AbortSignal }>,
  ) => PromiseLike<void>
  readBinaryFile: (
    input: OpenAgentsSandboxFileInput & Readonly<{ sessionId: string }>,
  ) => PromiseLike<Uint8Array | null>
  writeBinaryFile: (
    input: OpenAgentsSandboxWriteFileInput & Readonly<{ sessionId: string }>,
  ) => PromiseLike<void>
  run: (
    input: OpenAgentsSandboxProcessInput & Readonly<{ sessionId: string }>,
  ) => PromiseLike<{ exitCode: number; stdout: string; stderr: string }>
  spawn: (
    input: OpenAgentsSandboxProcessInput & Readonly<{ sessionId: string }>,
  ) => PromiseLike<Experimental_SandboxProcess>
  getPortUrl: (
    input: Readonly<{
      sessionId: string
      port: number
      protocol?: "http" | "https" | "ws"
      abortSignal?: AbortSignal
    }>,
  ) => PromiseLike<string>
  setPorts: (
    input: Readonly<{
      sessionId: string
      ports: ReadonlyArray<number>
      abortSignal?: AbortSignal
    }>,
  ) => PromiseLike<ReadonlyArray<number>>
  setNetworkPolicy: (
    input: Readonly<{
      sessionId: string
      policy: HarnessV1NetworkPolicy
      abortSignal?: AbortSignal
    }>,
  ) => PromiseLike<void>
}>

export type OpenAgentsAiSdkSandboxProviderOptions = Readonly<{
  accountHomes: OpenAgentsSandboxAccountHomes
  client: OpenAgentsSandboxV1Client
  providerId?: string
  defaultWorkingDirectory?: string
  initialNetworkPolicy?: HarnessV1NetworkPolicy
  lane?: OpenAgentsSandboxLane
  ports?: ReadonlyArray<number>
  snapshotInputs: OpenAgentsSandboxSnapshotInputs
}>

export function createOpenAgentsAiSdkSandboxProvider(
  options: OpenAgentsAiSdkSandboxProviderOptions,
): HarnessV1SandboxProvider {
  return new OpenAgentsAiSdkSandboxProvider(options)
}

export class OpenAgentsAiSdkSandboxProvider implements HarnessV1SandboxProvider {
  readonly specificationVersion = "harness-sandbox-v1" as const
  readonly providerId: string
  private readonly accountHomes: OpenAgentsSandboxAccountHomes
  private readonly client: OpenAgentsSandboxV1Client
  private readonly defaultWorkingDirectory: string
  private readonly initialNetworkPolicy: HarnessV1NetworkPolicy | undefined
  private readonly lane: OpenAgentsSandboxLane
  private readonly ports: ReadonlyArray<number>
  private readonly snapshotInputs: OpenAgentsSandboxSnapshotInputs

  constructor(options: OpenAgentsAiSdkSandboxProviderOptions) {
    assertExplicitAccountHomes(options.accountHomes)
    this.accountHomes = options.accountHomes
    this.client = options.client
    this.defaultWorkingDirectory = options.defaultWorkingDirectory ?? "/workspace"
    this.initialNetworkPolicy = options.initialNetworkPolicy
    this.lane = options.lane ?? "owner_local"
    this.ports = options.ports ?? []
    this.providerId = options.providerId ?? DEFAULT_PROVIDER_ID
    this.snapshotInputs = options.snapshotInputs
    assertLaneNetworkPolicy({
      lane: this.lane,
      policy: this.initialNetworkPolicy,
    })
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
    const sessionId = options.sessionId ?? cryptoRandomSessionId()
    const snapshotIdentity = buildOpenAgentsSandboxSnapshotIdentity({
      bridgeBootstrapRecipeRef:
        options.identity ?? "ai-sdk-harness-bootstrap.unspecified",
      inputs: this.snapshotInputs,
    })
    const descriptor = await this.client.createSession({
      accountHomes: this.accountHomes,
      defaultWorkingDirectory: this.defaultWorkingDirectory,
      lane: this.lane,
      ports: this.ports,
      sessionId,
      snapshotIdentity,
      ...(this.initialNetworkPolicy === undefined
        ? {}
        : { networkPolicy: this.initialNetworkPolicy }),
      ...optionalAbortSignal(options.abortSignal),
    })
    const session = this.wrapDescriptor(descriptor)
    if (descriptor.fresh === true && options.onFirstCreate !== undefined) {
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
    const descriptor = await this.client.resumeSession({
      sessionId: options.sessionId,
      ...optionalAbortSignal(options.abortSignal),
    })
    return this.wrapDescriptor(descriptor)
  }

  private wrapDescriptor(
    descriptor: OpenAgentsSandboxSessionDescriptor,
  ): OpenAgentsAiSdkNetworkSandboxSession {
    return new OpenAgentsAiSdkNetworkSandboxSession({
      accountHomes: this.accountHomes,
      client: this.client,
      descriptor,
      lane: this.lane,
      providerId: this.providerId,
    })
  }
}

export class OpenAgentsAiSdkNetworkSandboxSession
  implements HarnessV1NetworkSandboxSession
{
  readonly defaultWorkingDirectory: string
  readonly id: string
  private readonly accountHomes: OpenAgentsSandboxAccountHomes
  private readonly client: OpenAgentsSandboxV1Client
  private readonly lane: OpenAgentsSandboxLane
  private readonly providerId: string
  private exposedPorts: ReadonlyArray<number>
  private stopped = false

  constructor(input: Readonly<{
    accountHomes: OpenAgentsSandboxAccountHomes
    client: OpenAgentsSandboxV1Client
    descriptor: OpenAgentsSandboxSessionDescriptor
    lane: OpenAgentsSandboxLane
    providerId: string
  }>) {
    this.accountHomes = input.accountHomes
    this.client = input.client
    this.defaultWorkingDirectory = posix.normalize(
      input.descriptor.defaultWorkingDirectory,
    )
    this.exposedPorts = [...input.descriptor.ports]
    this.id = input.descriptor.id
    this.lane = input.lane
    this.providerId = input.providerId
  }

  get description(): string {
    return [
      `OpenAgents AI SDK sandbox (${this.providerId}:${this.id}).`,
      `Workspace root: ${this.defaultWorkingDirectory}`,
      `Lane: ${this.lane}`,
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
        `Port ${options.port} is not exposed on OpenAgents sandbox ${this.id}.`,
      )
    }
    return this.client.getPortUrl({
      port: options.port,
      sessionId: this.id,
      ...(options.protocol === undefined ? {} : { protocol: options.protocol }),
    })
  }

  setNetworkPolicy = async (policy: HarnessV1NetworkPolicy): Promise<void> => {
    this.assertRunning()
    assertLaneNetworkPolicy({ lane: this.lane, policy })
    await this.client.setNetworkPolicy({
      policy,
      sessionId: this.id,
    })
  }

  setPorts = async (
    ports: ReadonlyArray<number>,
    options?: { abortSignal?: AbortSignal },
  ): Promise<void> => {
    options?.abortSignal?.throwIfAborted()
    this.assertRunning()
    this.exposedPorts = await this.client.setPorts({
      ports,
      sessionId: this.id,
      ...optionalAbortSignal(options?.abortSignal),
    })
  }

  stop = async (): Promise<void> => {
    if (this.stopped) return
    this.stopped = true
    await this.client.stopSession({ sessionId: this.id })
  }

  destroy = async (): Promise<void> => {
    this.stopped = true
    await this.client.destroySession({ sessionId: this.id })
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
    return this.client.readBinaryFile({
      path: this.resolvePath(options.path),
      sessionId: this.id,
      ...optionalAbortSignal(options.abortSignal),
    })
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
    await this.client.writeBinaryFile({
      content: options.content,
      path: this.resolvePath(options.path),
      sessionId: this.id,
      ...optionalAbortSignal(options.abortSignal),
    })
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
    return this.client.spawn({
      command: options.command,
      env: this.commandEnv(options.env),
      sessionId: this.id,
      workingDirectory:
        options.workingDirectory === undefined
          ? this.defaultWorkingDirectory
          : this.resolvePath(options.workingDirectory),
      ...optionalAbortSignal(options.abortSignal),
    })
  }

  async run(options: {
    command: string
    workingDirectory?: string
    env?: Record<string, string>
    abortSignal?: AbortSignal
  }): Promise<{ exitCode: number; stdout: string; stderr: string }> {
    options.abortSignal?.throwIfAborted()
    this.assertRunning()
    return this.client.run({
      command: options.command,
      env: this.commandEnv(options.env),
      sessionId: this.id,
      workingDirectory:
        options.workingDirectory === undefined
          ? this.defaultWorkingDirectory
          : this.resolvePath(options.workingDirectory),
      ...optionalAbortSignal(options.abortSignal),
    })
  }

  private commandEnv(env?: Record<string, string>): Readonly<Record<string, string>> {
    return {
      ...(env ?? {}),
      CLAUDE_CONFIG_DIR: this.accountHomes.claudeConfigDir,
      CODEX_HOME: this.accountHomes.codexHome,
      HOME: this.accountHomes.home,
    }
  }

  private resolvePath(path: string): string {
    if (path.includes("\0")) {
      throw new Error("Sandbox paths cannot contain NUL bytes.")
    }
    const resolved = path.startsWith("/")
      ? posix.normalize(path)
      : posix.normalize(posix.join(this.defaultWorkingDirectory, path))
    if (!isPosixPathInside(this.defaultWorkingDirectory, resolved)) {
      throw new Error(
        `Path ${path} escapes OpenAgents sandbox workspace ${this.defaultWorkingDirectory}.`,
      )
    }
    return resolved
  }

  private assertRunning(): void {
    if (this.stopped) {
      throw new Error(`OpenAgents sandbox ${this.id} is stopped.`)
    }
  }
}

export function buildOpenAgentsSandboxSnapshotIdentity(
  input: Readonly<{
    bridgeBootstrapRecipeRef: string
    inputs: OpenAgentsSandboxSnapshotInputs
  }>,
): OpenAgentsSandboxSnapshotIdentity {
  const normalizedInputs: OpenAgentsSandboxSnapshotIdentity["inputs"] = {
    ...input.inputs,
    additionalRefs: normalizeRecord(input.inputs.additionalRefs ?? {}),
    bridgeBootstrapRecipeRef: input.bridgeBootstrapRecipeRef,
    lockfileRefs: [...(input.inputs.lockfileRefs ?? [])].sort(),
    schemaVersion: SNAPSHOT_SCHEMA_VERSION,
  }
  const identity = createHash("sha256")
    .update(JSON.stringify(normalizedInputs))
    .digest("hex")
  return {
    identity: `sha256:${identity}`,
    inputs: normalizedInputs,
  }
}

function assertExplicitAccountHomes(homes: OpenAgentsSandboxAccountHomes): void {
  for (const [name, value] of Object.entries(homes)) {
    if (value.trim().length === 0 || value === "~" || value.startsWith("~/")) {
      throw new Error(
        `OpenAgents AI SDK sandbox requires an explicit ${name}; ambient defaults are not allowed.`,
      )
    }
  }
}

function assertLaneNetworkPolicy(input: Readonly<{
  lane: OpenAgentsSandboxLane
  policy: HarnessV1NetworkPolicy | undefined
}>): void {
  if (input.lane !== "public_untrusted") return
  if (input.policy === undefined) {
    throw new Error(
      "Public/untrusted OpenAgents AI SDK sandboxes require an explicit network policy.",
    )
  }
  if (input.policy.mode === "allow-all") {
    throw new Error(
      "Public/untrusted OpenAgents AI SDK sandboxes cannot use allow-all network policy.",
    )
  }
}

function optionalAbortSignal(
  abortSignal: AbortSignal | undefined,
): { abortSignal?: AbortSignal } {
  return abortSignal === undefined ? {} : { abortSignal }
}

function normalizeRecord(
  record: Readonly<Record<string, string>>,
): Readonly<Record<string, string>> {
  const result: Record<string, string> = {}
  for (const key of Object.keys(record).sort()) {
    result[key] = record[key] ?? ""
  }
  return result
}

function isPosixPathInside(root: string, candidate: string): boolean {
  const normalizedRoot = posix.normalize(root)
  const normalizedCandidate = posix.normalize(candidate)
  return (
    normalizedCandidate === normalizedRoot ||
    normalizedCandidate.startsWith(`${normalizedRoot}/`)
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

function cryptoRandomSessionId(): string {
  return `openagents-${createHash("sha256")
    .update(`${Date.now()}:${Math.random()}`)
    .digest("hex")
    .slice(0, 24)}`
}
