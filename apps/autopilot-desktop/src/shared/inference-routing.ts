// Coding-session inference routing decision (#5485, EPIC #5474).
//
// THE DECISION THIS MODULE OWNS
// ────────────────────────────────────────────────────────────────────────────
// When the desktop is about to spawn a coding turn it must pick HOW inference
// is paid for:
//
//   • "own_auth"  — the user's own Claude/Codex login (BYO). This is the
//                   existing behaviour and ALWAYS preferred when a usable own
//                   account is present (we never spend the user's credits when
//                   their own subscription can do the work).
//   • "gateway"   — route the turn through the OpenAgents inference gateway
//                   (OpenAI-compatible /v1/chat/completions) billed against the
//                   user's OpenAgents API key + pay-as-you-go credit balance.
//                   This is the DEFAULT FALLBACK for a fresh user with no own
//                   auth (or out of usable accounts) — the self-serve path.
//   • "blocked"   — neither path is usable; surface why (a blocker ref).
//
// This is a PURE function (no DOM, no RPC, no env) so the reducer and the Bun
// host can both call it and it unit-tests trivially. It takes a normalized
// snapshot of: the chosen adapter, the user's provider accounts + their
// readiness, the user's gateway preference, and the gateway readiness.
//
// Apple FM is a fully local on-device path with no provider auth and no gateway
// billing, so it is always "own_auth" (local) and never falls back.

export type InferenceAdapter = "codex" | "claude_agent" | "apple_fm"

// The user's gateway-fallback preference (persisted; see preferences.ts):
//   • "auto" — use the gateway as the default fallback when there is no usable
//              own auth (the conversion-friendly default).
//   • "off"  — never auto-route through the gateway; require own auth.
export type GatewayFallbackPreference = "auto" | "off"

// A normalized provider-account readiness row for the routing decision. We only
// need the provider + whether it is usable for a coding turn (ready, no
// blockers). Derived from the live `accounts.list` projection.
export type ProviderAccountReadiness = {
  readonly provider: string
  readonly ready: boolean
}

// What the routing decision needs to know about the gateway. Derived from the
// Bun-host readiness projection (server flag + API key presence + credits).
export type GatewayRoutingReadiness = {
  // Server-side flag gate. False until the gateway is actually served.
  readonly enabled: boolean
  // A Bun-host OpenAgents API key is present (the raw key never reaches here).
  readonly apiKeyPresent: boolean
  // Pay-as-you-go credit balance. Null = unknown (not yet fetched / fetch
  // failed); a non-positive number = no spendable credits.
  readonly creditBalance: number | null
}

export type InferenceRoutingInput = {
  readonly adapter: InferenceAdapter
  // The user's provider accounts (any provider). The decision checks for a
  // usable account matching the chosen adapter.
  readonly accounts: ReadonlyArray<ProviderAccountReadiness>
  readonly preference: GatewayFallbackPreference
  readonly gateway: GatewayRoutingReadiness
}

export type InferenceRoute = "own_auth" | "gateway" | "blocked"

export type InferenceRoutingDecision = {
  readonly route: InferenceRoute
  // A stable, public-safe reason ref for why this route was chosen. Used for
  // the UI hint and any telemetry — never raw account/key/path text.
  readonly reason: string
  // Whether the gateway is the route specifically because the user has no
  // usable own auth (the self-serve fallback). Drives the UI copy.
  readonly usedFallback: boolean
}

// Whether the user has a usable own-auth account for the chosen coding adapter.
const hasUsableOwnAuth = (
  adapter: InferenceAdapter,
  accounts: ReadonlyArray<ProviderAccountReadiness>,
): boolean => accounts.some((row) => row.provider === adapter && row.ready)

// Whether the gateway can actually serve + bill a turn right now: enabled
// server-side, an API key is present, and there is a positive (or unknown)
// credit balance. A known non-positive balance means out of credits.
const gatewayUsable = (gateway: GatewayRoutingReadiness): boolean =>
  gateway.enabled &&
  gateway.apiKeyPresent &&
  (gateway.creditBalance === null || gateway.creditBalance > 0)

export const decideInference = (
  input: InferenceRoutingInput,
): InferenceRoutingDecision => {
  // Apple FM is local on-device inference — no provider auth, no gateway bill.
  if (input.adapter === "apple_fm") {
    return {
      route: "own_auth",
      reason: "inference.route.apple_fm_local",
      usedFallback: false,
    }
  }

  const ownAuth = hasUsableOwnAuth(input.adapter, input.accounts)

  // ALWAYS prefer the user's own subscription when it can do the work — we do
  // not spend their credits while a usable own login exists.
  if (ownAuth) {
    return {
      route: "own_auth",
      reason: "inference.route.own_auth",
      usedFallback: false,
    }
  }

  // No usable own auth. The gateway is the default fallback when the user has
  // opted into it (the default) AND the gateway can serve + bill the turn.
  if (input.preference === "auto" && gatewayUsable(input.gateway)) {
    return {
      route: "gateway",
      reason: "inference.route.gateway_fallback",
      usedFallback: true,
    }
  }

  // No own auth and the gateway path is not usable — explain why so the UI can
  // point the user at the fix (enable the fallback, add credits, or sign in).
  const reason =
    input.preference === "off"
      ? "blocker.inference.no_own_auth_fallback_off"
      : !input.gateway.enabled
        ? "blocker.inference.gateway_disabled"
        : !input.gateway.apiKeyPresent
          ? "blocker.inference.gateway_api_key_missing"
          : "blocker.inference.gateway_out_of_credits"

  return { route: "blocked", reason, usedFallback: false }
}

// Whether the gateway credit balance is at/below the low-balance threshold (a
// presentational hint, not a hard gate). Unknown balance is never "low".
export const isGatewayBalanceLow = (
  creditBalance: number | null,
  lowBalanceThreshold: number,
): boolean => creditBalance !== null && creditBalance <= lowBalanceThreshold
