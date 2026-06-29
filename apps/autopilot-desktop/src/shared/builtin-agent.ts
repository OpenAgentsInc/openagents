export type BuiltInAgentLane = "cloud-gcp" | "cloud-shc"

export type BuiltInAgentSettings = {
  readonly enabled: boolean
  readonly hostedComputeConfigured: boolean
  readonly lane: BuiltInAgentLane
  readonly modelSet: string
  readonly maxSessionSeconds: number
  readonly dailySessionCap: number
  readonly meteringLabel: string
}

const DEFAULT_MODEL_SET = "openagents-hosted-gemini"
const DEFAULT_MAX_SESSION_SECONDS = 600
const DEFAULT_DAILY_SESSION_CAP = 3

const envString = (
  env: Readonly<Record<string, string | undefined>>,
  key: string,
): string | null => {
  const value = env[key]?.trim()
  return value && value.length > 0 ? value : null
}

const boundedInteger = (
  value: string | null,
  fallback: number,
  min: number,
  max: number,
): number => {
  if (value === null) return fallback
  const parsed = Number.parseInt(value, 10)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, parsed))
}

export const resolveBuiltInAgentSettings = (
  env: Readonly<Record<string, string | undefined>>,
): BuiltInAgentSettings => {
  const lane =
    envString(env, "OPENAGENTS_BUILTIN_AGENT_LANE") === "cloud-shc"
      ? "cloud-shc"
      : "cloud-gcp"
  const modelSet =
    envString(env, "OPENAGENTS_BUILTIN_AGENT_MODEL_SET") ?? DEFAULT_MODEL_SET
  const maxSessionSeconds = boundedInteger(
    envString(env, "OPENAGENTS_BUILTIN_AGENT_MAX_SESSION_SECONDS"),
    DEFAULT_MAX_SESSION_SECONDS,
    60,
    1200,
  )
  const dailySessionCap = boundedInteger(
    envString(env, "OPENAGENTS_BUILTIN_AGENT_DAILY_SESSION_CAP"),
    DEFAULT_DAILY_SESSION_CAP,
    1,
    20,
  )

  return {
    enabled: envString(env, "OPENAGENTS_BUILTIN_AGENT_ENABLED") !== "0",
    hostedComputeConfigured:
      envString(env, "OA_CLOUD_CONTROL_URL") !== null &&
      envString(env, "OA_CLOUD_CONTROL_TOKEN") !== null,
    lane,
    modelSet,
    maxSessionSeconds,
    dailySessionCap,
    meteringLabel: `${dailySessionCap} sessions/day · ${maxSessionSeconds}s/session · ${modelSet}`,
  }
}

export const builtInAgentObjective = (settings: BuiltInAgentSettings): string =>
  [
    "You are the built-in OpenAgents first-run agent running on OpenAgents-provided compute.",
    "Do one bounded usability pass for the local user: read https://openagents.com/AGENTS.md and https://openagents.com/api/public/product-promises, then report one actionable product gap or report that the public surfaces are reachable.",
    "Do not modify code, do not spend funds, do not ask the user for a provider API key, and do not claim owner authority.",
    `Stay within ${settings.maxSessionSeconds} seconds on ${settings.modelSet}; summarize evidence, blocker refs if any, and the next useful action.`,
  ].join("\n")
