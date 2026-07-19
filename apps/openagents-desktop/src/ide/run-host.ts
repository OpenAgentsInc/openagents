import { spawn, type ChildProcess } from "node:child_process"
import { createHash, randomUUID } from "node:crypto"
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
  type Dirent,
} from "node:fs"
import path from "node:path"

import { Context, Effect, Exit, Layer, Result, Schema, Scope, Stream } from "effect"

import type { TerminalEvent } from "../terminal-contract.ts"
import {
  IdeEnvironmentGenerationSchema,
  IdeEnvironmentManifestRefSchema,
  IdeEnvironmentManifestSchema,
  IdeExecutableRefSchema,
  IdeExecutableAdmissionSchema,
  IdeArtifactRefSchema,
  IdeArtifactSchema,
  IdeOutputChannelRefSchema,
  IdeRunBindingSchema,
  IdeRunCommandResultSchema,
  IdeRunEventSchema,
  IdeRunSnapshotSchema,
  IdeTaskDefinitionRefSchema,
  IdeTaskDefinitionSchema,
  IdeTaskDiscoveryGenerationSchema,
  IdeTerminalProfileRefSchema,
  IdeTerminalProfileSchema,
  IdeTerminalReconnectGenerationSchema,
  IdeTerminalSessionRefSchema,
  IdeTerminalSessionSchema,
  IdeTerminalSplitRefSchema,
  IdeTestControllerRefSchema,
  IdeTestControllerSchema,
  IdeTestDiscoveryGenerationSchema,
  IdeTestItemRefSchema,
  IdeTestItemSchema,
  decodeIdeRunCommand,
  type IdeArtifact,
  type IdeEnvironmentManifest,
  type IdeExecutableAdmission,
  type IdeOutputLocation,
  type IdeOutputChannelRef,
  type IdeRunActor,
  type IdeRunBinding,
  type IdeRunCommand,
  type IdeRunCommandResult,
  type IdeRunEvent,
  type IdeRunSnapshot,
  type IdeTaskDefinition,
  type IdeTaskRun,
  type IdeTerminalProfile,
  type IdeTestController,
  type IdeTestItem,
  type IdeTestItemResult,
  type IdeTestRun,
} from "./run-contract.ts"
import {
  IdeRunInvalidInput,
  IdeRunNotFound,
  IdeRunService,
  IdeRunStale,
  IdeRunStopped,
  emptyIdeRunSnapshot,
  makeIdeRunServiceLayer,
  refusedIdeRunCommand,
  type IdeRunServiceError,
  type IdeRunServiceShape,
} from "./run-service.ts"
import {
  IdeAttachmentGenerationSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"

const MAX_TEST_FILES = 2_000
const MAX_DISCOVERY_DEPTH = 12
const MAX_OUTPUT_FRAME = 65_536
const SAFE_ENVIRONMENT_KEYS = [
  "HOME",
  "USER",
  "LOGNAME",
  "PATH",
  "SHELL",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "TERM",
  "COLORTERM",
  "TMPDIR",
] as const
const SECRET_ENVIRONMENT_NAME = /(?:^|_)(?:TOKEN|SECRET|PASSWORD|PASSWD|API_KEY|ACCESS_KEY|PRIVATE_KEY|CREDENTIAL|AUTH|SESSION|COOKIE|MNEMONIC|SEED)(?:$|_)/u
const TEST_FILE_PATTERN = /(?:^|\/)[^/]+\.(?:test|spec)\.(?:[cm]?[jt]sx?|py|rs)$/u
const TASK_MANIFEST_PATH = path.join(".openagents", "tasks.json")

const IdeDeclaredTaskManifestSchema = Schema.Struct({
  version: Schema.Literal(1),
  tasks: Schema.Array(Schema.Struct({
    id: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(96), Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/u)),
    label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    group: Schema.Literals(["build", "test", "lint", "run", "other"]),
    executable: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512)),
    argv: Schema.Array(Schema.String.check(Schema.isMaxLength(4_096))).check(Schema.isMaxLength(128)),
    dependsOn: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(96))).check(Schema.isMaxLength(32)),
    background: Schema.Boolean,
    readinessPattern: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240))),
    timeoutMs: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1_000, maximum: 86_400_000 })),
    maxRetries: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum: 5 })),
    artifactPaths: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512))).check(Schema.isMaxLength(32)),
  })).check(Schema.isMaxLength(256)),
}).annotate({ identifier: "IdeDeclaredTaskManifest" })

interface IdeDeclaredTaskManifest extends Schema.Schema.Type<typeof IdeDeclaredTaskManifestSchema> {}

export type IdeRunWorkspaceBinding = Readonly<{
  root: string
  grantRef: string
}>

export type IdeRunHostOptions = Readonly<{
  workspace: () => IdeRunWorkspaceBinding | null
  emit: (event: IdeRunEvent) => void
  environment?: () => Readonly<Record<string, string | undefined>>
  shell?: Readonly<{ command: string; args: ReadonlyArray<string> }>
  exportRoot?: string
  now?: () => string
}>

export type IdeRunHost = Readonly<{
  snapshot: () => Promise<IdeRunSnapshot | null>
  command: (value: unknown) => Promise<IdeRunCommandResult | null>
  observeTerminalEvent: (event: TerminalEvent) => Promise<void>
  observeTerminalResize: (sessionRef: string, cols: number, rows: number) => Promise<void>
  dispose: () => Promise<void>
}>

type Runtime = Readonly<{
  root: string
  grantRef: string
  scope: Scope.Closeable
  service: IdeRunServiceShape
  environment: Readonly<Record<string, string>>
  environmentManifest: IdeEnvironmentManifest
  profile: IdeTerminalProfile
}>

type RunningProcess = Readonly<{
  child: ChildProcess
  output: () => string
  cancel: () => void
  kind: "task" | "test"
}>

const digest = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex")
const shortDigest = (value: string): string => digest(value).slice(0, 24)

export const ideRunBindingFor = (workspace: IdeRunWorkspaceBinding): IdeRunBinding => {
  const identity = shortDigest(`${workspace.root}\0${workspace.grantRef}`)
  return IdeRunBindingSchema.make({
    projectRef: IdeProjectRefSchema.make(`ide.project.${identity}`),
    rootRef: IdeRootRefSchema.make(`ide.root.${identity}`),
    worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${identity}`),
    attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
    placementGeneration: IdePlacementGenerationSchema.make(1),
    placementRef: IdePlacementRefSchema.make("ide.placement.desktop-local"),
    cwdRef: `workspace:${identity}`,
    cwdLabel: path.basename(workspace.root) || "workspace",
  })
}

export const buildIdeRunEnvironment = (
  source: Readonly<Record<string, string | undefined>>,
  profileValues: Readonly<Record<string, string>> = {},
): Readonly<{
  values: Readonly<Record<string, string>>
  manifest: IdeEnvironmentManifest
}> => {
  const host: Record<string, string> = {}
  for (const key of SAFE_ENVIRONMENT_KEYS) {
    const value = source[key]
    if (typeof value === "string" && value.length > 0 && !SECRET_ENVIRONMENT_NAME.test(key)) host[key] = value
  }
  const profile: Record<string, string> = {}
  for (const [key, value] of Object.entries(profileValues)) {
    if (!/^[A-Z][A-Z0-9_]{0,95}$/u.test(key) || SECRET_ENVIRONMENT_NAME.test(key)) continue
    profile[key] = value
  }
  const values: Record<string, string> = {
    ...host,
    ...profile,
    TERM: profile.TERM ?? host.TERM ?? "xterm-256color",
    COLORTERM: profile.COLORTERM ?? host.COLORTERM ?? "truecolor",
    OPENAGENTS_DESKTOP_TERMINAL: "1",
  }
  const hostKeys = Object.keys(host).sort()
  const profileKeys = [...new Set([...Object.keys(profile), "TERM", "COLORTERM", "OPENAGENTS_DESKTOP_TERMINAL"])].sort()
  const admittedKeys = Object.keys(values).sort()
  const material = admittedKeys.map((key) => `${key}\0${values[key] ?? ""}`).join("\0")
  return {
    values,
    manifest: IdeEnvironmentManifestSchema.make({
      manifestRef: IdeEnvironmentManifestRefSchema.make(`ide.environment.${digest(material).slice(0, 32)}`),
      generation: IdeEnvironmentGenerationSchema.make(1),
      sources: [
        { _tag: "HostSafe", precedence: 10, keys: hostKeys },
        {
          _tag: "Profile",
          precedence: 20,
          profileRef: IdeTerminalProfileRefSchema.make("ide.terminal-profile.default"),
          keys: profileKeys,
        },
      ],
      admittedKeys,
      redactedKeys: [],
      inheritedAllHostVariables: false,
      valuesExposedToRenderer: false,
      digest: `sha256:${digest(material)}`,
    }),
  }
}

const executableAdmission = (
  executable: string,
  argv: ReadonlyArray<string>,
  source: IdeExecutableAdmission["source"],
  label: string,
): IdeExecutableAdmission => IdeExecutableAdmissionSchema.make({
  executableRef: IdeExecutableRefSchema.make(`ide.executable.${digest(`${executable}\0${argv.join("\0")}`).slice(0, 32)}`),
  executable,
  argv,
  displayLabel: label,
  source,
  shellInterpolation: false,
  admitted: path.isAbsolute(executable) || /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/u.test(executable),
  refusalReason: path.isAbsolute(executable) || /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/u.test(executable)
    ? null
    : "Executable did not match the admitted absolute-path or bounded-name grammar.",
})

const defaultProfile = (
  shell: Readonly<{ command: string; args: ReadonlyArray<string> }>,
): IdeTerminalProfile => IdeTerminalProfileSchema.make({
  profileRef: IdeTerminalProfileRefSchema.make("ide.terminal-profile.default"),
  label: "Default workspace shell",
  shellLabel: path.basename(shell.command),
  executable: executableAdmission(shell.command, shell.args, "profile", path.basename(shell.command)),
  environmentKeys: ["HOME", "USER", "LOGNAME", "PATH", "SHELL", "LANG", "LC_ALL", "LC_CTYPE", "TERM", "COLORTERM", "TMPDIR", "OPENAGENTS_DESKTOP_TERMINAL"],
  isDefault: true,
})

const ignoredDirectory = (name: string): boolean =>
  name === ".git" || name === "node_modules" || name === "dist" || name === "out" || name === "target" || name === ".next" || name === "coverage"

const discoverTestFiles = (root: string): ReadonlyArray<string> => {
  const found: string[] = []
  const visit = (directory: string, depth: number): void => {
    if (found.length >= MAX_TEST_FILES || depth > MAX_DISCOVERY_DEPTH) return
    let entries: ReadonlyArray<Dirent<string>>
    try {
      entries = readdirSync(directory, { withFileTypes: true, encoding: "utf8" })
    } catch {
      return
    }
    for (const entry of entries) {
      if (found.length >= MAX_TEST_FILES) break
      const absolute = path.join(directory, entry.name)
      if (entry.isDirectory()) {
        if (!ignoredDirectory(entry.name)) visit(absolute, depth + 1)
      } else if (entry.isFile()) {
        const relative = path.relative(root, absolute).split(path.sep).join("/")
        if (TEST_FILE_PATTERN.test(relative)) found.push(relative)
      }
    }
  }
  visit(root, 0)
  return found.sort()
}

const packageScripts = (root: string): Readonly<Record<string, string>> => {
  try {
    const value: unknown = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"))
    if (typeof value !== "object" || value === null) return {}
    const scriptValues: unknown = Reflect.get(value, "scripts")
    if (typeof scriptValues !== "object" || scriptValues === null || Array.isArray(scriptValues)) return {}
    const scripts: Record<string, string> = {}
    for (const [name, command] of Object.entries(scriptValues)) {
      if (/^[A-Za-z0-9][A-Za-z0-9:._-]{0,95}$/u.test(name) && typeof command === "string") scripts[name] = command
    }
    return scripts
  } catch {
    return {}
  }
}

const declaredTaskManifest = (root: string): IdeDeclaredTaskManifest | null => {
  try {
    const raw: unknown = JSON.parse(readFileSync(path.join(root, TASK_MANIFEST_PATH), "utf8"))
    const decoded = Schema.decodeUnknownExit(IdeDeclaredTaskManifestSchema, { onExcessProperty: "error" })(raw)
    return Exit.isSuccess(decoded) ? decoded.value : null
  } catch {
    return null
  }
}

const taskGroup = (name: string): IdeTaskDefinition["group"] => {
  if (/build/u.test(name)) return "build"
  if (/test/u.test(name)) return "test"
  if (/lint|check|typecheck/u.test(name)) return "lint"
  if (/dev|start|serve/u.test(name)) return "run"
  return "other"
}

const discover = (
  runtime: Runtime,
): Readonly<{ tasks: ReadonlyArray<IdeTaskDefinition>; controllers: ReadonlyArray<IdeTestController> }> => {
  const binding = ideRunBindingFor({ root: runtime.root, grantRef: runtime.grantRef })
  const scripts = packageScripts(runtime.root)
  const taskGeneration = IdeTaskDiscoveryGenerationSchema.make(1)
  const tasks = Object.keys(scripts).sort().slice(0, 256).map((name) => {
    const definitionRef = IdeTaskDefinitionRefSchema.make(`ide.task-definition.package.${digest(name).slice(0, 24)}`)
    return IdeTaskDefinitionSchema.make({
      definitionRef,
      discoveryGeneration: taskGeneration,
      version: 1,
      label: `package.json: ${name}`,
      group: taskGroup(name),
      dependencies: [],
      binding,
      executable: executableAdmission("pnpm", ["run", name], "task_definition", `pnpm run ${name}`),
      environment: runtime.environmentManifest,
      problemMatchers: [
        { matcherRef: "ide.problem-matcher.generic-location", kind: "generic_location", severity: "error" },
        { matcherRef: "ide.problem-matcher.typescript", kind: "typescript", severity: "error" },
      ],
      background: { enabled: /^(?:dev|start|serve)(?::|$)/u.test(name), readinessPattern: /^(?:dev|start|serve)(?::|$)/u.test(name) ? "ready|listening|local:" : null },
      timeoutMs: /^(?:dev|start|serve)(?::|$)/u.test(name) ? 3_600_000 : 900_000,
      maxRetries: 0,
      artifactPatterns: [],
      exactRerunLabel: `pnpm run ${name}`,
    })
  })
  const manifest = declaredTaskManifest(runtime.root)
  const declaredRefs = new Map((manifest?.tasks ?? []).map((task) => [
    task.id,
    IdeTaskDefinitionRefSchema.make(`ide.task-definition.declared.${digest(task.id).slice(0, 24)}`),
  ]))
  const declaredTasks = (manifest?.tasks ?? []).map((task) => IdeTaskDefinitionSchema.make({
    definitionRef: declaredRefs.get(task.id) ?? IdeTaskDefinitionRefSchema.make(`ide.task-definition.declared.${digest(task.id).slice(0, 24)}`),
    discoveryGeneration: taskGeneration,
    version: 1,
    label: task.label,
    group: task.group,
    dependencies: task.dependsOn.map((id) => declaredRefs.get(id)
      ?? IdeTaskDefinitionRefSchema.make(`ide.task-definition.declared.${digest(id).slice(0, 24)}`)),
    binding,
    executable: executableAdmission(task.executable, task.argv, "task_definition", task.label),
    environment: runtime.environmentManifest,
    problemMatchers: [{ matcherRef: "ide.problem-matcher.generic-location", kind: "generic_location", severity: "error" }],
    background: { enabled: task.background, readinessPattern: task.readinessPattern },
    timeoutMs: task.timeoutMs,
    maxRetries: task.maxRetries,
    artifactPatterns: task.artifactPaths,
    exactRerunLabel: `Declared task: ${task.id}`,
  }))
  const files = discoverTestFiles(runtime.root)
  const controllerRef = IdeTestControllerRefSchema.make("ide.test-controller.workspace")
  const discoveryGeneration = IdeTestDiscoveryGenerationSchema.make(1)
  const rootItem = IdeTestItemSchema.make({
    itemRef: IdeTestItemRefSchema.make("ide.test-item.workspace-root"),
    controllerRef,
    discoveryGeneration,
    parentRef: null,
    label: "Workspace tests",
    kind: "root",
    location: null,
    runnable: files.length > 0,
    debugSupported: false,
  })
  const items: IdeTestItem[] = [rootItem, ...files.map((relative) => IdeTestItemSchema.make({
    itemRef: IdeTestItemRefSchema.make(`ide.test-item.file.${digest(relative).slice(0, 32)}`),
    controllerRef,
    discoveryGeneration,
    parentRef: rootItem.itemRef,
    label: relative,
    kind: "file",
    location: { pathRef: relative, line: 1, column: 1, label: relative },
    runnable: true,
    debugSupported: false,
  }))]
  const controller = IdeTestControllerSchema.make({
    controllerRef,
    label: "Workspace test files",
    discoveryGeneration,
    binding,
    executable: executableAdmission("pnpm", ["exec", "vp", "test", "--run"], "test_controller", "Vitest workspace controller"),
    environment: runtime.environmentManifest,
    items,
    profiles: ["run", "coverage"],
    discoveryComplete: true,
    discoveryError: null,
  })
  return { tasks: [...declaredTasks, ...tasks], controllers: [controller] }
}

const parseLocations = (text: string): ReadonlyArray<IdeOutputLocation> => {
  const locations: IdeOutputLocation[] = []
  const pattern = /(?:^|\s)([A-Za-z0-9_./-]+\.[A-Za-z0-9]+):(\d+):(\d+)/gmu
  let match: RegExpExecArray | null
  while ((match = pattern.exec(text)) !== null && locations.length < 64) {
    locations.push({
      pathRef: match[1] ?? "unknown",
      line: Number.parseInt(match[2] ?? "1", 10),
      column: Number.parseInt(match[3] ?? "1", 10),
      label: `${match[1] ?? "unknown"}:${match[2] ?? "1"}:${match[3] ?? "1"}`,
    })
  }
  return locations
}

const redact = (text: string): Readonly<{ text: string; redacted: boolean }> => {
  const next = text
    .replace(/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}/gu, "«redacted»")
    .replace(/-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----[\s\S]*?-----END(?: [A-Z]+)? PRIVATE KEY-----/gu, "«redacted»")
  return { text: next, redacted: next !== text }
}

const signalProcessGroup = (child: ChildProcess, signal: NodeJS.Signals): void => {
  if (typeof child.pid !== "number") return
  try { process.kill(-child.pid, signal) } catch { /* already exited */ }
}

const settleReason = (error: IdeRunServiceError): Extract<IdeRunCommandResult, { readonly _tag: "Refused" }>["reason"] => {
  if (error instanceof IdeRunInvalidInput) return "not_admitted"
  if (error instanceof IdeRunStale) return "stale_generation"
  if (error instanceof IdeRunNotFound) return "not_found"
  if (error instanceof IdeRunStopped) return "stopped"
  return "unavailable"
}

const executeCommand = (
  service: IdeRunServiceShape,
  command: IdeRunCommand,
): Effect.Effect<IdeRunSnapshot, IdeRunServiceError> => {
  switch (command._tag) {
    case "Refresh": return service.snapshot
    case "RenameTerminal": return service.terminalRenamed(command.sessionRef, command.title)
    case "SplitTerminal": return service.terminalSplit(command.sessionRef)
    case "ClearOutput": return service.clearOutput(command.channelRef)
    case "Stop": return service.stop(command.reason)
    case "Discover":
    case "StartTask":
    case "CancelTask":
    case "RunTests":
    case "CancelTests":
    case "ExportOutput":
      return service.snapshot
  }
}

export const openIdeRunHost = async (options: IdeRunHostOptions): Promise<IdeRunHost> => {
  let runtime: Runtime | null = null
  let disposed = false
  const running = new Map<string, RunningProcess>()
  const shell = options.shell ?? { command: process.env.SHELL ?? "/bin/bash", args: [] }
  const environmentSource = options.environment ?? (() => process.env)

  const closeRuntime = async (reason: string): Promise<void> => {
    for (const process of running.values()) process.cancel()
    running.clear()
    const current = runtime
    runtime = null
    if (current === null) return
    await Effect.runPromise(current.service.stop(reason)).catch(() => undefined)
    await Effect.runPromise(Scope.close(current.scope, Exit.void)).catch(() => undefined)
  }

  const ensureRuntime = async (): Promise<Runtime | null> => {
    if (disposed) return null
    const workspace = options.workspace()
    if (workspace === null) {
      await closeRuntime("workspace unavailable")
      return null
    }
    if (runtime !== null && runtime.root === workspace.root && runtime.grantRef === workspace.grantRef) return runtime
    await closeRuntime("project generation changed")
    const admittedEnvironment = buildIdeRunEnvironment(environmentSource(), {
      TERM: "xterm-256color",
      COLORTERM: "truecolor",
      OPENAGENTS_DESKTOP_TERMINAL: "1",
    })
    const profile = defaultProfile(shell)
    const seed = emptyIdeRunSnapshot(ideRunBindingFor(workspace), [profile])
    const scope = await Effect.runPromise(Scope.make())
    const context = await Effect.runPromise(Layer.buildWithScope(makeIdeRunServiceLayer(seed, { now: options.now }), scope))
    const service = Context.get(context, IdeRunService)
    const opened: Runtime = {
      root: workspace.root,
      grantRef: workspace.grantRef,
      scope,
      service,
      environment: admittedEnvironment.values,
      environmentManifest: admittedEnvironment.manifest,
      profile,
    }
    runtime = opened
    await Effect.runPromise(service.events.pipe(
      Stream.runForEach((event) => Effect.sync(() => options.emit(event))),
      Effect.forkIn(scope),
    ))
    return opened
  }

  const appendProcessOutput = async (
    current: Runtime,
    channelRef: IdeOutputChannelRef,
    producer: Parameters<IdeRunServiceShape["appendOutput"]>[0]["producer"],
    stream: "stdout" | "stderr",
    raw: Buffer,
  ): Promise<void> => {
    const decoded = raw.toString("utf8")
    const invalidEncoding = decoded.includes("\uFFFD")
    for (let offset = 0; offset < decoded.length; offset += MAX_OUTPUT_FRAME) {
      const frame = decoded.slice(offset, offset + MAX_OUTPUT_FRAME)
      const safe = redact(frame)
      await Effect.runPromise(current.service.appendOutput({
        channelRef,
        producer,
        stream,
        text: safe.text,
        byteLength: Buffer.byteLength(frame),
        redacted: safe.redacted,
        truncated: decoded.length > MAX_OUTPUT_FRAME,
        gapBefore: false,
        invalidEncoding,
        locations: parseLocations(safe.text),
      })).catch(() => undefined)
    }
  }

  const spawnRun = (
    current: Runtime,
    input: Readonly<{
      key: string
      kind: "task" | "test"
      executable: string
      argv: ReadonlyArray<string>
      channelRef: IdeOutputChannelRef
      producer: Parameters<IdeRunServiceShape["appendOutput"]>[0]["producer"]
      timeoutMs: number
      readinessPattern?: string | null
      ready?: () => Promise<void>
      settle: (exitCode: number | null, output: string, cancelled: boolean, timedOut: boolean) => Promise<void>
    }>,
  ): Promise<void> => {
    let complete = (): void => undefined
    const completion = new Promise<void>((resolve) => { complete = resolve })
    const child = spawn(input.executable, [...input.argv], {
      cwd: current.root,
      env: { ...current.environment },
      detached: true,
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
    })
    let output = ""
    let cancelled = false
    let timedOut = false
    let ready = false
    let settled = false
    const onData = (stream: "stdout" | "stderr") => (buffer: Buffer) => {
      output = (output + buffer.toString("utf8")).slice(-1_000_000)
      void appendProcessOutput(current, input.channelRef, input.producer, stream, buffer)
      if (!ready && input.readinessPattern !== undefined && input.readinessPattern !== null) {
        try {
          if (new RegExp(input.readinessPattern, "iu").test(output)) {
            ready = true
            if (input.ready !== undefined) void input.ready()
          }
        } catch {
          // Discovery validates the bounded pattern. A later defect cannot
          // fabricate readiness; settlement records semantic failure.
        }
      }
    }
    child.stdout?.on("data", onData("stdout"))
    child.stderr?.on("data", onData("stderr"))
    const finish = (exitCode: number | null): void => {
      if (settled) return
      settled = true
      clearTimeout(timeout)
      running.delete(input.key)
      void input.settle(exitCode, output, cancelled, timedOut)
        .finally(complete)
    }
    child.on("exit", (code) => finish(code))
    child.on("error", () => finish(null))
    const cancel = (): void => {
      if (cancelled) return
      cancelled = true
      signalProcessGroup(child, "SIGTERM")
      const timer = setTimeout(() => signalProcessGroup(child, "SIGKILL"), 2_000)
      timer.unref?.()
    }
    const timeout = setTimeout(() => {
      if (settled) return
      timedOut = true
      signalProcessGroup(child, "SIGTERM")
      const killTimer = setTimeout(() => signalProcessGroup(child, "SIGKILL"), 2_000)
      killTimer.unref?.()
    }, input.timeoutMs)
    timeout.unref?.()
    running.set(input.key, { child, output: () => output, cancel, kind: input.kind })
    return completion
  }

  const discoverRuntime = async (current: Runtime): Promise<IdeRunSnapshot> => {
    try {
      const discovered = discover(current)
      return await Effect.runPromise(current.service.replaceDiscovery(discovered.tasks, discovered.controllers))
    } catch (cause) {
      if (cause instanceof IdeRunInvalidInput || cause instanceof IdeRunStale) throw cause
      throw new IdeRunInvalidInput({
        operation: "IdeRunHost.discover",
        detail: `Task or test discovery failed schema validation: ${String(cause).slice(0, 1_600)}`,
      })
    }
  }

  const collectArtifacts = (
    current: Runtime,
    definition: IdeTaskDefinition,
  ): ReadonlyArray<IdeArtifact> => definition.artifactPatterns.flatMap((relative) => {
    if (path.isAbsolute(relative) || relative.split(/[\\/]/u).some((part) => part === "" || part === "." || part === "..")) return []
    const absolute = path.resolve(current.root, relative)
    const rootWithSeparator = current.root.endsWith(path.sep) ? current.root : `${current.root}${path.sep}`
    if (!absolute.startsWith(rootWithSeparator) || !existsSync(absolute)) return []
    try {
      const stats = statSync(absolute)
      if (!stats.isFile() || stats.size > 64 * 1024 * 1024) return []
      const bytes = readFileSync(absolute)
      return [IdeArtifactSchema.make({
        artifactRef: IdeArtifactRefSchema.make(`ide.artifact.task.${digest(`${definition.definitionRef}\0${relative}\0${digest(bytes)}`).slice(0, 32)}`),
        label: relative,
        pathRef: relative,
        mediaType: "application/octet-stream",
        byteLength: stats.size,
        digest: `sha256:${digest(bytes)}`,
        available: true,
      })]
    } catch {
      return []
    }
  })

  const executeTaskRun = async (
    current: Runtime,
    run: IdeTaskRun,
    definition: IdeTaskDefinition,
  ): Promise<void> => {
    await spawnRun(current, {
      key: run.runRef,
      kind: "task",
      executable: definition.executable.executable,
      argv: definition.executable.argv,
      channelRef: run.outputChannelRef,
      producer: { _tag: "Task", runRef: run.runRef },
      timeoutMs: definition.timeoutMs,
      readinessPattern: definition.background.enabled ? definition.background.readinessPattern : null,
      ready: async () => {
        await Effect.runPromise(current.service.taskReady(run.runRef)).catch(() => undefined)
      },
      settle: async (exitCode, output, cancelled, timedOut) => {
        const readinessSatisfied = !definition.background.enabled || definition.background.readinessPattern === null || new RegExp(definition.background.readinessPattern, "iu").test(output)
        const problems = parseLocations(output)
        const artifacts = collectArtifacts(current, definition)
        const artifactsComplete = definition.artifactPatterns.length === 0 || artifacts.length === definition.artifactPatterns.length
        await Effect.runPromise(current.service.settleTask({
          runRef: run.runRef,
          exitCode,
          cancelled,
          timedOut,
          semanticChecksPassed: readinessSatisfied && artifactsComplete,
          problems,
          artifacts,
        })).catch(() => undefined)
      },
    })
  }

  const executeDependencies = async (
    current: Runtime,
    definition: IdeTaskDefinition,
    actor: IdeRunActor,
    completed: Set<string>,
  ): Promise<boolean> => {
    const snapshot = await Effect.runPromise(current.service.snapshot)
    for (const dependencyRef of definition.dependencies) {
      if (completed.has(dependencyRef)) continue
      const dependency = snapshot.taskDefinitions.find((candidate) => candidate.definitionRef === dependencyRef)
      if (dependency === undefined) return false
      if (!await executeDependencies(current, dependency, actor, completed)) return false
      const dependencyRun = await Effect.runPromise(current.service.startTask({ definitionRef: dependencyRef, actor }))
      await executeTaskRun(current, dependencyRun, dependency)
      const settled = (await Effect.runPromise(current.service.snapshot)).taskRuns.find((candidate) => candidate.runRef === dependencyRun.runRef)
      if (settled?.outcome._tag !== "Succeeded" && settled?.outcome._tag !== "Ready") return false
      completed.add(dependencyRef)
    }
    return true
  }

  const startTask = async (current: Runtime, command: Extract<IdeRunCommand, { readonly _tag: "StartTask" }>): Promise<IdeRunSnapshot> => {
    const run = await Effect.runPromise(current.service.startTask(command))
    const snapshot = await Effect.runPromise(current.service.snapshot)
    const definition = snapshot.taskDefinitions.find((candidate) => candidate.definitionRef === run.definitionRef)
    if (definition === undefined) throw new IdeRunNotFound({ operation: "IdeRunHost.startTask", detail: "Task definition disappeared after start." })
    void executeDependencies(current, definition, command.actor, new Set<string>()).then(async (dependenciesPassed) => {
      if (dependenciesPassed) return executeTaskRun(current, run, definition)
      await Effect.runPromise(current.service.settleTask({
        runRef: run.runRef,
        exitCode: null,
        cancelled: false,
        timedOut: false,
        semanticChecksPassed: false,
        problems: [],
        artifacts: [],
      })).catch(() => undefined)
    })
    return Effect.runPromise(current.service.snapshot)
  }

  const testPaths = (controller: IdeTestController, refs: ReadonlyArray<string>): ReadonlyArray<string> => {
    const selected = refs.length === 0
      ? controller.items.filter((item) => item.kind === "file" && item.runnable)
      : controller.items.filter((item) => refs.includes(item.itemRef))
    return selected.flatMap((item) => item.location === null ? [] : [item.location.pathRef])
  }

  const startTests = async (current: Runtime, command: Extract<IdeRunCommand, { readonly _tag: "RunTests" }>): Promise<IdeRunSnapshot> => {
    const run = await Effect.runPromise(current.service.startTests({ ...command, retryOf: command.retryOf }))
    const snapshot = await Effect.runPromise(current.service.snapshot)
    const controller = snapshot.testControllers.find((candidate) => candidate.controllerRef === command.controllerRef)
    if (controller === undefined) throw new IdeRunNotFound({ operation: "IdeRunHost.startTests", detail: "Test controller disappeared after start." })
    const paths = testPaths(controller, run.requestedItemRefs)
    const argv = [...controller.executable.argv, ...paths]
    if (command.profile === "coverage") argv.push("--coverage")
    void spawnRun(current, {
      key: run.runRef,
      kind: "test",
      executable: controller.executable.executable,
      argv,
      channelRef: run.outputChannelRef,
      producer: { _tag: "Test", runRef: run.runRef },
      timeoutMs: 900_000,
      settle: async (exitCode, output, cancelled) => {
        const assertionsObserved = /(?:Tests?|Assertions?)\s+[^\n]*(?:passed|failed)|\bPASS\b|\bFAIL\b/iu.test(output)
        const passed = exitCode === 0 && assertionsObserved && !cancelled
        const results: IdeTestItemResult[] = run.requestedItemRefs.map((itemRef) => ({
          itemRef,
          status: cancelled ? "cancelled" : passed ? "passed" : "failed",
          durationMs: null,
          message: passed ? null : assertionsObserved ? "Test process reported a failure." : "No assertion summary was observed.",
          location: controller.items.find((item) => item.itemRef === itemRef)?.location ?? null,
        }))
        await Effect.runPromise(current.service.settleTests({
          runRef: run.runRef,
          exitCode,
          cancelled,
          assertionsObserved,
          results,
          artifacts: [],
          coveragePercent: null,
        })).catch(() => undefined)
      },
    })
    return Effect.runPromise(current.service.snapshot)
  }

  const observeTerminalEvent = async (event: TerminalEvent): Promise<void> => {
    const current = await ensureRuntime()
    if (current === null) return
    const service = current.service
    const sessionRef = IdeTerminalSessionRefSchema.make(event.sessionRef.replace(/^terminal\./u, "ide.terminal."))
    const snapshot = await Effect.runPromise(service.snapshot)
    const existing = snapshot.terminals.find((terminal) => terminal.sessionRef === sessionRef)
    switch (event.kind) {
      case "ready": {
        if (existing === undefined) {
          const startedAt = options.now?.() ?? new Date().toISOString()
          await Effect.runPromise(service.terminalStarted({
            actor: { _tag: "Human", actorRef: "owner.desktop" },
            session: IdeTerminalSessionSchema.make({
              sessionRef,
              title: event.shellLabel || event.cwdLabel,
              profileRef: current.profile.profileRef,
              splitRef: IdeTerminalSplitRefSchema.make(`ide.terminal-split.${randomUUID()}`),
              binding: ideRunBindingFor({ root: current.root, grantRef: current.grantRef }),
              environment: current.environmentManifest,
              executable: current.profile.executable,
              outputChannelRef: IdeOutputChannelRefSchema.make(`ide.output-channel.terminal.${shortDigest(sessionRef)}`),
              cols: event.cols,
              rows: event.rows,
              reconnectGeneration: IdeTerminalReconnectGenerationSchema.make(1),
              shellIntegration: ["links"],
              lifecycle: { _tag: "Running", startedAt, pidPresent: true },
            }),
          })).catch(() => undefined)
        } else {
          await Effect.runPromise(service.terminalReconnected(sessionRef)).catch(() => undefined)
        }
        break
      }
      case "output": {
        const terminal = existing ?? (await Effect.runPromise(service.snapshot)).terminals.find((candidate) => candidate.sessionRef === sessionRef)
        if (terminal === undefined) break
        const safe = redact(event.chunk)
        await Effect.runPromise(service.appendOutput({
          channelRef: terminal.outputChannelRef,
          producer: { _tag: "Terminal", sessionRef },
          stream: "pty",
          text: safe.text,
          byteLength: Buffer.byteLength(event.chunk),
          redacted: safe.redacted || event.chunk.includes("«redacted"),
          truncated: false,
          gapBefore: false,
          invalidEncoding: event.chunk.includes("\uFFFD"),
          locations: parseLocations(safe.text),
        })).catch(() => undefined)
        break
      }
      case "exit":
        await Effect.runPromise(service.terminalExited(sessionRef, event.exitCode, event.signal)).catch(() => undefined)
        break
      case "closed":
        await Effect.runPromise(service.terminalClosed(
          sessionRef,
          event.reason === "workspace_revoked" ? "project_stale" : event.reason,
          { _tag: "Human", actorRef: "owner.desktop" },
        )).catch(() => undefined)
        break
      case "preview":
      case "error":
        break
    }
  }

  const command = async (value: unknown): Promise<IdeRunCommandResult | null> => {
    const current = await ensureRuntime()
    if (current === null) return null
    const decoded = decodeIdeRunCommand(value)
    if (decoded === null) return refusedIdeRunCommand("invalid_input", "The IDE run command did not match its schema.", await Effect.runPromise(current.service.snapshot))
    try {
      let snapshot: IdeRunSnapshot
      switch (decoded._tag) {
        case "Discover": snapshot = await discoverRuntime(current); break
        case "StartTask": snapshot = await startTask(current, decoded); break
        case "CancelTask": {
          running.get(decoded.runRef)?.cancel()
          await Effect.runPromise(current.service.cancelTask(decoded.runRef, decoded.actor))
          snapshot = await Effect.runPromise(current.service.snapshot)
          break
        }
        case "RunTests": snapshot = await startTests(current, decoded); break
        case "CancelTests": {
          running.get(decoded.runRef)?.cancel()
          await Effect.runPromise(current.service.cancelTests(decoded.runRef, decoded.actor))
          snapshot = await Effect.runPromise(current.service.snapshot)
          break
        }
        case "ExportOutput": {
          const artifact = await Effect.runPromise(current.service.exportOutput(decoded.channelRef, decoded.actor))
          const channel = (await Effect.runPromise(current.service.snapshot)).outputChannels.find((candidate) => candidate.channelRef === decoded.channelRef)
          if (channel !== undefined && options.exportRoot !== undefined) {
            mkdirSync(options.exportRoot, { recursive: true })
            const file = path.join(options.exportRoot, `${shortDigest(decoded.channelRef)}.txt`)
            writeFileSync(file, channel.chunks.map((chunk) => chunk.text).join(""), { encoding: "utf8", mode: 0o600 })
            void artifact
          }
          snapshot = await Effect.runPromise(current.service.snapshot)
          break
        }
        default: {
          const settled = await Effect.runPromise(Effect.result(executeCommand(current.service, decoded)))
          if (Result.isFailure(settled)) return refusedIdeRunCommand(settleReason(settled.failure), settled.failure.detail, await Effect.runPromise(current.service.snapshot))
          snapshot = settled.success
        }
      }
      return IdeRunCommandResultSchema.cases.Succeeded.make({ snapshot })
    } catch (cause) {
      if (cause instanceof IdeRunInvalidInput || cause instanceof IdeRunStale || cause instanceof IdeRunNotFound || cause instanceof IdeRunStopped) {
        return refusedIdeRunCommand(settleReason(cause), cause.detail, await Effect.runPromise(current.service.snapshot))
      }
      return refusedIdeRunCommand("unavailable", "The IDE run host failed outside its typed service boundary.", await Effect.runPromise(current.service.snapshot))
    }
  }

  return {
    snapshot: async () => {
      const current = await ensureRuntime()
      if (current === null) return null
      return Effect.runPromise(current.service.snapshot)
    },
    command,
    observeTerminalEvent,
    observeTerminalResize: async (sessionRef, cols, rows) => {
      const current = await ensureRuntime()
      if (current === null) return
      const ref = IdeTerminalSessionRefSchema.make(sessionRef.replace(/^terminal\./u, "ide.terminal."))
      await Effect.runPromise(current.service.terminalResized(ref, cols, rows)).catch(() => undefined)
    },
    dispose: async () => {
      if (disposed) return
      disposed = true
      await closeRuntime("host dispose")
    },
  }
}
