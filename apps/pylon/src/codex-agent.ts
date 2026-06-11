import { readFile, stat } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

/**
 * The local Codex bridge probe (issue #4788, epic #4793, promise
 * autopilot.codex_probe_pylon_successor.v1).
 *
 * The Codex SDK is a lazy optional dependency: this module never imports it
 * at the top level, so every Pylon command keeps working when the SDK (or
 * its bundled platform binary) is absent. The probe reports presence only —
 * it never reads, logs, or persists credential values, and the capability
 * ref is the only public signal that a local Codex exists.
 *
 * Credential policy (the CX1 ToS review, recorded in
 * docs/codex-bridge.md): the lane honors exactly the owner's own
 * CODEX_API_KEY or OPENAI_API_KEY, or the owner's own existing Codex CLI
 * login (`codex login`) on this device. Platform-supplied, shared, leased,
 * or brokered credentials are never honored — the no-resale law binds.
 */

export const CODEX_AGENT_CAPABILITY_REF = "capability.pylon.local_codex"
export const CODEX_AGENT_SDK_PACKAGE = "@openai/codex-sdk"
export const CODEX_AGENT_READINESS_SCHEMA = "openagents.pylon.codex_agent_readiness.v0.3"

export type CodexAgentReadinessState =
  | "ready"
  | "sdk_missing"
  | "credentials_missing"
  | "platform_unsupported"
  | "disabled_by_config"

export type CodexAgentReadiness = {
  schema: typeof CODEX_AGENT_READINESS_SCHEMA
  state: CodexAgentReadinessState
  enabled: boolean
  capabilityRefs: string[]
  blockerRefs: string[]
  credentialSourceRef: string | null
}

export type CodexAgentSandboxMode = "read-only" | "workspace-write"

export type CodexAgentConfig = {
  enabled?: boolean
  model?: string
  maxTurns?: number
  timeoutSeconds?: number
  sandboxMode?: CodexAgentSandboxMode
}

export type CodexAgentProbeOptions = {
  env?: Record<string, string | undefined>
  platform?: string
  importer?: (specifier: string) => Promise<unknown>
  config?: CodexAgentConfig
  /** Test injection for the owner's `codex login` state; presence only. */
  codexCliLoginPresent?: boolean
}

const SUPPORTED_PLATFORMS = new Set(["darwin", "linux"])

/**
 * Detects whether the owner has an existing Codex CLI login on this device,
 * by presence of a non-empty auth file only. The file is never read; its
 * contents never enter probe output.
 */
export async function detectCodexCliLogin(
  env: Record<string, string | undefined>,
): Promise<boolean> {
  const codexHome = (env.CODEX_HOME ?? "").trim().length > 0
    ? (env.CODEX_HOME as string)
    : join(homedir(), ".codex")
  try {
    const info = await stat(join(codexHome, "auth.json"))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

/**
 * Reports which BYOK credential source is configured, by presence only.
 * Returns a public-safe source ref, never the credential value. Order:
 * explicit Codex key, owner's OpenAI key, owner's own Codex CLI login.
 */
export function codexAgentCredentialSource(
  env: Record<string, string | undefined>,
  codexCliLoginPresent: boolean,
): string | null {
  if ((env.CODEX_API_KEY ?? "").trim().length > 0) {
    return "credential.source.codex_agent.codex_api_key"
  }
  if ((env.OPENAI_API_KEY ?? "").trim().length > 0) {
    return "credential.source.codex_agent.openai_api_key"
  }
  if (codexCliLoginPresent) {
    return "credential.source.codex_agent.codex_cli_login"
  }
  return null
}

function readiness(
  state: CodexAgentReadinessState,
  input: { enabled: boolean; credentialSourceRef: string | null; blockerRef?: string },
): CodexAgentReadiness {
  return {
    schema: CODEX_AGENT_READINESS_SCHEMA,
    state,
    enabled: input.enabled,
    capabilityRefs: state === "ready" ? [CODEX_AGENT_CAPABILITY_REF] : [],
    blockerRefs: input.blockerRef === undefined ? [] : [input.blockerRef],
    credentialSourceRef: input.credentialSourceRef,
  }
}

/**
 * Probes whether this device can run the local Codex lane. Order: config
 * gate, platform support, lazy SDK import, BYOK credential presence. The
 * default importer resolves the real optional dependency; tests inject
 * their own to simulate every state without the SDK or a key.
 */
export async function probeCodexAgentReadiness(
  options: CodexAgentProbeOptions = {},
): Promise<CodexAgentReadiness> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const platform = options.platform ?? process.platform
  const enabled = options.config?.enabled !== false
  const codexCliLoginPresent = options.codexCliLoginPresent ?? (await detectCodexCliLogin(env))
  const credentialSourceRef = codexAgentCredentialSource(env, codexCliLoginPresent)

  if (!enabled) {
    return readiness("disabled_by_config", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.codex_agent.disabled_by_config",
    })
  }
  if (!SUPPORTED_PLATFORMS.has(platform)) {
    return readiness("platform_unsupported", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.codex_agent.platform_unsupported",
    })
  }

  const importer = options.importer ?? ((specifier: string) => import(specifier))
  try {
    await importer(CODEX_AGENT_SDK_PACKAGE)
  } catch {
    return readiness("sdk_missing", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.codex_agent.sdk_missing",
    })
  }

  if (credentialSourceRef === null) {
    return readiness("credentials_missing", {
      enabled,
      credentialSourceRef,
      blockerRef: "blocker.codex_agent.credentials_missing",
    })
  }

  return readiness("ready", { enabled, credentialSourceRef })
}

/**
 * Applies probe truth to a capability list: declares the local-Codex
 * capability when ready, and strips a stale declaration when not — a Pylon
 * whose SDK or credentials disappeared must stop advertising the lane.
 */
export function withCodexAgentCapability(
  capabilityRefs: ReadonlyArray<string>,
  probed: CodexAgentReadiness,
): string[] {
  const base = capabilityRefs.filter((ref) => ref !== CODEX_AGENT_CAPABILITY_REF)
  return probed.state === "ready"
    ? [...new Set([...base, CODEX_AGENT_CAPABILITY_REF])]
    : [...new Set(base)]
}

/**
 * Reads the codex section of the persisted Pylon config file. Best-effort:
 * a missing or malformed file means no overrides. sandboxMode accepts only
 * the two bounded modes — danger-full-access is never configurable.
 */
export async function loadCodexAgentConfig(
  summary: { paths: { config: string } },
): Promise<CodexAgentConfig> {
  try {
    const raw = JSON.parse(
      await readFile(summary.paths.config, "utf8"),
    ) as { codex?: unknown }
    const section = raw.codex
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
      ...(config.sandboxMode === "read-only" || config.sandboxMode === "workspace-write"
        ? { sandboxMode: config.sandboxMode }
        : {}),
    }
  } catch {
    return {}
  }
}
