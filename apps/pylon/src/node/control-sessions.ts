import { createHash, randomBytes } from "node:crypto"
import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import {
  publicPylonAccountSelection,
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
  type PublicPylonAccountSelection,
  type PylonAccountProvider,
  type ResolvedPylonAccountSelection,
} from "../account-registry.js"
import { loadClaudeAgentConfig, loadClaudeDevConfig } from "../claude-agent.js"
import {
  permissionModeForClaudeComposerExecutionMode,
  runClaudeComposerStream,
} from "../claude-composer.js"
import {
  type CodexAgentConfig,
  type CodexDevConfig,
  loadCodexAgentConfig,
  loadCodexDevConfig,
  type PylonComposerAdapter,
} from "../codex-agent.js"
import {
  runCodexComposerStream,
  sandboxModeForCodexComposerExecutionMode,
} from "../codex-composer.js"
import type { BootstrapSummary } from "../bootstrap.js"
import {
  PYLON_DEV_CHECK_SCHEMA,
  recordPylonDevCodexRun,
  runPylonDevCheck,
  type PylonDevCheckProjection,
} from "../dev-loop.js"
import {
  PROOF_REDACTION_PATTERN_REFS,
  scanProofSerialization,
} from "../proof-redaction.js"
import { classifySessionError } from "../session-error-class.js"
import { ControlCommandValidationError } from "./control-command-error.js"
import { assertPublicProjectionSafe } from "../state.js"
import {
  materializeGitCheckoutWorkspaceWithLease,
  releaseWorkspace,
  type GitCheckoutWorkspace,
  type WorkspaceCheckoutRunner,
} from "../workspace-materializer.js"
import { estimateAppleFmLocalSessionEnergy } from "./apple-fm-energy-estimate.js"
import { collectPylonAppleFmStatus } from "./apple-fm-status.js"
import { runAppleFmLocalControlSession } from "./apple-fm-local-session.js"

export const CONTROL_SESSION_EVENT_SCHEMA = "openagents.pylon.control_session_event.v0.1"
export const CONTROL_SESSION_ARTIFACT_SCHEMA = "openagents.pylon.control_session_artifact.v0.1"
export const CONTROL_SESSION_FAILURE_SCHEMA = "openagents.pylon.control_session_failure.v0.1"

type ControlSessionRepositoryRef = GitCheckoutWorkspace["repository"]

// Requested execution lane for a control session (#4998). Owner direction:
//   - `auto`       — own-Pylon-first-and-free, then overflow to `cloud-gcp`
//   - `local`      — run on this local Pylon node (today's behavior)
//   - `cloud-gcp`  — OpenAgents Cloud on Google GCE (the default cloud lane)
//   - `cloud-shc`  — OpenAgents Cloud SHC capacity (the cloud fallback)
// Full cloud dispatch is tracked by #4997. Here we accept, default, persist, and
// surface the requested lane so it round-trips on the session record; `local`
// and `auto` (resolved local) execute as today.
export type ControlSessionLane = "auto" | "local" | "cloud-gcp" | "cloud-shc"
export const DEFAULT_CONTROL_SESSION_LANE: ControlSessionLane = "auto"
const CONTROL_SESSION_LANES: readonly ControlSessionLane[] = [
  "auto",
  "local",
  "cloud-gcp",
  "cloud-shc",
]

export type ControlSessionAdapter = PylonComposerAdapter | "apple_fm"

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
  lane?: ControlSessionLane
}

export type ControlSessionListCommand = { type: "session.list" }
export type ControlSessionEventsCommand = { type: "session.events"; sessionRef: string }
export type ControlSessionCancelCommand = { type: "session.cancel"; sessionRef: string }
export type ControlSessionArtifactCommand = { type: "session.artifact"; sessionRef: string }
export type ControlSessionReplyCommand = {
  type: "session.reply"
  sessionRef: string
  objective: string
  timeoutSeconds?: number
}
export type AppleFmSessionStartCommand = {
  type: "apple_fm.session.start"
  prompt: string
  worktreePath: string
  timeoutSeconds?: number
}

export type AppleFmSessionStartResult =
  | { ok: true; sessionRef: string; state: ControlSessionState }
  | {
      ok: false
      sessionRef: ""
      blockerRefs: string[]
      error: string
    }

export type ControlSessionCommand =
  | ControlSessionSpawnCommand
  | AppleFmSessionStartCommand
  | ControlSessionListCommand
  | ControlSessionEventsCommand
  | ControlSessionCancelCommand
  | ControlSessionArtifactCommand
  | ControlSessionReplyCommand

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
  adapter: ControlSessionAdapter
  account: PublicPylonAccountSelection | null
  workspaceRef: string
  messageRef?: string
  // Bounded, human-readable description of the composer event (the agent's
  // text / tool call / file change) so remote viewers can see live activity.
  // Passes the proof-serialization redaction scanner like every other field.
  messageText?: string
  composerEventIndex?: number
  artifactRef?: string
  resultRef?: string
  errorClass?: string
  errorDigestRef?: string
  workspaceCleanupReceiptRef?: string
  workspaceRetentionReasonRef?: string
  violationRefs?: string[]
}

export type ControlSessionProjection = {
  sessionRef: string
  parentSessionRef: string | null
  adapter: ControlSessionAdapter
  // Requested execution lane (#4998), surfaced for "running on Google GCE / SHC
  // / local" provenance. `auto`/`local` execute locally today; cloud lanes are
  // recorded pending full cloud dispatch (#4997).
  lane: ControlSessionLane
  account: PublicPylonAccountSelection | null
  accountRefHash: string | null
  objectiveRef: string
  workspaceRef: string
  workspaceCleanupRef: string | null
  workspaceCleanupReceiptRef: string | null
  workspaceRetentionReasonRef: string | null
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
  updatedAt: string
  eventCount: number
  // Latest human-readable action (for the session list — what it's doing now).
  latestActivity: string
  // #4997: cloud runner provenance for the "running on Google GCE / SHC"
  // indicator. `null` for local/auto-resolved-local sessions.
  cloudRunner: ControlSessionCloudRunner | null
  // #4997: the resource_usage_receipt ref surfaced by a cloud run, if any.
  resourceUsageReceiptRef: string | null
}

export type ControlSessionActions = {
  spawn: (command: ControlSessionSpawnCommand) => Promise<{ sessionRef: string; state: ControlSessionState }>
  reply: (command: ControlSessionReplyCommand) => Promise<{
    sessionRef: string
    parentSessionRef: string
    state: ControlSessionState
  }>
  startAppleFm: (command: AppleFmSessionStartCommand) => Promise<AppleFmSessionStartResult>
  list: () => Promise<ControlSessionProjection[]>
  cancel: (sessionRef: string) => Promise<ControlSessionProjection>
  events: (sessionRef: string) => Promise<{
    sessionRef: string
    eventsPath: string
    state: ControlSessionState
    // Bounded inline tail of the in-memory event log, so non-streaming clients
    // (RN fetch can't consume the SSE stream cleanly) can render a live
    // session-detail timeline by polling POST /command { session.events }.
    recentEvents: ControlSessionEvent[]
  }>
  // CL-19: the retained artifact (proof/failure) a completed session produced —
  // projection-safe + redaction-scanned at write time, so safe to return inline.
  artifact: (sessionRef: string) => Promise<{
    sessionRef: string
    kind: "proof" | "failure" | "none"
    artifact: unknown | null
  }>
  eventStream: (sessionRef: string) => ReadableStream<Uint8Array>
}

export type ControlSessionExecutorInput = {
  adapter: ControlSessionAdapter
  account: ResolvedPylonAccountSelection | null
  // Requested execution lane (#4998). Cloud executors (#4997) use this to pick
  // the cloud compute lane; the local executor ignores it.
  lane: ControlSessionLane
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
  executionMode?: "local_bounded" | "local_supervised_danger"
  externalSessionRef: string | null
  networkAccessEnabled?: boolean
  permissionMode?: "acceptEdits" | "bypassPermissions"
  responseDigestRef: string | null
  sandboxMode?: "read-only" | "workspace-write" | "danger-full-access"
  totalTokens: number
  // #4997: when the session ran on a cloud runner, the resolved runner
  // provenance ("running on Google GCE / SHC") and the surfaced
  // resource_usage_receipt ref, so the desktop indicator is real.
  cloudRunner?: ControlSessionCloudRunner
  resourceUsageReceiptRef?: string | null
}

// #4997: bounded, refs-and-limits-only provenance for a cloud-executed session.
// No raw owner identity, cost, GCP project id, instance name, IP, credentials,
// or topology — only the lane and the runner id/label.
export type ControlSessionCloudRunner = {
  lane: "cloud-gcp" | "cloud-shc"
  providerLane: "gcp" | "shc"
  runnerId: string
  externalRunId: string
}

export type ControlSessionExecutor = (
  input: ControlSessionExecutorInput,
) => Promise<ControlSessionExecutorResult>

type WorkspaceSelection = {
  workspaceRef: string
  workingDirectory: string
  cleanupRef?: string
  workspaceStateRoot?: string
}

// Owner-local "no sandbox" opt-in for the desktop's own embedded node.
//
// The desktop app launches its own authenticated local Pylon node and forwards
// `PYLON_CODEX_NO_SANDBOX=1` into the node child env (node-launcher.ts). When set,
// a Codex control session runs full-access with network enabled — Codex can use
// git / GitHub / credentials, which a sandboxed session cannot (that no-network
// sandbox is exactly why "Codex didn't connect to GitHub").
//
// This honors the workspace INVARIANT: "authenticated local control sessions
// may honor the local dev overlay" for danger modes. It is a node-boot env on
// the OWNER's local machine — NOT a `session.spawn` wire field — so the
// `rejectDangerFields` wire-defense (a remote spawn cannot force danger) stays
// fully intact.
export function codexControlSessionNoSandboxOptIn(
  env: Record<string, string | undefined> | undefined,
): boolean {
  const value = env?.PYLON_CODEX_NO_SANDBOX?.trim().toLowerCase()
  return value === "1" || value === "true" || value === "yes"
}

export function codexControlSessionExecutionSettings(
  config: Pick<CodexAgentConfig, "sandboxMode">,
  devConfig: Pick<CodexDevConfig, "codexExecutionMode">,
  env?: Record<string, string | undefined>,
): {
  executionMode: "local_bounded" | "local_supervised_danger"
  networkAccessEnabled: boolean
  sandboxMode: "read-only" | "workspace-write" | "danger-full-access"
} {
  const executionMode =
    devConfig.codexExecutionMode === "local_supervised_danger" ||
    codexControlSessionNoSandboxOptIn(env)
      ? "local_supervised_danger"
      : "local_bounded"
  return {
    executionMode,
    networkAccessEnabled: executionMode === "local_supervised_danger",
    sandboxMode: sandboxModeForCodexComposerExecutionMode(executionMode, config.sandboxMode),
  }
}

type SessionRecord = {
  sessionRef: string
  parentSessionRef: string | null
  adapter: ControlSessionAdapter
  lane: ControlSessionLane
  account: ResolvedPylonAccountSelection | null
  workspace: WorkspaceSelection
  workspaceCleanupReceiptRef: string | null
  workspaceRetentionReasonRef: string | null
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
  cloudRunner: ControlSessionCloudRunner | null
  resourceUsageReceiptRef: string | null
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

// `record.state` is mutated concurrently by `cancel()` across `await` points,
// so a literal-narrowed read (after e.g. `record.state = "running"`) does not
// reflect the value that may have changed during the await. Read it through
// this helper so callers always see the full ControlSessionState union.
function currentSessionState(record: SessionRecord): ControlSessionState {
  return record.state
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
  if (!/^[A-Za-z0-9_./-]+$/.test(record.branch) || record.branch.includes("..") || record.branch.startsWith("-")) return null
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
    throw new ControlCommandValidationError(
      "danger_mode_rejected",
      "control sessions reject local danger modes",
    )
  }
}

function parseLane(value: unknown): ControlSessionLane {
  if (value === undefined || value === null) return DEFAULT_CONTROL_SESSION_LANE
  if (
    typeof value === "string" &&
    (CONTROL_SESSION_LANES as readonly string[]).includes(value)
  ) {
    return value as ControlSessionLane
  }
  throw new ControlCommandValidationError(
    "lane_invalid",
    "session.spawn lane must be one of auto|local|cloud-gcp|cloud-shc",
  )
}

function parseSpawnCommand(raw: ControlSessionSpawnCommand): ControlSessionSpawnCommand {
  const record = raw as Record<string, unknown>
  rejectDangerFields(record)
  if (raw.adapter !== "codex" && raw.adapter !== "claude_agent") {
    throw new ControlCommandValidationError(
      "adapter_invalid",
      "session.spawn requires adapter codex or claude_agent",
    )
  }
  if (typeof raw.objective !== "string" || raw.objective.trim().length === 0) {
    throw new ControlCommandValidationError(
      "objective_required",
      "session.spawn requires a non-empty objective",
    )
  }
  const verify = stringArray(raw.verify)
  if (verify === null) {
    throw new ControlCommandValidationError(
      "verify_required",
      "session.spawn requires non-empty verify argv",
    )
  }
  const repoRef = raw.repoRef === undefined ? undefined : repositoryRefFrom(raw.repoRef)
  if (repoRef === null) {
    throw new ControlCommandValidationError(
      "repo_ref_invalid",
      "session.spawn repoRef is invalid",
    )
  }
  const worktreePath =
    typeof raw.worktreePath === "string"
      ? raw.worktreePath
      : typeof raw.worktree === "string"
        ? raw.worktree
        : raw.worktree && typeof raw.worktree === "object" && typeof raw.worktree.path === "string"
          ? raw.worktree.path
          : undefined
  // #5453: a Blueprint chat turn dispatches `session.spawn` with NO workspace
  // selector (no repoRef, no worktreePath) — it is a conversational program
  // turn, not a repo-checkout coding task. That is now valid: the node
  // materializes a private ephemeral scratch workspace for it (see
  // `workspaceForCommand` -> `workspaceForEphemeralScratch`). Previously this
  // threw, and the control server turned the throw into a raw HTTP 500
  // (`control 500`) in the desktop chat composer. Only reject the genuinely
  // ambiguous case of BOTH selectors set at once.
  if (repoRef !== undefined && worktreePath !== undefined) {
    throw new ControlCommandValidationError(
      "workspace_selector_conflict",
      "session.spawn must use only one workspace selector",
    )
  }
  const lane = parseLane(raw.lane)
  return {
    type: "session.spawn",
    adapter: raw.adapter,
    lane,
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

function parseReplyCommand(raw: ControlSessionReplyCommand): ControlSessionReplyCommand {
  const record = raw as Record<string, unknown>
  rejectDangerFields(record)
  if (typeof raw.sessionRef !== "string" || raw.sessionRef.trim().length === 0) {
    throw new ControlCommandValidationError(
      "session_ref_required",
      "session.reply requires --session-ref",
    )
  }
  if (typeof raw.objective !== "string" || raw.objective.trim().length === 0) {
    throw new ControlCommandValidationError(
      "objective_required",
      "session.reply requires a non-empty objective",
    )
  }
  return {
    type: "session.reply",
    sessionRef: raw.sessionRef.trim(),
    objective: raw.objective,
    ...(typeof raw.timeoutSeconds === "number" && Number.isFinite(raw.timeoutSeconds)
      ? { timeoutSeconds: Math.max(1, Math.min(1200, Math.floor(raw.timeoutSeconds))) }
      : {}),
  }
}

function buildContinuationObjective(priorObjective: string, followUp: string): string {
  const trimmedFollowUp = followUp.trim()
  const prior = priorObjective.trim()
  if (prior.length === 0) return trimmedFollowUp
  return [
    "Continue the current coding session. Earlier turns in this thread:",
    `1. ${prior}`,
    "",
    "Next instruction:",
    trimmedFollowUp,
  ].join("\n")
}

function parseAppleFmStartCommand(raw: AppleFmSessionStartCommand): AppleFmSessionStartCommand {
  const record = raw as Record<string, unknown>
  rejectDangerFields(record)
  if (typeof raw.prompt !== "string" || raw.prompt.trim().length === 0) {
    throw new ControlCommandValidationError(
      "prompt_required",
      "apple_fm.session.start requires a non-empty prompt",
    )
  }
  if (typeof raw.worktreePath !== "string" || raw.worktreePath.trim().length === 0) {
    throw new ControlCommandValidationError(
      "worktree_path_required",
      "apple_fm.session.start requires worktreePath",
    )
  }
  return {
    type: "apple_fm.session.start",
    prompt: raw.prompt,
    worktreePath: raw.worktreePath,
    ...(typeof raw.timeoutSeconds === "number" && Number.isFinite(raw.timeoutSeconds)
      ? { timeoutSeconds: Math.max(1, Math.min(600, Math.floor(raw.timeoutSeconds))) }
      : {}),
  }
}

function projectionFor(record: SessionRecord): ControlSessionProjection {
  const account = publicPylonAccountSelection(record.account)
  return {
    sessionRef: record.sessionRef,
    parentSessionRef: record.parentSessionRef,
    adapter: record.adapter,
    lane: record.lane,
    account,
    accountRefHash: account?.accountRefHash ?? null,
    objectiveRef: record.objectiveDigestRef,
    workspaceRef: record.workspace.workspaceRef,
    workspaceCleanupRef: record.workspace.cleanupRef ?? null,
    workspaceCleanupReceiptRef: record.workspaceCleanupReceiptRef,
    workspaceRetentionReasonRef: record.workspaceRetentionReasonRef,
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
    updatedAt: record.completedAt ?? record.startedAt ?? record.createdAt,
    eventCount: record.events.length,
    latestActivity: latestActivityOf(record.events),
    cloudRunner: record.cloudRunner,
    resourceUsageReceiptRef: record.resourceUsageReceiptRef,
  }
}

// A one-line "what is it doing right now" for the session list: the most recent
// event that carries a human-readable action (agent text / tool call / file
// change), falling back to the latest phase.
function latestActivityOf(events: ControlSessionEvent[]): string {
  for (let i = events.length - 1; i >= 0; i--) {
    const text = events[i].messageText
    if (typeof text === "string" && text.length > 0) return text.slice(0, 160)
  }
  const last = events[events.length - 1]
  return last ? last.phase : ""
}

async function workspaceForCommand(input: {
  command: ControlSessionSpawnCommand
  index: number
  runRef: string
  summary: BootstrapSummary
  workspaceCheckoutRunner?: WorkspaceCheckoutRunner
}): Promise<WorkspaceSelection> {
  if (input.command.worktreePath !== undefined) {
    return workspaceForWorktreePath(input.command.worktreePath)
  }

  const repoRef = input.command.repoRef
  if (repoRef === undefined) {
    // #5453: workspace-less turn (e.g. a Blueprint chat turn). No repo target
    // and no caller-supplied path, so materialize a fresh private scratch
    // directory under the node's cache for the bounded session to run in.
    return workspaceForEphemeralScratch({
      cacheRoot: join(input.summary.paths.cache, "control-session-scratch"),
      runRef: input.runRef,
      index: input.index,
    })
  }
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
    ...(input.workspaceCheckoutRunner === undefined ? {} : { checkoutRunner: input.workspaceCheckoutRunner }),
    leaseRef: stableRef("lease.pylon.control_session.workspace", `${input.runRef}:${input.index}`),
    refPrefix: "workspace.pylon.control_session",
    repositoryCacheRoot: join(input.summary.paths.cache, "workspace-git-cache"),
    retentionPolicy: "remove_on_closeout",
    workspaceStateRoot: join(input.summary.paths.cache, "workspace-leases"),
  })
  return {
    cleanupRef: materialized.cleanupRef,
    workingDirectory: materialized.workingDirectory,
    workspaceStateRoot: join(input.summary.paths.cache, "workspace-leases"),
    workspaceRef: materialized.workspaceRef,
  }
}

// #5453: materialize a fresh private scratch working directory for a
// workspace-less control session (a Blueprint chat turn has no repo and no
// caller-supplied path). The directory lives under the node's cache so it never
// touches the user's repos, and a deterministic ref keys it for cleanup. We do
// NOT register a workspace lease here (there is no checkout to retain); the
// directory is bounded scratch for the bounded session.
async function workspaceForEphemeralScratch(input: {
  cacheRoot: string
  runRef: string
  index: number
}): Promise<WorkspaceSelection> {
  const workspaceRef = stableRef(
    "workspace.pylon.control_session.scratch",
    `${input.runRef}:${input.index}`,
  )
  const workingDirectory = join(input.cacheRoot, workspaceRef)
  await mkdir(workingDirectory, { recursive: true })
  return { workingDirectory, workspaceRef }
}

async function workspaceForWorktreePath(worktreePath: string): Promise<WorkspaceSelection> {
  const workingDirectory = resolve(worktreePath)
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

function appleFmLocalSessionCheck(input: ControlSessionExecutorInput): PylonDevCheckProjection {
  return {
    schema: PYLON_DEV_CHECK_SCHEMA,
    observedAt: nowIso(),
    action: "check",
    state: "passed",
    changeSummary: {
      repo: {
        state: "not_git",
        rootRef: null,
        branch: null,
        commit: null,
      },
      dirty: {
        state: "unknown",
        changedCount: 0,
        stagedCount: 0,
        unstagedCount: 0,
        untrackedCount: 0,
      },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: [],
    },
    checkPlan: {
      state: "ready",
      commandRefs: ["command.pylon.apple_fm.local_chat_tool_session"],
      blockerRefs: [],
    },
    commandResults: [
      {
        commandRef: "command.pylon.apple_fm.local_chat_tool_session",
        reasonRef: "check.pylon.apple_fm.local_chat_tool_session",
        cwdRef: stableRef("command.cwd", input.cwd),
        argvRef: "command.argv.pylon.apple_fm.local_chat_tool_session",
        exitCode: 0,
        status: "passed",
        durationMs: 0,
        stdoutBytes: 0,
        stderrBytes: 0,
        stdoutDigestRef: null,
        stderrDigestRef: null,
      },
    ],
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: [],
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
  let executionMode: "local_bounded" | "local_supervised_danger" = "local_bounded"
  let networkAccessEnabled: boolean | undefined
  let permissionMode: "acceptEdits" | "bypassPermissions" | undefined
  let sandboxMode: "read-only" | "workspace-write" | "danger-full-access" | undefined

  if (input.adapter === "apple_fm") {
    const result = await runAppleFmLocalControlSession(input)
    input.emit({
      phase: "composer_event",
      message: "control session mode: local_bounded; adapter: apple_fm; sandbox: read-only; network: disabled",
      composerEventIndex: result.eventCount + 1,
    })
    return {
      ...result,
      devCheck: appleFmLocalSessionCheck(input),
    }
  } else if (input.adapter === "codex") {
    const config = await loadCodexAgentConfig(input.summary)
    const devConfig = await loadCodexDevConfig(input.summary)
    const settings = codexControlSessionExecutionSettings(config, devConfig, input.env)
    executionMode = settings.executionMode
    sandboxMode = settings.sandboxMode
    networkAccessEnabled = settings.networkAccessEnabled
    const result = await runCodexComposerStream(
      input.objective,
      {
        abortSignal: input.abortSignal,
        account: input.account,
        approvalPolicy: "never",
        config,
        cwd: input.cwd,
        env: input.env,
        executionMode,
        humanReadableReasoning: true,
        ...(config.model === undefined ? {} : { model: config.model }),
        networkAccessEnabled,
        sandboxMode,
        timeoutMs: input.timeoutMs,
        usageStateSummary: input.summary,
      },
      {
        onEvent: emitComposerEvent,
        onThreadId: (_threadId, ref) => {
          externalSessionRef = ref
          input.emit({
            phase: "composer_event",
            message: `external session: ${ref}`,
            composerEventIndex: eventCount + 1,
          })
        },
      },
    )
    await recordPylonDevCodexRun(
      {
        commandCount: result.commandCount,
        cwd: input.cwd,
        editedFileCount: result.editedFileCount,
        eventCount: result.eventCount,
        executionMode,
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
      result.threadId === null ? externalSessionRef : stableRef("session.pylon.codex_composer", result.threadId)
    responseDigestRef =
      result.text.length === 0 ? null : stableRef("digest.pylon.control_session.response", result.text)
    input.emit({
      phase: "composer_event",
      message: `control session mode: ${executionMode}; sandbox: ${sandboxMode}; network: ${
        networkAccessEnabled ? "enabled" : "disabled"
      }`,
      composerEventIndex: result.eventCount + 1,
    })
  } else {
    const config = await loadClaudeAgentConfig(input.summary)
    const devConfig = await loadClaudeDevConfig(input.summary)
    executionMode =
      devConfig.claudeExecutionMode === "local_supervised_danger"
        ? "local_supervised_danger"
        : "local_bounded"
    permissionMode = permissionModeForClaudeComposerExecutionMode(executionMode)
    const result = await runClaudeComposerStream(
      input.objective,
      {
        abortSignal: input.abortSignal,
        account: input.account,
        config,
        cwd: input.cwd,
        env: input.env,
        executionMode,
        ...(config.model === undefined ? {} : { model: config.model }),
        permissionMode,
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
    input.emit({
      phase: "composer_event",
      message: `control session mode: ${executionMode}; permissions: ${permissionMode}`,
      composerEventIndex: result.eventCount + 1,
    })
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
    executionMode,
    externalSessionRef,
    ...(networkAccessEnabled === undefined ? {} : { networkAccessEnabled }),
    ...(permissionMode === undefined ? {} : { permissionMode }),
    responseDigestRef,
    ...(sandboxMode === undefined ? {} : { sandboxMode }),
    totalTokens,
  }
}

export function createControlSessionActions(options: {
  appleFmFetch?: typeof fetch
  appleFmNow?: Date
  env?: Record<string, string | undefined>
  executor?: ControlSessionExecutor
  workspaceCheckoutRunner?: WorkspaceCheckoutRunner
  // #4997: optional cloud executor used when a session's lane resolves to a
  // cloud lane and a cloud control plane is configured. When omitted, the
  // factory builds one from env via `cloudExecutorFactory`. When neither is
  // available (no cloud config), cloud lanes fall back to the local executor so
  // a Pylon with no cloud config still works locally exactly as before.
  cloudExecutor?: ControlSessionExecutor
  cloudExecutorFactory?: (env: Record<string, string | undefined>) => ControlSessionExecutor | null
  proofsDir?: string
  summary: BootstrapSummary
}): ControlSessionActions {
  const encoder = new TextEncoder()
  const records = new Map<string, SessionRecord>()
  const subscribers = new Map<string, Set<ReadableStreamDefaultController<Uint8Array>>>()
  const executor = options.executor ?? defaultControlSessionExecutor
  const baseEnv = options.env ?? (Bun.env as Record<string, string | undefined>)
  // Resolve the cloud executor lazily: explicit injection wins (tests), else a
  // factory-from-env builds one when cloud config is present. `null` means "no
  // cloud configured" and cloud lanes degrade to the local executor.
  const cloudExecutor =
    options.cloudExecutor ??
    (options.cloudExecutorFactory ? options.cloudExecutorFactory(baseEnv) : null)
  const laneIsCloud = (lane: ControlSessionLane): boolean =>
    lane === "cloud-gcp" || lane === "cloud-shc"
  const selectExecutor = (lane: ControlSessionLane): ControlSessionExecutor => {
    if (laneIsCloud(lane) && cloudExecutor) return cloudExecutor
    return executor
  }
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
    const energyEstimate = record.adapter === "apple_fm"
      ? estimateAppleFmLocalSessionEnergy({
          env: baseEnv,
          startedAt: record.startedAt,
          completedAt: record.completedAt,
        })
      : undefined
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
        executionPathRef: record.adapter === "apple_fm"
          ? "control_session.apple_fm_local"
          : "control_session.composer",
        executionMode: result.executionMode ?? "local_bounded",
        ...(result.sandboxMode === undefined ? {} : { sandboxMode: result.sandboxMode }),
        ...(result.permissionMode === undefined ? {} : { permissionMode: result.permissionMode }),
        ...(result.networkAccessEnabled === undefined
          ? {}
          : { networkAccessEnabled: result.networkAccessEnabled }),
        outcome: result.devCheck.state === "passed" ? "completed" : "failed",
        eventCount: result.eventCount,
        commandCount: result.commandCount,
        editedFileCount: result.editedFileCount,
        totalTokens: result.totalTokens,
        externalSessionRef: result.externalSessionRef,
        responseDigestRef: result.responseDigestRef,
        ...(energyEstimate === undefined ? {} : { energyEstimate }),
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

  const releaseManagedWorkspace = async (record: SessionRecord) => {
    if (record.workspace.workspaceStateRoot === undefined) return
    if (record.workspaceCleanupReceiptRef !== null || record.workspaceRetentionReasonRef !== null) return
    const result = await releaseWorkspace({
      workspaceStateRoot: record.workspace.workspaceStateRoot,
      workspaceRef: record.workspace.workspaceRef,
    })
    if (result?.cleanupReceiptRef !== undefined) {
      record.workspaceCleanupReceiptRef = result.cleanupReceiptRef
    }
    if (result?.retentionReasonRef !== undefined) {
      record.workspaceRetentionReasonRef = result.retentionReasonRef
    }
  }

  const workspaceCleanupEventFields = (record: SessionRecord) => ({
    ...(record.workspaceCleanupReceiptRef === null
      ? {}
      : { workspaceCleanupReceiptRef: record.workspaceCleanupReceiptRef }),
    ...(record.workspaceRetentionReasonRef === null
      ? {}
      : { workspaceRetentionReasonRef: record.workspaceRetentionReasonRef }),
  })

  const finishCancelled = async (record: SessionRecord) => {
    if (record.state === "completed" || record.state === "failed" || record.state === "cancelled") return
    record.completedAt = nowIso()
    record.errorClass = "cancelled"
    record.errorDigestRef = stableRef("digest.pylon.control_session.cancelled", record.sessionRef)
    await releaseManagedWorkspace(record)
    record.state = "cancelled"
    emit(record, {
      phase: "cancelled",
      errorClass: record.errorClass,
      errorDigestRef: record.errorDigestRef,
      ...workspaceCleanupEventFields(record),
    })
  }

  const runSession = async (record: SessionRecord) => {
    if (record.abort.signal.aborted) {
      await finishCancelled(record)
      return
    }
    record.state = "running"
    record.startedAt = nowIso()
    emit(record, { phase: "started" })
    try {
      const result = await selectExecutor(record.lane)({
        adapter: record.adapter,
        account: record.account,
        lane: record.lane,
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
            ...(event.message === undefined ? {} : { messageText: event.message.slice(0, 2000) }),
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
      if (currentSessionState(record) === "cancelled" || record.abort.signal.aborted) return
      if (result.cloudRunner !== undefined) record.cloudRunner = result.cloudRunner
      if (result.resourceUsageReceiptRef !== undefined && result.resourceUsageReceiptRef !== null) {
        record.resourceUsageReceiptRef = result.resourceUsageReceiptRef
      }
      record.completedAt = nowIso()
      record.artifactRef = await writeRetainedArtifact(record, result)
      if (result.devCheck.state === "passed") {
        record.resultRef = stableRef("result.pylon.control_session", record.sessionRef)
        await releaseManagedWorkspace(record)
        record.state = "completed"
        emit(record, {
          phase: "completed",
          artifactRef: record.artifactRef,
          resultRef: record.resultRef,
          ...workspaceCleanupEventFields(record),
        })
      } else {
        const failure = classifySessionError("dev check did not pass")
        record.errorClass = "verification_failed"
        record.errorDigestRef = failure.errorDigestRef
        await releaseManagedWorkspace(record)
        record.state = "failed"
        emit(record, {
          phase: "failed",
          artifactRef: record.artifactRef,
          errorClass: record.errorClass,
          errorDigestRef: record.errorDigestRef,
          ...workspaceCleanupEventFields(record),
        })
      }
    } catch (error) {
      if (currentSessionState(record) === "cancelled" || record.abort.signal.aborted) {
        await finishCancelled(record)
        return
      }
      record.completedAt = nowIso()
      const failure = classifySessionError(error)
      record.errorClass = failure.errorClass
      record.errorDigestRef = failure.errorDigestRef
      try {
        record.artifactRef = await writeFailureArtifact(record, failure)
      } catch {
        record.artifactRef = null
      }
      await releaseManagedWorkspace(record)
      record.state = "failed"
      emit(record, {
        phase: "failed",
        ...(record.artifactRef === null ? {} : { artifactRef: record.artifactRef }),
        errorClass: record.errorClass,
        errorDigestRef: record.errorDigestRef,
        ...workspaceCleanupEventFields(record),
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
      const workspace = await workspaceForCommand({
        command,
        index,
        runRef,
        summary: options.summary,
        ...(options.workspaceCheckoutRunner === undefined
          ? {}
          : { workspaceCheckoutRunner: options.workspaceCheckoutRunner }),
      })
      const sessionRef = stableRef(
        "session.pylon.control",
        `${runRef}:${command.adapter}:${command.objective}:${workspace.workspaceRef}`,
      )
      const record: SessionRecord = {
        sessionRef,
        parentSessionRef: null,
        adapter: command.adapter,
        lane: command.lane ?? DEFAULT_CONTROL_SESSION_LANE,
        account,
        workspace,
        workspaceCleanupReceiptRef: null,
        workspaceRetentionReasonRef: null,
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
        cloudRunner: null,
        resourceUsageReceiptRef: null,
      }
      records.set(sessionRef, record)
      emit(record, { phase: "queued" })
      void runSession(record)
      return { sessionRef, state: record.state }
    },
    reply: async (raw) => {
      const command = parseReplyCommand(raw)
      const parent = records.get(command.sessionRef)
      if (!parent) throw new Error("session.reply parent session not found")
      const runRef = randomBytes(12).toString("hex")
      const objective = buildContinuationObjective(parent.objective, command.objective)
      const sessionRef = stableRef(
        "session.pylon.control",
        `${runRef}:${parent.adapter}:${parent.sessionRef}:${objective}:${parent.workspace.workspaceRef}`,
      )
      const record: SessionRecord = {
        sessionRef,
        parentSessionRef: parent.sessionRef,
        adapter: parent.adapter,
        lane: parent.lane,
        account: parent.account,
        workspace: parent.workspace,
        workspaceCleanupReceiptRef: null,
        workspaceRetentionReasonRef: null,
        objective,
        objectiveDigestRef: stableRef("digest.pylon.control_session.objective", objective),
        verify: [...parent.verify],
        verifyRef: parent.verifyRef,
        timeoutMs: (command.timeoutSeconds ?? Math.ceil(parent.timeoutMs / 1000)) * 1000,
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
        cloudRunner: null,
        resourceUsageReceiptRef: null,
      }
      records.set(sessionRef, record)
      emit(record, { phase: "queued", messageText: "Continuation session queued" })
      void runSession(record)
      return { sessionRef, parentSessionRef: parent.sessionRef, state: record.state }
    },
    startAppleFm: async (raw) => {
      const command = parseAppleFmStartCommand(raw)
      const status = await collectPylonAppleFmStatus({
        summary: options.summary,
        env: baseEnv,
        fetch: options.appleFmFetch,
        now: options.appleFmNow,
      })
      const ready =
        status.available &&
        status.status === "ready" &&
        status.advertisedCapabilities.includes(status.capability) &&
        status.blockerRefs.length === 0
      if (!ready) {
        return {
          ok: false,
          sessionRef: "",
          blockerRefs: status.blockerRefs.length > 0
            ? [...status.blockerRefs]
            : ["blocker.pylon.apple_fm.not_ready"],
          error: status.message ?? status.unavailableReason ?? status.status,
        }
      }
      const index = spawnIndex
      spawnIndex += 1
      const runRef = randomBytes(12).toString("hex")
      const workspace = await workspaceForWorktreePath(command.worktreePath)
      const sessionRef = stableRef(
        "session.pylon.apple_fm",
        `${runRef}:apple_fm:${command.prompt}:${workspace.workspaceRef}`,
      )
      const record: SessionRecord = {
        sessionRef,
        parentSessionRef: null,
        adapter: "apple_fm",
        lane: "local",
        account: null,
        workspace,
        workspaceCleanupReceiptRef: null,
        workspaceRetentionReasonRef: null,
        objective: command.prompt,
        objectiveDigestRef: stableRef("digest.pylon.apple_fm.prompt", command.prompt),
        verify: ["bun", "--version"],
        verifyRef: stableRef("command.pylon.control_session.verify", "bun\0--version"),
        timeoutMs: (command.timeoutSeconds ?? 300) * 1000,
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
        cloudRunner: null,
        resourceUsageReceiptRef: null,
      }
      records.set(sessionRef, record)
      emit(record, {
        phase: "queued",
        messageText: "Apple FM local session queued",
      })
      void runSession(record)
      return { ok: true, sessionRef, state: record.state }
    },
    list: async () =>
      [...records.values()]
        .sort((left, right) => left.createdAt.localeCompare(right.createdAt))
        .map(projectionFor),
    cancel: async (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      record.abort.abort()
      await finishCancelled(record)
      return projectionFor(record)
    },
    events: async (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      return {
        sessionRef,
        eventsPath: `/sessions/${encodeURIComponent(sessionRef)}/events`,
        state: record.state,
        recentEvents: record.events.slice(-100),
      }
    },
    artifact: async (sessionRef) => {
      const record = records.get(sessionRef)
      if (!record) throw new Error("session not found")
      for (const kind of ["proof", "failure"] as const) {
        try {
          const raw = await readFile(artifactPathFor(record, kind), "utf8")
          return { sessionRef, kind, artifact: JSON.parse(raw) }
        } catch {
          // not this kind / not yet written
        }
      }
      return { sessionRef, kind: "none", artifact: null }
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
