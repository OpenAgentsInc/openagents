// OpenAgents inference-gateway default-inference settings (#5485, EPIC #5474).
//
// The gateway is the OpenAI-compatible `/v1/chat/completions` surface on
// openagents.com, billed against the user's OpenAgents API key + pay-as-you-go
// credit balance. This module is the single PURE resolver for whether the
// desktop should offer/use that path as the coding-session inference fallback —
// mirroring `builtin-agent.ts`: an env → settings function plus a public-safe
// readiness projection, both free of DOM/electrobun so they unit-test cleanly.
//
// INERT-SAFE: the gateway is flag-gated server-side. The client path + the
// "route through gateway when I have no own auth" setting are wired now; they
// only take real effect once `OPENAGENTS_INFERENCE_GATEWAY_ENABLED=1` is set
// (the server actually serves the gateway). With the flag off, the resolver
// reports `enabled: false` and the routing decision keeps BYO-auth behaviour.
//
// SECRET BOUNDARY: the OpenAgents API key lives ONLY in the Bun host env
// (`OPENAGENTS_INFERENCE_API_KEY`). It is NEVER returned to the webview — the
// readiness projection carries an `apiKeyPresent` boolean and a numeric credit
// balance only. Never hardcode a key.

export type InferenceGatewaySettings = {
  // Whether the gateway is enabled server-side (the flag gate). When false the
  // desktop must not route inference through the gateway, regardless of the
  // user's preference, and the routing decision falls back to BYO-auth.
  readonly enabled: boolean
  // The OpenAI-compatible base URL (defaults to the openagents.com gateway).
  readonly baseUrl: string
  // Whether a Bun-host OpenAgents API key is present. The raw key never leaves
  // the host; this boolean is the only thing the webview ever sees about it.
  readonly apiKeyPresent: boolean
  // The chat-completions model id the gateway routes coding turns to.
  readonly model: string
  // The credit balance at/below which the UI shows a low-balance hint (USD-ish
  // credit units; the gateway authority owns the exact unit + ledger).
  readonly lowBalanceThreshold: number
}

const DEFAULT_BASE_URL = "https://openagents.com"
const DEFAULT_MODEL = "openagents-gateway-default"
const DEFAULT_LOW_BALANCE_THRESHOLD = 1

const envString = (
  env: Readonly<Record<string, string | undefined>>,
  key: string,
): string | null => {
  const value = env[key]?.trim()
  return value && value.length > 0 ? value : null
}

const boundedNumber = (
  value: string | null,
  fallback: number,
  min: number,
  max: number,
): number => {
  if (value === null) return fallback
  const parsed = Number.parseFloat(value)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, parsed))
}

export const resolveInferenceGatewaySettings = (
  env: Readonly<Record<string, string | undefined>>,
): InferenceGatewaySettings => ({
  // Default OFF so an un-flagged install never silently changes routing. The
  // server lights the gateway up by setting this to "1".
  enabled: envString(env, "OPENAGENTS_INFERENCE_GATEWAY_ENABLED") === "1",
  baseUrl:
    envString(env, "OPENAGENTS_INFERENCE_GATEWAY_BASE_URL") ??
    envString(env, "OPENAGENTS_COM_BASE_URL") ??
    DEFAULT_BASE_URL,
  apiKeyPresent: envString(env, "OPENAGENTS_INFERENCE_API_KEY") !== null,
  model: envString(env, "OPENAGENTS_INFERENCE_GATEWAY_MODEL") ?? DEFAULT_MODEL,
  lowBalanceThreshold: boundedNumber(
    envString(env, "OPENAGENTS_INFERENCE_GATEWAY_LOW_BALANCE"),
    DEFAULT_LOW_BALANCE_THRESHOLD,
    0,
    1_000_000,
  ),
})

// The OpenAI-compatible chat-completions endpoint for the gateway base URL.
// Single source so the Bun fetch and any future caller agree on the path.
export const inferenceGatewayChatCompletionsUrl = (baseUrl: string): string =>
  `${baseUrl.replace(/\/+$/, "")}/v1/chat/completions`

// The credit-balance read endpoint. The Bun host calls this with the API key
// (in the Authorization header) and projects only the numeric balance forward.
export const inferenceGatewayCreditsUrl = (baseUrl: string): string =>
  `${baseUrl.replace(/\/+$/, "")}/v1/credits`
