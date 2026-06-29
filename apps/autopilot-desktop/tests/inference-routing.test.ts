import { describe, expect, test } from "bun:test"

import {
  decideInference,
  isGatewayBalanceLow,
  type GatewayRoutingReadiness,
  type InferenceAdapter,
  type ProviderAccountReadiness,
} from "../src/shared/inference-routing"

// #5485 (EPIC #5474): the own-auth-vs-gateway routing decision for coding turns.
// This is the heart of the issue — a fresh user with no own auth should default
// to gateway credits, while BYO-auth always wins when present.

const gateway = (
  overrides: Partial<GatewayRoutingReadiness> = {},
): GatewayRoutingReadiness => ({
  enabled: true,
  apiKeyPresent: true,
  creditBalance: 10,
  ...overrides,
})

const readyAccount = (
  provider: string,
): ProviderAccountReadiness => ({ provider, ready: true })

const blockedAccount = (
  provider: string,
): ProviderAccountReadiness => ({ provider, ready: false })

const decide = (
  adapter: InferenceAdapter,
  accounts: ProviderAccountReadiness[],
  preference: "auto" | "off",
  gw: GatewayRoutingReadiness,
) => decideInference({ adapter, accounts, preference, gateway: gw })

describe("decideInference — own-auth always wins when present", () => {
  test("a ready own account routes to own_auth even when the gateway is fully usable", () => {
    const d = decide("codex", [readyAccount("codex")], "auto", gateway())
    expect(d.route).toBe("own_auth")
    expect(d.usedFallback).toBe(false)
    expect(d.reason).toBe("inference.route.own_auth")
  })

  test("own auth wins per-adapter (a ready claude account does NOT cover a codex turn)", () => {
    const d = decide("codex", [readyAccount("claude_agent")], "auto", gateway())
    // No usable codex account → falls back to the gateway.
    expect(d.route).toBe("gateway")
    expect(d.usedFallback).toBe(true)
  })
})

describe("decideInference — gateway fallback for the no-own-auth user (the conversion path)", () => {
  test("no accounts at all + auto + usable gateway → gateway fallback", () => {
    const d = decide("codex", [], "auto", gateway())
    expect(d.route).toBe("gateway")
    expect(d.usedFallback).toBe(true)
    expect(d.reason).toBe("inference.route.gateway_fallback")
  })

  test("only a BLOCKED own account (login required) + auto → gateway fallback", () => {
    const d = decide("claude_agent", [blockedAccount("claude_agent")], "auto", gateway())
    expect(d.route).toBe("gateway")
    expect(d.usedFallback).toBe(true)
  })

  test("unknown credit balance (not yet fetched) still allows the fallback", () => {
    const d = decide("codex", [], "auto", gateway({ creditBalance: null }))
    expect(d.route).toBe("gateway")
  })
})

describe("decideInference — blocked / off paths", () => {
  test("preference off + no own auth → blocked (require own auth)", () => {
    const d = decide("codex", [], "off", gateway())
    expect(d.route).toBe("blocked")
    expect(d.reason).toBe("blocker.inference.no_own_auth_fallback_off")
  })

  test("gateway disabled server-side → blocked with the disabled reason (INERT default)", () => {
    const d = decide("codex", [], "auto", gateway({ enabled: false }))
    expect(d.route).toBe("blocked")
    expect(d.reason).toBe("blocker.inference.gateway_disabled")
  })

  test("no API key configured → blocked with the key-missing reason", () => {
    const d = decide("codex", [], "auto", gateway({ apiKeyPresent: false }))
    expect(d.route).toBe("blocked")
    expect(d.reason).toBe("blocker.inference.gateway_api_key_missing")
  })

  test("out of credits → blocked with the out-of-credits reason", () => {
    const d = decide("codex", [], "auto", gateway({ creditBalance: 0 }))
    expect(d.route).toBe("blocked")
    expect(d.reason).toBe("blocker.inference.gateway_out_of_credits")
  })
})

describe("decideInference — Apple FM is always local", () => {
  test("apple_fm routes to own_auth (local) regardless of accounts/gateway", () => {
    const d = decide("apple_fm", [], "auto", gateway({ enabled: false }))
    expect(d.route).toBe("own_auth")
    expect(d.usedFallback).toBe(false)
    expect(d.reason).toBe("inference.route.apple_fm_local")
  })
})

describe("isGatewayBalanceLow", () => {
  test("at/below threshold is low; above is not; unknown is never low", () => {
    expect(isGatewayBalanceLow(0, 1)).toBe(true)
    expect(isGatewayBalanceLow(1, 1)).toBe(true)
    expect(isGatewayBalanceLow(2, 1)).toBe(false)
    expect(isGatewayBalanceLow(null, 1)).toBe(false)
  })
})
