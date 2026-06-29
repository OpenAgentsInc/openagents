import { access, readFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

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

export type ClaudeLocalExecutionMode = "local_supervised_danger"

export type ClaudeDevConfig = {
  claudeExecutionMode?: ClaudeLocalExecutionMode
}

export type ClaudeAgentProbeOptions = {
  env?: Record<string, string | undefined>
  platform?: string
  importer?: (specifier: string) => Promise<unknown>
  config?: ClaudeAgentConfig
  /**
   * Presence-only detector for a local Claude Code session (the user's own
   * logged-in Claude on this machine). Injected by tests; defaults to the
   * real filesystem/keychain presence check. Never reads credential values.
   */
  localSessionProbe?: () => Promise<boolean>
}

export const CLAUDE_AGENT_LOCAL_SESSION_SOURCE_REF =
  "credential.source.claude_agent.local_claude_session"

/**
 * Detects whether this machine has a logged-in local Claude Code session —
 * the user's own subscription credentials, which the bundled SDK binary
 * reuses when no API key is exported. Presence only: the file check tests
 * existence without reading contents, and the macOS keychain check discards
 * all output. BYOK still holds — the session is the user's own credential.
 */
export async function localClaudeSessionPresent(
  platform: string = process.platform,
  env: Record<string, string | undefined> = Bun.env as Record<string, string | undefined>,
): Promise<boolean> {
  // A per-account OAuth token (CLAUDE_CODE_OAUTH_TOKEN) outranks the macOS
  // Keychain credential and is how Pylon pools multiple Claude accounts. When
  // it is present, the session is authenticated regardless of any config-dir
  // credential file.
  if ((env.CLAUDE_CODE_OAUTH_TOKEN ?? "").trim().length > 0) return true
  const configured = (env.CLAUDE_CONFIG_DIR ?? "").trim()
  if (configured.length > 0) {
    try {
      await access(join(configured, ".credentials.json"))
      return true
    } catch {
      return false
    }
  }
  const home = (env.HOME ?? "").trim().length > 0 ? (env.HOME as string) : homedir()
  try {
    await access(join(home, ".claude", ".credentials.json"))
    return true
  } catch {
    // fall through to the platform keychain check
  }
  if (platform === "darwin") {
    try {
      const proc = Bun.spawn(
        ["security", "find-generic-password", "-s", "Claude Code-credentials"],
        { stdout: "ignore", stderr: "ignore" },
      )
      return (await proc.exited) === 0
    } catch {
      return false
    }
  }
  return false
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

  let resolvedSourceRef = credentialSourceRef
  if (resolvedSourceRef === null) {
    const detectLocalSession =
      options.localSessionProbe ?? (() => localClaudeSessionPresent(platform, env))
    if (await detectLocalSession()) {
      resolvedSourceRef = CLAUDE_AGENT_LOCAL_SESSION_SOURCE_REF
    }
  }

  if (resolvedSourceRef === null) {
    return readiness("credentials_missing", {
      enabled,
      credentialSourceRef: resolvedSourceRef,
      blockerRef: "blocker.claude_agent.credentials_missing",
    })
  }

  return readiness("ready", { enabled, credentialSourceRef: resolvedSourceRef })
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
 * Best-effort: a missing or malformed file means no overrides. This is the
 * assignment-safe surface: permission/execution-mode keys are deliberately
 * not read here, so a permissive mode can never reach the delegated
 * assignment executor through config — only loadClaudeDevConfig() reads the
 * local-only dev overlay (issue #4845).
 */
export async function loadClaudeAgentConfig(
  summary: { paths: { config: string } },
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

/**
 * Reads the local-only dev section's Claude execution mode. Intentionally
 * separate from the assignment-safe claudeAgent config: local_supervised_danger
 * may affect only the direct composer/dev path, never public assignment
 * placement (issue #4845, mirroring loadCodexDevConfig).
 */
export async function loadClaudeDevConfig(
  summary: { paths: { config: string } },
): Promise<ClaudeDevConfig> {
  try {
    const raw = JSON.parse(
      await readFile(summary.paths.config, "utf8"),
    ) as { dev?: unknown }
    const section = raw.dev
    if (section === null || typeof section !== "object") return {}
    const config = section as Record<string, unknown>
    return {
      ...(config.claudeExecutionMode === "local_supervised_danger"
        ? { claudeExecutionMode: config.claudeExecutionMode }
        : {}),
    }
  } catch {
    return {}
  }
}
