import { CODEX_AGENT_CAPABILITY_REF } from "./codex-agent.js"
import { CLAUDE_AGENT_CAPABILITY_REF } from "./claude-agent.js"
import type { PylonAccountUsageSummary } from "./account-usage.js"
import { collectPylonDevDoctor, type PylonDevDoctorOptions, type PylonDevDoctorProjection } from "./dev-doctor.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_CONTEXT_SCHEMA = "openagents.pylon.context.v0.3"

export type PylonAdapterRef = "codex" | "claude_agent" | "fable" | "unknown"

export type PylonContextProjection = {
  schema: typeof PYLON_CONTEXT_SCHEMA
  observedAt: string
  repo: {
    state: "ready" | "not_git" | "unknown"
    provider: "github" | "unknown" | null
    fullName: string | null
    branch: string | null
    commitRef: string | null
    dirtyState: "clean" | "dirty" | "unknown"
    changedCount: number
    blockerRefs: string[]
  }
  instructions: {
    refs: Array<{
      sourceRef: string
      state: "present" | "missing"
      relativePath: string
      digestRef: string | null
    }>
    configRefs: string[]
    blockerRefs: string[]
  }
  adapters: {
    mode: "normal" | "dev"
    primaryAdapter: PylonAdapterRef
    reviewerAdapter: PylonAdapterRef | null
    codex: {
      state: string
      enabled: boolean
      cli: "present" | "missing"
      credentialSourceRef: string | null
      modelRef: string | null
      executionMode: string
      sandboxMode: string
      danger: boolean
      capabilityRefs: string[]
      blockerRefs: string[]
    }
    openai: {
      state: "configured" | "unknown"
      sourceRefs: string[]
      blockerRefs: string[]
    }
    claudeAgent: {
      state: string
      enabled: boolean
      credentialSourceRef: string | null
      modelRef: string | null
      fableReviewAvailable: boolean
      executionMode: string
      permissionMode: string
      danger: boolean
      capabilityRefs: string[]
      blockerRefs: string[]
    }
    backends: PylonDevDoctorProjection["backends"]["refs"]
    blockerRefs: string[]
  }
  currentJob: {
    assignmentRef: string | null
    workRequestRef: string | null
    workOrderRef: string | null
    workspaceRef: string | null
    worktreeRef: string | null
    verificationCommandRef: string | null
    latestVerificationRef: string | null
    primaryAdapter: PylonAdapterRef
    reviewerAdapter: PylonAdapterRef | null
    requiredCapabilityRefs: string[]
    blockerRefs: string[]
  }
  usage?: PylonAccountUsageSummary | null
  blockerRefs: string[]
}

const contextForbiddenStringPattern =
  /([a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}|\/Users\/[^\s"']+|\/home\/[^\s"']+|\/var\/folders\/[^\s"']+|\.codex\/auth\.json|\.claude\/\.credentials\.json|bearer\s+[a-z0-9._-]+|sk-[a-z0-9_-]+)/i

function assertNoContextPrivateStrings(value: unknown, path = "context"): void {
  if (typeof value === "string") {
    if (contextForbiddenStringPattern.test(value)) {
      throw new Error(`${path} contains private context text`)
    }
    return
  }
  if (!value || typeof value !== "object") return
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    assertNoContextPrivateStrings(child, `${path}.${key}`)
  }
}

export function assertPylonContextProjectionSafe(value: unknown): asserts value is PylonContextProjection {
  assertPublicProjectionSafe(value)
  assertNoContextPrivateStrings(value)
}

function sanitizeRefSegment(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9._-]+/g, "_").replace(/^_+|_+$/g, "") || "unknown"
}

function modelRef(namespace: string, value: string | null): string | null {
  if (!value) return null
  return `model.${namespace}.${sanitizeRefSegment(value)}`
}

function commitRef(commit: string | null): string | null {
  if (!commit) return null
  return `commit.${commit.slice(0, 12)}`
}

function unique(values: ReadonlyArray<string | null | undefined>): string[] {
  return [...new Set(values.filter((value): value is string => typeof value === "string" && value.length > 0))]
}

export function emptyPylonContextProjection(observedAt = "1970-01-01T00:00:00.000Z"): PylonContextProjection {
  return {
    schema: PYLON_CONTEXT_SCHEMA,
    observedAt,
    repo: {
      state: "unknown",
      provider: null,
      fullName: null,
      branch: null,
      commitRef: null,
      dirtyState: "unknown",
      changedCount: 0,
      blockerRefs: ["blocker.context.repo_unknown"],
    },
    instructions: {
      refs: [],
      configRefs: [],
      blockerRefs: ["blocker.context.instructions_unknown"],
    },
    adapters: {
      mode: "normal",
      primaryAdapter: "unknown",
      reviewerAdapter: null,
      codex: {
        state: "unknown",
        enabled: false,
        cli: "missing",
        credentialSourceRef: null,
        modelRef: null,
        executionMode: "unknown",
        sandboxMode: "unknown",
        danger: false,
        capabilityRefs: [],
        blockerRefs: ["blocker.context.codex_unknown"],
      },
      openai: {
        state: "unknown",
        sourceRefs: [],
        blockerRefs: ["blocker.context.openai_unknown"],
      },
      claudeAgent: {
        state: "unknown",
        enabled: false,
        credentialSourceRef: null,
        modelRef: null,
        fableReviewAvailable: false,
        executionMode: "unknown",
        permissionMode: "unknown",
        danger: false,
        capabilityRefs: [],
        blockerRefs: ["blocker.context.claude_agent_unknown"],
      },
      backends: [],
      blockerRefs: ["blocker.context.adapters_unknown"],
    },
    currentJob: {
      assignmentRef: null,
      workRequestRef: null,
      workOrderRef: null,
      workspaceRef: null,
      worktreeRef: null,
      verificationCommandRef: null,
      latestVerificationRef: null,
      primaryAdapter: "unknown",
      reviewerAdapter: null,
      requiredCapabilityRefs: [],
      blockerRefs: ["blocker.context.current_job_unknown"],
    },
    blockerRefs: [
      "blocker.context.repo_unknown",
      "blocker.context.instructions_unknown",
      "blocker.context.adapters_unknown",
      "blocker.context.current_job_unknown",
    ],
  }
}

export function contextProjectionFromDevDoctor(dev: PylonDevDoctorProjection): PylonContextProjection {
  const codexReady = dev.codex.sdkReadiness.state === "ready"
  const claudeReady = dev.claudeAgent.readiness.state === "ready"
  const primaryAdapter: PylonAdapterRef =
    dev.pylonConfig.defaultAdapter === "claude_agent"
      ? claudeReady ? "claude_agent" : "unknown"
      : codexReady ? "codex" : "unknown"
  const reviewerAdapter: PylonAdapterRef | null =
    primaryAdapter === "codex"
      ? dev.claudeAgent.fableReviewAvailable
        ? "fable"
        : claudeReady
          ? "claude_agent"
          : null
      : primaryAdapter === "claude_agent" && codexReady
        ? "codex"
        : null
  const codexCapabilityRefs = dev.codex.sdkReadiness.capabilityRefs
  const claudeCapabilityRefs = dev.claudeAgent.readiness.capabilityRefs
  const requiredCapabilityRefs = unique([
    primaryAdapter === "codex" ? CODEX_AGENT_CAPABILITY_REF : null,
    reviewerAdapter === "codex" ? CODEX_AGENT_CAPABILITY_REF : null,
    primaryAdapter === "claude_agent" || reviewerAdapter === "fable" || reviewerAdapter === "claude_agent"
      ? CLAUDE_AGENT_CAPABILITY_REF
      : null,
  ])
  const openaiSourceRefs = unique(
    dev.codex.credentialSourceRef &&
      (dev.codex.credentialSourceRef.includes("openai") || dev.codex.credentialSourceRef.includes("codex"))
      ? [dev.codex.credentialSourceRef]
      : [],
  )
  const context: PylonContextProjection = {
    schema: PYLON_CONTEXT_SCHEMA,
    observedAt: dev.observedAt,
    repo: {
      state: dev.repo.state,
      provider: dev.repo.provider,
      fullName: dev.repo.fullName,
      branch: dev.repo.branch,
      commitRef: commitRef(dev.repo.commit),
      dirtyState: dev.repo.dirty.state,
      changedCount: dev.repo.dirty.changedCount,
      blockerRefs: dev.repo.blockerRefs,
    },
    instructions: {
      refs: dev.instructions.refs,
      configRefs: unique([
        dev.pylonConfig.configRef,
        dev.pylonConfig.devOverlayRef,
        dev.pylonConfig.claudeDevOverlayRef,
        `config.pylon.dev.default_adapter.${dev.pylonConfig.defaultAdapter}`,
      ]),
      blockerRefs: [
        ...dev.instructions.blockerRefs,
        ...(dev.pylonConfig.state === "present" ? [] : ["blocker.context.pylon_config_missing"]),
      ],
    },
    adapters: {
      mode:
        dev.pylonConfig.devOverlayRef ||
        dev.pylonConfig.claudeDevOverlayRef ||
        dev.codex.executionMode !== "local_bounded" ||
        dev.claudeAgent.executionMode !== "local_bounded"
          ? "dev"
          : "normal",
      primaryAdapter,
      reviewerAdapter,
      codex: {
        state: dev.codex.sdkReadiness.state,
        enabled: dev.codex.sdkReadiness.enabled,
        cli: dev.codex.cli,
        credentialSourceRef: dev.codex.credentialSourceRef,
        modelRef: modelRef("codex", dev.codex.configuredModel),
        executionMode: dev.codex.executionMode,
        sandboxMode: dev.codex.sandboxMode,
        danger: dev.codex.sandboxMode === "danger-full-access" || dev.codex.executionMode === "local_supervised_danger",
        capabilityRefs: codexCapabilityRefs,
        blockerRefs: dev.codex.blockerRefs,
      },
      openai: {
        state: openaiSourceRefs.length > 0 ? "configured" : "unknown",
        sourceRefs: openaiSourceRefs,
        blockerRefs: openaiSourceRefs.length > 0 ? [] : ["blocker.context.openai_account_unknown"],
      },
      claudeAgent: {
        state: dev.claudeAgent.readiness.state,
        enabled: dev.claudeAgent.readiness.enabled,
        credentialSourceRef: dev.claudeAgent.readiness.credentialSourceRef,
        modelRef: modelRef("claude_agent", dev.claudeAgent.configuredModel),
        fableReviewAvailable: dev.claudeAgent.fableReviewAvailable,
        executionMode: dev.claudeAgent.executionMode,
        permissionMode: dev.claudeAgent.permissionMode,
        danger: dev.claudeAgent.executionMode === "local_supervised_danger",
        capabilityRefs: claudeCapabilityRefs,
        blockerRefs: dev.claudeAgent.blockerRefs,
      },
      backends: dev.backends.refs,
      blockerRefs: [...dev.codex.blockerRefs, ...dev.claudeAgent.blockerRefs, ...dev.backends.blockerRefs],
    },
    currentJob: {
      assignmentRef: null,
      workRequestRef: null,
      workOrderRef: null,
      workspaceRef: null,
      worktreeRef: null,
      verificationCommandRef: null,
      latestVerificationRef: null,
      primaryAdapter,
      reviewerAdapter,
      requiredCapabilityRefs,
      blockerRefs: primaryAdapter === "unknown" ? ["blocker.context.primary_adapter_unavailable"] : [],
    },
    usage: dev.usage ?? null,
    blockerRefs: unique([
      ...dev.blockerRefs,
      primaryAdapter === "unknown" ? "blocker.context.primary_adapter_unavailable" : null,
    ]),
  }
  assertPylonContextProjectionSafe(context)
  return context
}

export async function collectPylonContextProjection(
  options: PylonDevDoctorOptions = {},
): Promise<PylonContextProjection> {
  return contextProjectionFromDevDoctor(await collectPylonDevDoctor(options))
}
