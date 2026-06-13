import { createHash, randomBytes } from "node:crypto"
import { mkdir, stat, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import {
  publicPylonAccountSelection,
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
  type PublicPylonAccountSelection,
  type PylonAccountProvider,
  type ResolvedPylonAccountSelection,
} from "../account-registry"
import { loadClaudeAgentConfig } from "../claude-agent"
import { runClaudeComposerStream } from "../claude-composer"
import { loadCodexAgentConfig, type PylonComposerAdapter } from "../codex-agent"
import { runCodexComposerStream, sandboxModeForCodexComposerExecutionMode } from "../codex-composer"
import type { BootstrapSummary } from "../bootstrap"
import {
  recordPylonDevCodexRun,
  runPylonDevCheck,
  type PylonDevCheckProjection,
} from "../dev-loop"
import {
  PROOF_REDACTION_PATTERN_REFS,
  scanProofSerialization,
} from "../proof-redaction"
import { classifySessionError } from "../session-error-class"
import { assertPublicProjectionSafe } from "../state"
import {
  materializeGitCheckoutWorkspaceWithLease,
  type GitCheckoutWorkspace,
} from "../workspace-materializer"

export const CONTROL_SESSION_EVENT_SCHEMA = "openagents.pylon.control_session_event.v0.1"
export const CONTROL_SESSION_ARTIFACT_SCHEMA = "openagents.pylon.control_session_artifact.v0.1"
export const CONTROL_SESSION_FAILURE_SCHEMA = "openagents.pylon.control_session_failure.v0.1"

type ControlSessionRepositoryRef = GitCheckoutWorkspace["repository"]

export type ControlSessionSpawnCommand = {
  type: "session.spawn"
  adapter: PylonComposerAdapter
  accountRef?: string
  accountHome?: string
  codexHome?: string
  claudeConfigDir?: string
  repoRef?: ControlSessionRepositoryRef
  worktree?: string | { path?: string }
  worktreePath?: string
  objective: string
  verify: string[]
  timeoutSeconds?: number
}

export type ControlSessionListCommand = { type: "session.list" }
export type ControlSessionEventsCommand = { type: "session.events"; sessionRef: string }
export type ControlSessionCancelCommand = { type: "session.cancel"; sessionRef: string }

export type ControlSessionCommand =
  | ControlSessionSpawnCommand
  | ControlSessionListCommand
  | ControlSessionEventsCommand
  | ControlSessionCancelCommand

export type ControlSessionState = "queued" | "running" | "completed" | "failed" | "cancelled"
export type ControlSessionEventPhase =
  | "queued"
  | "started"
  | "composer_event"
  | "dev_check_started"
  | "completed"
  | "failed"
  | "cancelled"
  | "redaction_blocked"

export type ControlSessionEvent = {
  schema: typeof CONTROL_SESSION_EVENT_SCHEMA
  sessionRef: string
  observedAt: string
  eventIndex: number
  phase: ControlSessionEventPhase
  state: ControlSessionState
  adapter: PylonComposerAdapter
  account: PublicPylonAccountSelection | null
  workspaceRef: string
  messageRef?: string
  composerEventIndex?: number
  artifactRef?: string
  resultRef?: string
  errorClass?: string
  errorDigestRef?: string
  violationRefs?: string[]
}

export type ControlSessionProjection = {
  sessionRef: string
  adapter: PylonComposerAdapter
  account: PublicPylonAccountSelection | null
  workspaceRef: string
  objectiveDigestRef: string
  verifyRef: string
  state: ControlSessionState
  artifactRef: string | null
  resultRef: string | null
  errorClass: string | null
  errorDigestRef: string | null
  createdAt: string
  startedAt: string | null
  completedAt: string | null
  eventCount: number
}

export type ControlSessionActions = {
  spawn: (command: ControlSessionSpawnCommand) => Promise<{ sessionRef: string; state: ControlSessionState }>
  list: () => Promise<ControlSessionProjection[]>
  cancel: (sessionRef: string) => Promise<ControlSessionProjection>
  events: (sessionRef: string) => Promise<{ sessionRef: string; eventsPath: string; state: ControlSessionState }>
  eventStream: (sessionRef: string) => ReadableStream<Uint8Array>
}

export type ControlSessionExecutorInput = {
  adapter: PylonComposerAdapter
  account: ResolvedPylonAccountSelection | null
  abortSignal: AbortSignal
  cwd: string
  env: Record<string, string | undefined>
  emit: (event: { phase: "composer_event" | "dev_check_started"; message?: string; composerEventIndex?: number }) => void
  objective: string
  sessionRef: string
  summary: BootstrapSummary
  timeoutMs: number
  verify: string[]
  workspaceRef: string
}

export type ControlSessionExecutorResult = {
  commandCount: number
  devCheck: PylonDevCheckProjection
  editedFileCount: number
  eventCount: number
  externalSessionRef: string | null
  responseDigestRef: string | null
  totalTokens: number
}

export type ControlSessionExecutor = (
  input: ControlSessionExecutorInput,
) => Promise<ControlSessionExecutorResult>

type WorkspaceSelection = {
  workspaceRef: string
  workingDirectory: string
}

type SessionRecord = {
  sessionRef: string
  adapter: PylonComposerAdapter
  account: ResolvedPylonAccountSelection | null
  workspace: WorkspaceSelection
  objective: string
  objectiveDigestRef: string
  verify: string[]
  verifyRef: string
  timeoutMs: number
  state: ControlSessionState
  abort: AbortController
  createdAt: string
  startedAt: string | null
  completedAt: string | null
  artifactRef: string | null
  resultRef: string | null
  errorClass: string | null
  errorDigestRef: string | null
  events: ControlSessionEvent[]
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function nowIso() {
  return new Date().toISOString()
}

function providerForAdapter(adapter: PylonComposerAdapter): PylonAccountProvider {
  return adapter === "codex" ? "codex" : "claude_agent"
}

function stringArray(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length === 0) return null
  const entries = value.filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
  return entries.length === value.length ? entries : null
}

function repositoryRefFrom(value: unknown): ControlSessionRepositoryRef | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (record.provider !== "github" || record.visibility !== "public") return null
  if (typeof record.fullName !== "string" || typeof record.branch !== "string") return null
  if (typeof record.commitSha !== "string" || !/^[a-f0-9]{40}$/i.test(record.commitSha)) return null
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(record.fullName)) return null
  return {
    provider: "github",
    visibility: "public",
    fullName: record.fullName,
    branch: record.branch,
    commitSha: record.commitSha,
  }
}

function rejectDangerFields(record: Record<string, unknown>) {
  if (
    record.executionMode === "local_supervised_danger" ||
    record.sandboxMode === "danger-full-access" ||
    record.permissionMode === "bypassPermissions" ||
    record.codexDanger === true ||
    record.claudeDanger === true
  ) {
    throw new Error("control sessions reject local danger modes")
  }
}

function parseSpawnCommand(raw: ControlSessionSpawnCommand): ControlSessionSpawnCommand {
  const record = raw as Record<string, unknown>
  rejectDangerFields(record)
  if (raw.adapter !== "codex" && raw.adapter !== "claude_agent") {
    throw new Error("session.spawn requires adapter codex or claude_agent")
  }
  if (typeof raw.objective !== "string" || raw.objective.trim().length === 0) {
    throw new Error("session.spawn requires a non-empty objective")
  }
  const verify = stringArray(raw.verify)
  if (verify === null) throw new Error("session.spawn requires non-empty verify argv")
  const repoRef = raw.repoRef === undefined ? undefined : repositoryRefFrom(raw.repoRef)
  if (raw.repoRef !== undefined && repoRef === null) throw new Error("session.spawn repoRef is invalid")
  const worktreePath =
    typeof raw.worktreePath === "string"
      ? raw.worktreePath
      : typeof raw.worktree === "string"
        ? raw.worktree
        : raw.worktree && typeof raw.worktree === "object" && typeof raw.worktree.path === "string"
          ? raw.worktree.path
          : undefined
  if (repoRef === undefined && worktreePath === undefined) {
    throw new Error("session.spawn requires repoRef or worktreePath")
  }
  if (repoRef !== undefined && worktreePath !== undefined) {
    throw new Error("session.spawn must use only one workspace selector")
  }
  return {
    type: "session.spawn",
    adapter: raw.adapter,
    ...(typeof raw.accountRef === "string" ? { accountRef: raw.accountRef } : {}),
    ...(typeof raw.accountHome === "string" ? { accountHome: raw.accountHome } : {}),
    ...(typeof raw.codexHome === "string" ? { codexHome: raw.codexHome } : {}),
    ...(typeof raw.claudeConfigDir === "string" ? { claudeConfigDir: raw.claudeConfigDir } : {}),
    ...(repoRef === undefined ? {} : { repoRef }),
    ...(worktreePath === undefined ? {} : { worktreePath }),
    objective: raw.objective,
    verify,
    ...(typeof raw.timeoutSeconds === "number" && Number.isFinite(raw.timeoutSeconds)
      ? { timeoutSeconds: Math.max(1, Math.min(1200, Math.floor(raw.timeoutSeconds))) }
      : {}),
  }
}

function projectionFor(record: SessionRecord): ControlSessionProjection {
  return {
    sessionRef: record.sessionRef,
    adapter: record.adapter,
    account: publicPylonAccountSelection(record.account),
    workspaceRef: record.workspace.workspaceRef,
    objectiveDigestRef: record.objectiveDigestRef,
    verifyRef: record.verifyRef,
    state: record.state,
    artifactRef: record.artifactRef,
    resultRef: record.resultRef,
    errorClass: record.errorClass,
    errorDigestRef: record.errorDigestRef,
    createdAt: record.createdAt,
    startedAt: record.startedAt,
    completedAt: record.completedAt,
    eventCount: record.events.length,
  }
}

async function workspaceForCommand(input: {
  command: ControlSessionSpawnCommand
  index: number
  runRef: string
  summary: BootstrapSummary
}): Promise<WorkspaceSelection> {
  if (input.command.worktreePath !== undefined) {
    const workingDirectory = resolve(input.command.worktreePath)
    try {
      const info = await stat(workingDirectory)
      if (!info.isDirectory()) throw new Error("not a directory")
    } catch {
      throw new Error("worktree_path_missing")
    }
    return {
      workingDirectory,
      workspaceRef: stableRef("workspace.pylon.control_session.injected", workingDirectory),
    }
  }

  const repoRef = input.command.repoRef
  if (repoRef === undefined) throw new Error("workspace_selector_missing")
  const checkout: GitCheckoutWorkspace = {
    kind: "git_checkout",
    repository: repoRef,
    verificationCommand: {
      args: input.command.verify,
      commandRef: stableRef("command.pylon.control_session.verify", input.command.verify.join("\0")),
    },
  }
  const materialized = await materializeGitCheckoutWorkspaceWithLease({
    cacheRoot: join(input.summary.paths.cache, "control-session-worktrees"),
    checkout,
    leaseRef: stableRef("lease.pylon.control_session.workspace", `${input.runRef}:${input.index}`),
    refPrefix: "workspace.pylon.control_session",
    repositoryCacheRoot: join(input.summary.paths.cache, "workspace-git-cache"),
    workspaceStateRoot: join(input.summary.paths.cache, "workspace-leases"),
  })
  return {
    workingDirectory: materialized.workingDirectory,
    workspaceRef: materialized.workspaceRef,
  }
}

async function defaultControlSessionExecutor(
  input: ControlSessionExecutorInput,
): Promise<ControlSessionExecutorResult> {
  const emitComposerEvent = (message: string, eventCount: number) =>
    input.emit({ phase: "composer_event", message, composerEventIndex: eventCount })

  let eventCount = 0
  let commandCount = 0
  let editedFileCount = 0
  let totalTokens = 0
  let externalSessionRef: string | null = null
  let responseDigestRef: string | null = null

  if (input.adapter === "codex") {
    const config = await loadCodexAgentConfig(input.summary)
    const sandboxMode = sandboxModeForCodexComposerExecutionMode("local_bounded", config.sandboxMode)
    if (sandboxMode === "danger-full-access") throw new Error("control sessions never use danger-full-access")
    const result = await runCodexComposerStream(
      input.objective,
      {
        abortSignal: input.abortSignal,
        account: input.account,
        approvalPolicy: "never",
        config,
        cwd: input.cwd,
        env: input.env,
        executionMode: "local_bounded",
        ...(config.model === undefined ? {} : { model: config.model }),
        networkAccessEnabled: false,
        sandboxMode,
        timeoutMs: input.timeoutMs,
        usageStateSummary: input.summary,
      },
      {
        onEvent: emitComposerEvent,
      },
    )
    await recordPylonDevCodexRun(
      {
        commandCount: result.commandCount,
        cwd: input.cwd,
        editedFileCount: result.editedFileCount,
        eventCount: result.eventCount,
        executionMode: "local_bounded",
        sandboxMode,
        totalTokens: result.totalTokens,
      },
      { cwd: input.cwd, env: input.env, summary: input.summary },
    )
    eventCount = result.eventCount
    commandCount = result.commandCount
    editedFileCount = result.editedFileCount
    totalTokens = result.totalTokens
    externalSessionRef =
      result.threadId === null ? null : stableRef("session.pylon.codex_composer", result.threadId)
    responseDigestRef =
      result.text.length === 0 ? null : stableRef("digest.pylon.control_session.response", result.text)
  } else {
    const config = await loadClaudeAgentConfig(input.summary)
    const result = await runClaudeComposerStream(
      input.objective,
      {
        abortSignal: input.abortSignal,
        account: input.account,
        config,
        cwd: input.cwd,
        env: input.env,
        executionMode: "local_bounded",
        ...(config.model === undefined ? {} : { model: config.model }),
        permissionMode: "acceptEdits",
        timeoutMs: input.timeoutMs,
        usageStateSummary: input.summary,
      },
      {
        onEvent: emitComposerEvent,
      },
    )
    eventCount = result.eventCount
    commandCount = result.commandCount
    editedFileCount = result.editedFileCount
    totalTokens = result.totalTokens
    externalSessionRef = result.sessionRef
    responseDigestRef =
      result.text.length === 0 ? null : stableRef("digest.pylon.control_session.response", result.text)
  }

  input.emit({ phase: "dev_check_started" })
  if (input.abortSignal.aborted) throw new Error("control session cancelled")
  const devCheck = await runPylonDevCheck({
    allowDetached: true,
    allowDirty: true,
    commands: [
      { argv: input.verify, cwd: input.cwd, reasonRef: "check.pylon.control_session_verification" },
    ],
    cwd: input.cwd,
    env: input.env,
    summary: input.summary,
  })

  return {
    commandCount,
    devCheck,
    editedFileCount,
    eventCount,
    externalSessionRef,
    responseDigestRef,
    totalTokens,
  }
}

export function createControlSessionActions(options: {
  env?: Record<string, string | undefined>
  executor?: ControlSessionExecutor
  proofsDir?: string
  summary: BootstrapSummary
}): ControlSessionActions {
  const encoder = new TextEncoder()
  const records = new Map<string, SessionRecord>()
  const subscribers = new Map<string, Set<ReadableStreamDefaultController<Uint8Array>>>()
  const executor = options.executor ?? defaultControlSessionExecutor
  const baseEnv = options.env ?? (Bun.env as Record<string, string | undefined>)
  const proofsDir = options.proofsDir ?? join(options.summary.paths.home, "proofs", "control-sessions")
  let spawnIndex = 0

  const sseFrame = (payload: unknown) => `data: ${JSON.stringify(payload)}\n\n`

  const publish = (record: SessionRecord, event: ControlSessionEvent) => {
    record.events.push(event)
    const frame = encoder.encode(sseFrame(event))
    for (const controller of subscribers.get(record.sessionRef) ?? []) {
      try {
        controller.enqueue(frame)
      } catch {
        subscribers.get(record.sessionRef)?.delete(controller)
      }
    }
    if (event.phase === "completed" || event.phase === "failed" || event.phase === "cancelled") {
      for (const controller of subscribers.get(record.sessionRef) ?? []) {
        try {
          controller.close()
        } catch {
          // already closed
        }
      }
      subscribers.delete(record.sessionRef)
    }
  }

  const emit = (
    record: SessionRecord,
    event: Omit<ControlSessionEvent, "schema" | "sessionRef" | "observedAt" | "eventIndex" | "state" | "adapter" | "account" | "workspaceRef">,
  ) => {
    const payload: ControlSessionEvent = {
      schema: CONTROL_SESSION_EVENT_SCHEMA,
      sessionRef: record.sessionRef,
      observedAt: nowIso(),
      eventIndex: record.events.length,
      state: record.state,
      adapter: record.adapter,
      account: publicPylonAccountSelection(record.account),
      workspaceRef: record.workspace.workspaceRef,
      ...event,
    }
    const violations = scanProofSerialization(JSON.stringify(payload))
    if (violations.length > 0) {
      publish(record, {
        schema: CONTROL_SESSION_EVENT_SCHEMA,
        sessionRef: record.sessionRef,
        observedAt: nowIso(),
        eventIndex: record.events.length,
        phase: "redaction_blocked",
        state: record.state,
        adapter: record.adapter,
        account: publicPylonAccountSelection(record.account),
        workspaceRef: record.workspace.workspaceRef,
        violationRefs: violations,
      })
      return
    }
    publish(record, payload)
  }

  const artifactPathFor = (record: SessionRecord, suffix: "proof" | "failure") =>
    join(proofsDir, `${record.sessionRef.replace(/[^a-zA-Z0-9._-]/g, "-")}-${suffix}.json`)

  const writeRetainedArtifact = async (
    record: SessionRecord,
    result: ControlSessionExecutorResult,
  ): Promise<string> => {
    const artifact = {
      schema: CONTROL_SESSION_ARTIFACT_SCHEMA,
      sessionRef: record.sessionRef,
      observedAt: record.startedAt,
      completedAt: record.completedAt,
      adapter: record.adapter,
      account: publicPylonAccountSelection(record.account),
      workspaceRef: record.workspace.workspaceRef,
      task: {
        objectiveDigestRef: record.objectiveDigestRef,
        verifyRef: record.verifyRef,
      },
      executor: {
        executionPathRef: "control_session.composer",
        executionMode: "local_bounded",
        outcome: result.devCheck.state === "passed" ? "completed" : "failed",
        eventCount: result.eventCount,
        commandCount: result.commandCount,
        editedFileCount: result.editedFileCount,
        totalTokens: result.totalTokens,
        externalSessionRef: result.externalSessionRef,
        responseDigestRef: result.responseDigestRef,
      },
      devCheck: result.devCheck,
      redactionScan: {
        state: "clean",
        patternRefs: PROOF_REDACTION_PATTERN_REFS,
      },
      deviations:
        result.devCheck.state === "passed"
          ? []
          : ["deviation.pylon.control_session.dev_check_not_passed"],
    }
    assertPublicProjectionSafe(artifact)
    const serialized = JSON.stringify(artifact, null, 2)
    const violations = scanProofSerialization(serialized)
    if (violations.length > 0) {
      throw new Error(`control session artifact failed redaction scan: ${violations.join(", ")}`)
    }
    await mkdir(proofsDir, { recursive: true })
    await writeFile(artifactPathFor(record, "proof"), `${serialized}\n`, "utf8")
    return stableRef("artifact.pylon.control_session.proof", `${record.sessionRef}:proof`)
  }

  const writeFailureArtifact = async (
    record: SessionRecord,
    failure: { errorClass: string; errorDigestRef: string },
  ): Promise<string> => {
    const artifact = {
      schema: CONTROL_SESSION_FAILURE_SCHEMA,
      sessionRef: record.sessionRef,
      generatedAt: record.completedAt,
      adapter: record.adapter,
      account: publicPylonAccountSelection(record.account),
      workspaceRef: record.workspace.workspaceRef,
      errorClass: failure.errorClass,
      errorDigestRef: failure.errorDigestRef,
      redactionScan: {
        state: "clean",
        patternRefs: PROOF_REDACTION_PATTERN_REFS,
      },
    }
    assertPublicProjectionSafe(artifact)
    const serialized = JSON.stringify(artifact, null, 2)
    const violations = scanProofSerialization(serialized)
    if (violations.length > 0) {
      throw new Error(`control session failure artifact failed redaction scan: ${violations.join(", ")}`)
    }
    await mkdir(proofsDir, { recursive: true })
    await writeFile(artifactPathFor(record, "failure"), `${serialized}\n`, "utf8")
    return stableRef("artifact.pylon.control_session.failure", `${record.sessionRef}:failure`)
  }

  const finishCancelled = (record: SessionRecord) => {
    if (record.state === "completed" || record.state === "failed" || record.state === "cancelled") return
    record.state = "cancelled"
    record.completedAt = nowIso()
    record.errorClass = "cancelled"
    record.errorDigestRef = stableRef("digest.pylon.control_session.cancelled", record.sessionRef)
    emit(record, {
      phase: "cancelled",
      errorClass: record.errorClass,
      errorDigestRef: record.errorDigestRef,
    })
  }

  const runSession = async (record: SessionRecord) => {
    if (record.abort.signal.aborted) {
      finishCancelled(record)
      return
    }
    record.state = "running"
    record.startedAt = nowIso()
    emit(record, { phase: "started" })
    try {
      const result = await executor({
        adapter: record.adapter,
        account: record.account,
        abortSignal: record.abort.signal,
        cwd: record.workspace.workingDirectory,
        env: pylonAccountEnvironment(baseEnv, record.account),
        emit: (event) => {
          const messageRef =
            event.message === undefined
              ? undefined
              : stableRef("message.pylon.control_session", `${record.sessionRef}:${event.message}`)
          emit(record, {
            phase: event.phase,
            ...(messageRef === undefined ? {} : { messageRef }),
            ...(event.composerEventIndex === undefined ? {} : { composerEventIndex: event.composerEventIndex }),
          })
        },
        objective: record.objective,
        sessionRef: record.sessionRef,
        summary: options.summary,
        timeoutMs: record.timeoutMs,
        verify: record.verify,
        workspaceRef: record.workspace.workspaceRef,
      })
      if (record.state === "cancelled" || record.abort.signal.aborted) return
      record.completedAt = nowIso()
      record.artifactRef = await writeRetainedArtifact(record, result)
      if (result.devCheck.state === "passed") {
        record.state = "completed"
        record.resultRef = stableRef("result.pylon.control_session", record.sessionRef)
        emit(record, { phase: "completed", artifactRef: record.artifactRef, resultRef: record.resultRef })
      } else {
        const failure = classifySessionError("dev check did not pass")
        record.state = "failed"
        record.errorClass = "verification_failed"
        record.errorDigestRef = failure.errorDigestRef
        emit(record, {
          phase: "failed",
          artifactRef: record.artifactRef,
          errorClass: record.errorClass,
          errorDigestRef: record.errorDigestRef,
        })
      }
    } catch (error) {
      if (record.state === "cancelled" || record.abort.signal.aborted) {
        finishCancelled(record)
        return
      }
      record.state = "failed"
      record.completedAt = nowIso()
      const failure = classifySessionError(error)
      record.errorClass = failure.errorClass
      record.errorDigestRef = failure.errorDigestRef
      try {
        record.artifactRef = await writeFailureArtifact(record, failure)
      } catch {
        record.artifactRef = null
      }
      emit(record, {
        phase: "failed",
        ...(record.artifactRef === null ? {} : { artifactRef: record.artifactRef }),
        errorClass: record.errorClass,
        errorDigestRef: record.errorDigestRef,
      })
    }
  }

  return {
    spawn: async (raw) => {
      const command = parseSpawnCommand(raw)
      const index = spawnIndex
      spawnIndex += 1
      const runRef = randomBytes(12).toString("hex")
      const provider = providerForAdapter(command.adapter)
      const accountHome =
        command.accountHome ??
        (command.adapter === "codex" ? command.codexHome : command.claudeConfigDir) ??
        undefined
      const account = await resolvePylonAccountSelection(options.summary, {
        provider,
        ...(command.accountRef === undefined ? {} : { accountRef: command.accountRef }),
        ...(accountHome === undefined ? {} : { accountHome }),
      })
      const workspace = await workspaceForCommand({ command, index, runRef, summary: options.summary })
      const sessionRef = stableRef(
        "session.pylon.control",
        `${runRef}:${command.adapter}:${command.objective}:${workspace.workspaceRef}`,
      )
      const record: SessionRecord = {
        sessionRef,
        adapter: command.adapter,
        account,
        workspace,
        objective: command.objective,
        objectiveDigestRef: stableRef("digest.pylon.control_session.objective", command.objective),
        verify: command.verify,
        verifyRef: stableRef("command.pylon.control_session.verify", command.verify.join("\0")),
        timeoutMs: (command.timeoutSeconds ?? 600) * 1000,
        state: "queued",
        abort: new AbortController(),
        createdAt: nowIso(),
        startedAt: null,
        completedAt: null,
        artifactRef: null,
        resultRef: null,
        errorClass: null,
        errorDigestRef: null,
        events: [],
      }
      records.set(sessionRef, record)
      emit(record, { phase: "queued" })
      void runSession(record)
      return { sessionRef, state: record.state }
    },
    list: async () =>
      [...records.values()]
        .sort((left, right) => left.createdAt.localeCompare(right.createdAt))
        .map(projectionFor),
    cancel: async (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      record.abort.abort()
      finishCancelled(record)
      return projectionFor(record)
    },
    events: async (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      return {
        sessionRef,
        eventsPath: `/sessions/${encodeURIComponent(sessionRef)}/events`,
        state: record.state,
      }
    },
    eventStream: (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      let streamController: ReadableStreamDefaultController<Uint8Array> | null = null
      return new ReadableStream<Uint8Array>({
        start(controller) {
          streamController = controller
          for (const event of record.events) {
            controller.enqueue(encoder.encode(sseFrame(event)))
          }
          if (record.state === "completed" || record.state === "failed" || record.state === "cancelled") {
            controller.close()
            return
          }
          const set = subscribers.get(sessionRef) ?? new Set<ReadableStreamDefaultController<Uint8Array>>()
          set.add(controller)
          subscribers.set(sessionRef, set)
        },
        cancel() {
          if (streamController) subscribers.get(sessionRef)?.delete(streamController)
        },
      })
    },
  }
}
