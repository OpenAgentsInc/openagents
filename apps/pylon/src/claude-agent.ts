import { readFile } from "node:fs/promises"
import type { BootstrapSummary } from "./bootstrap"

/**
 * The local Claude Agent bridge probe (issue #4718, promise
 * pylon.local_claude_agent_bridge.v1).
 *
 * The Claude Agent SDK is a lazy optional dependency: this module never
 * imports it at the top level, so every Pylon command keeps working when the
 * SDK (or its platform binary) is absent. The probe reports presence only —
 * it never reads, logs, or persists credential values, and the capability
 * ref is the only public signal that a local Claude exists.
 */

export const CLAUDE_AGENT_CAPABILITY_REF = "capability.pylon.local_claude_agent"
export const CLAUDE_AGENT_SDK_PACKAGE = "@anthropic-ai/claude-agent-sdk"
export const CLAUDE_AGENT_READINESS_SCHEMA = "openagents.pylon.claude_agent_readiness.v0.3"

export type ClaudeAgentReadinessState =
  | "ready"
  | "sdk_missing"
  | "credentials_missing"
  | "platform_unsupported"
  | "disabled_by_config"

export type ClaudeAgentReadiness = {
  schema: typeof CLAUDE_AGENT_READINESS_SCHEMA
  state: ClaudeAgentReadinessState
  enabled: boolean
  capabilityRefs: string[]
  blockerRefs: string[]
  credentialSourceRef: string | null
}

export type ClaudeAgentConfig = {
  enabled?: boolean
  model?: string
  maxTurns?: number
  timeoutSeconds?: number
}

export type ClaudeAgentProbeOptions = {
  env?: Record<string, string | undefined>
  platform?: string
  importer?: (specifier: string) => Promise<unknown>
  config?: ClaudeAgentConfig
}

const SUPPORTED_PLATFORMS = new Set(["darwin", "linux"])

function flagEnabled(value: string | undefined): boolean {
  return value === "1" || value === "true"
}

/**
 * Reports which BYOK credential source is configured, by presence only.
 * Returns a public-safe source ref, never the credential value.
 */
export function claudeAgentCredentialSource(
  env: Record<string, string | undefined>,
): string | null {
  if ((env.ANTHROPIC_API_KEY ?? "").trim().length > 0) {
    return "credential.source.claude_agent.anthropic_api_key"
  }
  if (flagEnabled(env.CLAUDE_CODE_USE_BEDROCK)) {
    return "credential.source.claude_agent.amazon_bedrock"
  }
  if (flagEnabled(env.CLAUDE_CODE_USE_VERTEX)) {
    return "credential.source.claude_agent.google_vertex"
  }
  if (flagEnabled(env.CLAUDE_CODE_USE_FOUNDRY)) {
    return "credential.source.claude_agent.azure_foundry"
  }
  if (
    flagEnabled(env.CLAUDE_CODE_USE_ANTHROPIC_AWS) &&
    (env.ANTHROPIC_AWS_WORKSPACE_ID ?? "").trim().length > 0
  ) {
    return "credential.source.claude_agent.anthropic_aws"
  }
  return null
}

function readiness(
  state: ClaudeAgentReadinessState,
  input: { enabled: boolean; credentialSourceRef: string | null; blockerRef?: string },
): ClaudeAgentReadiness {
  return {
    schema: CLAUDE_AGENT_READINESS_SCHEMA,
    state,
    enabled: input.enabled,
    capabilityRefs: state === "ready" ? [CLAUDE_AGENT_CAPABILITY_REF] : [],
    blockerRefs: input.blockerRef === undefined ? [] : [input.blockerRef],
    credentialSourceRef: input.credentialSourceRef,
  }
}

/**
 * Probes whether this device can run the local Claude Agent lane. Order:
 * config gate, platform support, lazy SDK import, BYOK credential presence.
 * The default importer resolves the real optional dependency; tests inject
 * their own to simulate every state without the SDK or a key.
 */
export async function probeClaudeAgentReadiness(
  options: ClaudeAgentProbeOptions = {},
): Promise<ClaudeAgentReadiness> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const platform = options.platform ?? process.platform
  const enabled = options.config?.enabled !== false
  const credentialSourceRef = claudeAgentCredentialSource(env)

  if (!enabled) {
    return readiness("disabled_by_config", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.claude_agent.disabled_by_config",
    })
  }
  if (!SUPPORTED_PLATFORMS.has(platform)) {
    return readiness("platform_unsupported", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.claude_agent.platform_unsupported",
    })
  }

  const importer = options.importer ?? ((specifier: string) => import(specifier))
  try {
    await importer(CLAUDE_AGENT_SDK_PACKAGE)
  } catch {
    return readiness("sdk_missing", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.claude_agent.sdk_missing",
    })
  }

  if (credentialSourceRef === null) {
    return readiness("credentials_missing", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.claude_agent.credentials_missing",
    })
  }

  return readiness("ready", { enabled, credentialSourceRef })
}

/**
 * Applies probe truth to a capability list: declares the local-Claude
 * capability when ready, and strips a stale declaration when not — a Pylon
 * whose SDK or key disappeared must stop advertising the lane.
 */
export function withClaudeAgentCapability(
  capabilityRefs: ReadonlyArray<string>,
  probed: ClaudeAgentReadiness,
): string[] {
  const base = capabilityRefs.filter((ref) => ref !== CLAUDE_AGENT_CAPABILITY_REF)
  return probed.state === "ready"
    ? [...new Set([...base, CLAUDE_AGENT_CAPABILITY_REF])]
    : [...new Set(base)]
}

/**
 * Reads the claudeAgent section of the persisted Pylon config file.
 * Best-effort: a missing or malformed file means no overrides.
 */
export async function loadClaudeAgentConfig(
  summary: BootstrapSummary,
): Promise<ClaudeAgentConfig> {
  try {
    const raw = JSON.parse(
      await readFile(summary.paths.config, "utf8"),
    ) as { claudeAgent?: unknown }
    const section = raw.claudeAgent
    if (section === null || typeof section !== "object") return {}
    const config = section as Record<string, unknown>
    return {
      ...(typeof config.enabled === "boolean" ? { enabled: config.enabled } : {}),
      ...(typeof config.model === "string" && config.model.length > 0
        ? { model: config.model }
        : {}),
      ...(typeof config.maxTurns === "number" && Number.isFinite(config.maxTurns)
        ? { maxTurns: config.maxTurns }
        : {}),
      ...(typeof config.timeoutSeconds === "number" && Number.isFinite(config.timeoutSeconds)
        ? { timeoutSeconds: config.timeoutSeconds }
        : {}),
    }
  } catch {
    return {}
  }
}
