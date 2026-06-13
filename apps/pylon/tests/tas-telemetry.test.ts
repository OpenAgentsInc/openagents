import { describe, expect, test } from "bun:test"

import {
  redactTelemetry,
  type AccountHealthTelemetryEvent,
  type ProviderRoutingTelemetryEvent,
  type RateLimitTelemetryEvent,
  type ReconnectTelemetryEvent,
} from "../src/tas/telemetry"

describe("tas telemetry redaction", () => {
  test("redacts account health PII while preserving health metrics", () => {
    const event: AccountHealthTelemetryEvent = {
      type: "account_health",
      eventRef: "telemetry.fixture.account_health",
      occurredAtMs: 1_718_000_000_000,
      accountRef: "account.ref.codex.primary",
      providerId: "chatgpt_codex",
      status: "degraded",
      latencyMs: 235,
      consecutiveFailures: 2,
      errorClass: "provider_unavailable",
      email: "operator@example.test",
      accessToken: "sk-test-account-health-token",
    }

    const redacted = redactTelemetry(event)
    const serialized = JSON.stringify(redacted)

    expect(redacted.type).toBe("account_health")
    expect(redacted.status).toBe("degraded")
    expect(redacted.latencyMs).toBe(235)
    expect(redacted.consecutiveFailures).toBe(2)
    expect(serialized).not.toContain("operator@example.test")
    expect(serialized).not.toContain("sk-test-account-health-token")
    expect(redacted.email).toBeUndefined()
    expect(redacted.accessToken).toBeUndefined()
    expect(redacted.redactionRefs).toHaveLength(2)
    expect(redacted.redactionRefs.map((ref) => ref.field)).toEqual([
      "email",
      "accessToken",
    ])
    expect(redacted.redactionRefs[0]?.digestRef).toStartWith(
      "digest.pylon.telemetry.",
    )
  })

  test("redacts rate-limit provider payloads and tokens while preserving counters", () => {
    const event: RateLimitTelemetryEvent = {
      type: "rate_limit",
      eventRef: "telemetry.fixture.rate_limit",
      occurredAtMs: 1_718_000_000_100,
      accountRef: "account.ref.codex.primary",
      providerId: "chatgpt_codex",
      limited: true,
      remainingRequests: 0,
      retryAfterMs: 60_000,
      resetAtMs: 1_718_000_060_100,
      sourceDigestRef: "digest.pylon.account_quota.fixture",
      providerPayload: {
        message: "raw provider payload",
        token: "sk-test-rate-limit-token",
      },
      bearerToken: "Bearer test-rate-limit-token",
    }

    const redacted = redactTelemetry(event)
    const serialized = JSON.stringify(redacted)

    expect(redacted.type).toBe("rate_limit")
    expect(redacted.limited).toBe(true)
    expect(redacted.remainingRequests).toBe(0)
    expect(redacted.retryAfterMs).toBe(60_000)
    expect(redacted.sourceDigestRef).toBe("digest.pylon.account_quota.fixture")
    expect(serialized).not.toContain("raw provider payload")
    expect(serialized).not.toContain("sk-test-rate-limit-token")
    expect(serialized).not.toContain("Bearer test-rate-limit-token")
    expect(redacted.providerPayload).toBeUndefined()
    expect(redacted.bearerToken).toBeUndefined()
    expect(redacted.redactionRefs.map((ref) => ref.field)).toEqual([
      "providerPayload",
      "bearerToken",
    ])
  })

  test("redacts provider routing URLs with credentials while preserving route metadata", () => {
    const event: ProviderRoutingTelemetryEvent = {
      type: "provider_routing",
      eventRef: "telemetry.fixture.provider_routing",
      occurredAtMs: 1_718_000_000_200,
      accountRef: "account.ref.codex.primary",
      providerId: "router.local",
      routeRef: "route.fixture.primary",
      selectedProviderId: "chatgpt_codex",
      candidateProviderIds: ["chatgpt_codex", "anthropic_claude"],
      reason: "rate_limit",
      latencyMs: 18,
      rawUrl: "https://user:password@example.test/v1?token=query-secret",
    }

    const redacted = redactTelemetry(event)
    const serialized = JSON.stringify(redacted)

    expect(redacted.type).toBe("provider_routing")
    expect(redacted.routeRef).toBe("route.fixture.primary")
    expect(redacted.selectedProviderId).toBe("chatgpt_codex")
    expect(redacted.candidateProviderIds).toEqual([
      "chatgpt_codex",
      "anthropic_claude",
    ])
    expect(redacted.reason).toBe("rate_limit")
    expect(redacted.latencyMs).toBe(18)
    expect(serialized).not.toContain("user:password")
    expect(serialized).not.toContain("query-secret")
    expect(redacted.rawUrl).toBeUndefined()
    expect(redacted.redactionRefs).toEqual([
      {
        field: "rawUrl",
        reason: "credentialed_url",
        digestRef: expect.stringMatching(/^digest\.pylon\.telemetry\.[a-f0-9]{24}$/),
      },
    ])
  })

  test("redacts reconnect PII and credentialed URLs while preserving retry metrics", () => {
    const event: ReconnectTelemetryEvent = {
      type: "reconnect",
      eventRef: "telemetry.fixture.reconnect",
      occurredAtMs: 1_718_000_000_300,
      providerId: "chatgpt_codex",
      connectionRef: "connection.fixture.remote",
      attempt: 3,
      success: false,
      backoffMs: 5_000,
      durationMs: 420,
      lastErrorClass: "network_timeout",
      operatorEmail: "operator@example.test",
      reconnectUrl: "https://example.test/session?access_token=secret-token",
    }

    const redacted = redactTelemetry(event)
    const serialized = JSON.stringify(redacted)

    expect(redacted.type).toBe("reconnect")
    expect(redacted.connectionRef).toBe("connection.fixture.remote")
    expect(redacted.attempt).toBe(3)
    expect(redacted.success).toBe(false)
    expect(redacted.backoffMs).toBe(5_000)
    expect(redacted.durationMs).toBe(420)
    expect(redacted.lastErrorClass).toBe("network_timeout")
    expect(serialized).not.toContain("operator@example.test")
    expect(serialized).not.toContain("secret-token")
    expect(redacted.operatorEmail).toBeUndefined()
    expect(redacted.reconnectUrl).toBeUndefined()
    expect(redacted.redactionRefs.map((ref) => ref.reason)).toEqual([
      "email",
      "credentialed_url",
    ])
  })
})
