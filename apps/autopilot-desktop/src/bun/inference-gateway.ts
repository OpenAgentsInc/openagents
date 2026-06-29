// Bun-host OpenAgents inference-gateway readiness (#5485, EPIC #5474).
//
// Reads the gateway settings (server flag, base URL, API-key presence, model,
// low-balance threshold) and — when the gateway is enabled AND a Bun-host API
// key is configured — fetches the user's pay-as-you-go credit balance from the
// openagents.com `/v1/credits` endpoint. Projects a PUBLIC-SAFE readiness blob
// for the webview: server-flag state, `apiKeyPresent` boolean, model id, numeric
// balance, threshold, and blocker refs. The raw API key NEVER crosses the RPC
// boundary; it is only ever placed in the outbound Authorization header here.
//
// INERT-SAFE: with the flag off (the default) this returns `enabled: false`,
// does NO network call, and surfaces a `gateway_disabled` blocker — the routing
// decision then keeps BYO-auth behaviour. Lighting up the server flag is the
// only change needed to activate the credit-balance read + the fallback path.

import {
  inferenceGatewayCreditsUrl,
  resolveInferenceGatewaySettings,
  type InferenceGatewaySettings,
} from "../shared/inference-gateway.js"
import type { InferenceGatewayReadinessResponse } from "../shared/rpc.js"

type BuildInput = Readonly<{
  env: Readonly<Record<string, string | undefined>>
  // The OpenAgents API key (kept in the Bun host). Injected separately so the
  // pure builder can be tested without reading process env, and so the key is
  // never threaded through the public settings object the webview could see.
  apiKey: string | null
  fetchFn?: typeof fetch
  nowIso?: () => string
}>

// Parse a credit balance out of the gateway's `/v1/credits` response. Tolerant
// of a couple of shapes (`{ creditBalance }` or `{ balance }`); returns null on
// anything unexpected so an odd payload degrades to "unknown" rather than 0.
const parseCreditBalance = (body: unknown): number | null => {
  if (typeof body !== "object" || body === null) return null
  const record = body as Record<string, unknown>
  const candidate =
    typeof record.creditBalance === "number"
      ? record.creditBalance
      : typeof record.balance === "number"
        ? record.balance
        : null
  return candidate !== null && Number.isFinite(candidate) ? candidate : null
}

export const buildInferenceGatewayReadiness = async (
  input: BuildInput,
): Promise<InferenceGatewayReadinessResponse> => {
  const settings: InferenceGatewaySettings = resolveInferenceGatewaySettings(
    input.env,
  )
  const fetchFn = input.fetchFn ?? fetch
  const nowIso = input.nowIso ?? (() => new Date().toISOString())
  const sourceUrl = inferenceGatewayCreditsUrl(settings.baseUrl)
  const apiKeyPresent = input.apiKey !== null && input.apiKey.trim().length > 0

  const base = {
    fetchedAt: nowIso(),
    sourceUrl,
    enabled: settings.enabled,
    apiKeyPresent,
    model: settings.model,
    lowBalanceThreshold: settings.lowBalanceThreshold,
  } as const

  const blockerRefs: string[] = []
  if (!settings.enabled) {
    blockerRefs.push("blocker.inference.gateway_disabled")
  }
  if (!apiKeyPresent) {
    blockerRefs.push("blocker.inference.gateway_api_key_missing")
  }

  // INERT path: do not call the network until the gateway is actually served
  // AND we have a key to authorize the credit read.
  if (!settings.enabled || !apiKeyPresent) {
    return { ...base, ok: true, creditBalance: null, blockerRefs }
  }

  try {
    const res = await fetchFn(sourceUrl, {
      method: "GET",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${input.apiKey}`,
      },
    })
    if (!res.ok) {
      return {
        ...base,
        ok: false,
        creditBalance: null,
        blockerRefs: [...blockerRefs, "blocker.inference.gateway_credits_unavailable"],
        error: `credits ${res.status}`,
      }
    }
    const body = (await res.json()) as unknown
    const creditBalance = parseCreditBalance(body)
    const balanceBlockers =
      creditBalance !== null && creditBalance <= 0
        ? [...blockerRefs, "blocker.inference.gateway_out_of_credits"]
        : blockerRefs
    return { ...base, ok: true, creditBalance, blockerRefs: balanceBlockers }
  } catch (error) {
    return {
      ...base,
      ok: false,
      creditBalance: null,
      blockerRefs: [...blockerRefs, "blocker.inference.gateway_credits_unavailable"],
      error: error instanceof Error ? error.message : "credits fetch failed",
    }
  }
}
