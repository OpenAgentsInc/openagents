import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { readFile, stat } from "node:fs/promises"
import { dirname, join, relative, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import {
  loadClaudeAgentConfig,
  loadClaudeDevConfig,
  probeClaudeAgentReadiness,
  type ClaudeAgentReadiness,
} from "./claude-agent.js"
import {
  loadCodexAgentConfig,
  loadCodexDevConfig,
  probeCodexAgentReadiness,
  type CodexAgentReadiness,
  type PylonComposerAdapter,
} from "./codex-agent.js"
import {
  sandboxModeForCodexComposerExecutionMode,
  type CodexComposerExecutionMode,
} from "./codex-composer.js"
import {
  CLAUDE_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF,
  permissionModeForClaudeComposerExecutionMode,
  type ClaudeComposerExecutionMode,
  type ClaudeComposerPermissionMode,
} from "./claude-composer.js"
import { createBootstrapSummary, parseBootstrapArgs, type BootstrapSummary } from "./bootstrap.js"
import { discoverHostInventory, type PylonBackendHealth, type PylonHostInventoryProjection } from "./inventory.js"
import { collectPylonAccountUsageSummary, type PylonAccountUsageSummary } from "./account-usage.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_DEV_DOCTOR_SCHEMA = "openagents.pylon.dev_doctor.v0.3"

export type PylonDevDoctorInstructionRef = {
  sourceRef: string
  state: "present" | "missing"
  relativePath: string
  digestRef: string | null
}

export type PylonDevDoctorProjection = {
  schema: typeof PYLON_DEV_DOCTOR_SCHEMA
  observedAt: string
  package: {
    name: "@openagentsinc/pylon"
    version: string
    sourceCommit: string | null
  }
  repo: {
    state: "ready" | "not_git" | "unknown"
    provider: "github" | "unknown" | null
    fullName: string | null
    branch: string | null
    commit: string | null
    dirty: {
      state: "clean" | "dirty" | "unknown"
      changedCount: number
    }
    blockerRefs: string[]
  }
  instructions: {
    refs: PylonDevDoctorInstructionRef[]
    blockerRefs: string[]
  }
  pylonConfig: {
    state: "present" | "missing"
    configRef: "config.pylon.local"
    digestRef: string | null
    devOverlayRef: "config.pylon.dev.local_supervised_danger" | null
    claudeDevOverlayRef: "config.pylon.dev.claude_local_supervised_danger" | null
    defaultAdapter: PylonComposerAdapter
  }
  codex: {
    cli: "present" | "missing"
    sdkReadiness: CodexAgentReadiness
    credentialSourceRef: string | null
    configuredModel: string | null
    executionMode: CodexComposerExecutionMode
    sandboxMode: "read-only" | "workspace-write" | "danger-full-access"
    blockerRefs: string[]
  }
  claudeAgent: {
    readiness: ClaudeAgentReadiness
    configuredModel: string | null
    fableReviewAvailable: boolean
    executionMode: ClaudeComposerExecutionMode
    permissionMode: ClaudeComposerPermissionMode
    /**
     * Set while the permissive mode is active: the typed blocker every
     * public path (work/assignment/provider/node/attach) throws if the
     * danger flag reaches it. The danger mode is local-composer-only.
     */
    dangerPublicPathBlockerRef: string | null
    blockerRefs: string[]
  }
  backends: {
    refs: Pick<PylonBackendHealth, "backendRef" | "state" | "modelRef" | "blockerRefs">[]
    blockerRefs: string[]
  }
  usage?: PylonAccountUsageSummary | null
  blockerRefs: string[]
}

export type PylonDevDoctorOptions = {
  cwd?: string
  env?: Record<string, string | undefined>
  now?: Date
  summary?: BootstrapSummary
  codexImporter?: (specifier: string) => Promise<unknown>
  claudeImporter?: (specifier: string) => Promise<unknown>
  codexCliPath?: string | null
  codexCliLoginPresent?: boolean
  localClaudeSessionProbe?: () => Promise<boolean>
  inventory?: PylonHostInventoryProjection
  gitRunner?: (cwd: string, args: string[]) => Promise<string | null>
  dangerFlag?: boolean
  claudeDangerFlag?: boolean
}

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function digestRef(prefix: string, bytes: string) {
  return stableRef(prefix, bytes)
}

async function defaultGitRunner(cwd: string, args: string[]): Promise<string | null> {
  try {
    const proc = Bun.spawn(["git", ...args], {
      cwd,
      stderr: "ignore",
      stdout: "pipe",
    })
    const [stdout, exitCode] = await Promise.all([new Response(proc.stdout).text(), proc.exited])
    if (exitCode !== 0) return null
    return stdout.trim()
  } catch {
    return null
  }
}

async function fileDigest(path: string): Promise<string | null> {
  try {
    const info = await stat(path)
    if (!info.isFile()) return null
    return digestRef("file.digest", await readFile(path, "utf8"))
  } catch {
    return null
  }
}

function githubFullName(remote: string | null): { provider: "github"; fullName: string } | null {
  if (!remote) return null
  const match = remote.match(/github\.com[:/]([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+?)(?:\.git)?$/)
  if (!match) return null
  return { provider: "github", fullName: match[1] ?? "unknown/unknown" }
}

async function findParentWithFile(start: string, filename: string, stopBefore: string): Promise<string | null> {
  let cursor = resolve(start)
  const stop = resolve(stopBefore)
  while (cursor !== stop && cursor.startsWith(stop)) {
    const candidate = join(cursor, filename)
    if (existsSync(candidate)) return candidate
    const parent = dirname(cursor)
    if (parent === cursor) break
    cursor = parent
  }
  return null
}

function relativeSafe(root: string, path: string): string {
  const rel = relative(root, path)
  return rel.length === 0 || rel.startsWith("..") ? "unknown" : rel
}

async function instructionRef(input: {
  path: string | null
  root: string
  sourceRef: string
  fallbackRelativePath: string
}): Promise<PylonDevDoctorInstructionRef> {
  if (!input.path) {
    return {
      sourceRef: input.sourceRef,
      state: "missing",
      relativePath: input.fallbackRelativePath,
      digestRef: null,
    }
  }
  return {
    sourceRef: input.sourceRef,
    state: "present",
    relativePath: relativeSafe(input.root, input.path),
    digestRef: await fileDigest(input.path),
  }
}

function packageRoot() {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..")
}

async function packageVersion(root: string): Promise<string> {
  try {
    const parsed = JSON.parse(await readFile(join(root, "package.json"), "utf8")) as { version?: unknown }
    return typeof parsed.version === "string" ? parsed.version : "unknown"
  } catch {
    return "unknown"
  }
}

function activeRepoCwd(env: Record<string, string | undefined>, fallback: string) {
  const configured = env.PYLON_CODEX_CWD ?? env.PYLON_ACTIVE_REPO
  return configured && configured.trim().length > 0 ? configured : fallback
}

export async function collectPylonDevDoctor(
  options: PylonDevDoctorOptions = {},
): Promise<PylonDevDoctorProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const observedAt = (options.now ?? new Date()).toISOString()
  const summary = options.summary ?? createBootstrapSummary(parseBootstrapArgs(["--json"]), env as NodeJS.ProcessEnv)
  const cwd = activeRepoCwd(env, options.cwd ?? process.cwd())
  const git = options.gitRunner ?? defaultGitRunner
  const packageRootDir = packageRoot()

  const repoRoot = await git(cwd, ["rev-parse", "--show-toplevel"])
  const insideWorkTree = await git(cwd, ["rev-parse", "--is-inside-work-tree"])
  const branch = repoRoot ? await git(repoRoot, ["branch", "--show-current"]) : null
  const commit = repoRoot ? await git(repoRoot, ["rev-parse", "HEAD"]) : null
  const remote = repoRoot ? await git(repoRoot, ["config", "--get", "remote.origin.url"]) : null
  const status = repoRoot ? await git(repoRoot, ["status", "--porcelain=v1"]) : null
  const dirtyCount = status === null || status.length === 0 ? 0 : status.split("\n").filter(Boolean).length
  const remoteGithub = githubFullName(remote)
  const repoBlockers = [
    ...(insideWorkTree === "true" && repoRoot ? [] : ["blocker.dev_doctor.repo_unknown"]),
    ...(commit ? [] : ["blocker.dev_doctor.commit_unknown"]),
    ...(branch ? [] : ["blocker.dev_doctor.branch_unknown_or_detached"]),
    ...(dirtyCount > 0 ? ["blocker.dev_doctor.repo_dirty"] : []),
    ...(remoteGithub ? [] : ["blocker.dev_doctor.remote_unknown"]),
  ]

  const workspaceRoot = repoRoot ? dirname(repoRoot) : resolve(cwd)
  const repoAgents = repoRoot ? join(repoRoot, "AGENTS.md") : null
  const repoInvariants = repoRoot ? join(repoRoot, "INVARIANTS.md") : null
  const workspaceAgents = repoRoot ? await findParentWithFile(dirname(repoRoot), "AGENTS.md", dirname(dirname(repoRoot))) : null
  const workspaceInvariants = repoRoot
    ? await findParentWithFile(dirname(repoRoot), "INVARIANTS.md", dirname(dirname(repoRoot)))
    : null
  const instructionRefs = await Promise.all([
    instructionRef({
      path: workspaceAgents,
      root: workspaceRoot,
      sourceRef: "instruction.workspace.agents",
      fallbackRelativePath: "AGENTS.md",
    }),
    instructionRef({
      path: repoAgents && existsSync(repoAgents) ? repoAgents : null,
      root: repoRoot ?? cwd,
      sourceRef: "instruction.repo.agents",
      fallbackRelativePath: "AGENTS.md",
    }),
    instructionRef({
      path: workspaceInvariants,
      root: workspaceRoot,
      sourceRef: "instruction.workspace.invariants",
      fallbackRelativePath: "INVARIANTS.md",
    }),
    instructionRef({
      path: repoInvariants && existsSync(repoInvariants) ? repoInvariants : null,
      root: repoRoot ?? cwd,
      sourceRef: "instruction.repo.invariants",
      fallbackRelativePath: "INVARIANTS.md",
    }),
  ])
  const instructionBlockers = instructionRefs
    .filter((ref) => ref.state === "missing")
    .map((ref) => `blocker.dev_doctor.${ref.sourceRef.split(".").slice(1).join("_")}_missing`)

  const configDigest = await fileDigest(summary.paths.config)
  const codexConfig = await loadCodexAgentConfig(summary)
  const codexDevConfig = await loadCodexDevConfig(summary)
  const claudeConfig = await loadClaudeAgentConfig(summary)
  const claudeDevConfig = await loadClaudeDevConfig(summary)
  const executionMode: CodexComposerExecutionMode =
    options.dangerFlag === true || codexDevConfig.codexExecutionMode === "local_supervised_danger"
      ? "local_supervised_danger"
      : "local_bounded"
  const codexSandboxMode = sandboxModeForCodexComposerExecutionMode(executionMode, codexConfig.sandboxMode)
  const claudeExecutionMode: ClaudeComposerExecutionMode =
    options.claudeDangerFlag === true ||
    claudeDevConfig.claudeExecutionMode === "local_supervised_danger"
      ? "local_supervised_danger"
      : "local_bounded"
  const claudePermissionMode = permissionModeForClaudeComposerExecutionMode(claudeExecutionMode)
  const codexReadiness = await probeCodexAgentReadiness({
    config: codexConfig,
    codexCliLoginPresent: options.codexCliLoginPresent,
    env,
    importer: options.codexImporter,
  })
  const claudeReadiness = await probeClaudeAgentReadiness({
    config: claudeConfig,
    env,
    importer: options.claudeImporter,
    localSessionProbe: options.localClaudeSessionProbe,
  })
  const codexCliPath = options.codexCliPath === undefined ? Bun.which("codex") : options.codexCliPath
  const inventory = options.inventory ?? await discoverHostInventory({ env, now: options.now })
  const backendRefs = inventory.backendHealth.map((backend) => ({
    backendRef: backend.backendRef,
    state: backend.state,
    modelRef: backend.modelRef,
    blockerRefs: backend.blockerRefs,
  }))
  const fableModel = typeof claudeConfig.model === "string" && claudeConfig.model.toLowerCase().includes("fable")
  const projection: PylonDevDoctorProjection = {
    schema: PYLON_DEV_DOCTOR_SCHEMA,
    observedAt,
    package: {
      name: "@openagentsinc/pylon",
      version: await packageVersion(packageRootDir),
      sourceCommit: await git(packageRootDir, ["rev-parse", "HEAD"]),
    },
    repo: {
      state: insideWorkTree === "true" && repoRoot ? "ready" : "not_git",
      provider: remoteGithub?.provider ?? (remote ? "unknown" : null),
      fullName: remoteGithub?.fullName ?? null,
      branch: branch || null,
      commit,
      dirty: {
        state: status === null ? "unknown" : dirtyCount > 0 ? "dirty" : "clean",
        changedCount: dirtyCount,
      },
      blockerRefs: repoBlockers,
    },
    instructions: {
      refs: instructionRefs,
      blockerRefs: instructionBlockers,
    },
    pylonConfig: {
      state: configDigest ? "present" : "missing",
      configRef: "config.pylon.local",
      digestRef: configDigest,
      devOverlayRef:
        codexDevConfig.codexExecutionMode === "local_supervised_danger"
          ? "config.pylon.dev.local_supervised_danger"
          : null,
      claudeDevOverlayRef:
        claudeDevConfig.claudeExecutionMode === "local_supervised_danger"
          ? "config.pylon.dev.claude_local_supervised_danger"
          : null,
      defaultAdapter: codexDevConfig.defaultAdapter ?? "codex",
    },
    codex: {
      cli: codexCliPath ? "present" : "missing",
      sdkReadiness: codexReadiness,
      credentialSourceRef: codexReadiness.credentialSourceRef,
      configuredModel: codexConfig.model ?? null,
      executionMode,
      sandboxMode: codexSandboxMode,
      blockerRefs: [
        ...(codexCliPath ? [] : ["blocker.dev_doctor.codex_cli_missing"]),
        ...codexReadiness.blockerRefs,
      ],
    },
    claudeAgent: {
      readiness: claudeReadiness,
      configuredModel: claudeConfig.model ?? null,
      fableReviewAvailable: claudeReadiness.state === "ready" && fableModel,
      executionMode: claudeExecutionMode,
      permissionMode: claudePermissionMode,
      dangerPublicPathBlockerRef:
        claudeExecutionMode === "local_supervised_danger"
          ? CLAUDE_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF
          : null,
      blockerRefs: claudeReadiness.blockerRefs,
    },
    backends: {
      refs: backendRefs,
      blockerRefs: inventory.blockerRefs,
    },
    usage: await collectPylonAccountUsageSummary(summary, { now: options.now }),
    blockerRefs: [
      ...repoBlockers,
      ...instructionBlockers,
      ...(configDigest ? [] : ["blocker.dev_doctor.pylon_config_missing"]),
      ...(codexCliPath ? [] : ["blocker.dev_doctor.codex_cli_missing"]),
      ...codexReadiness.blockerRefs,
      ...claudeReadiness.blockerRefs,
      ...inventory.blockerRefs,
    ],
  }
  assertPublicProjectionSafe(projection)
  return projection
}
