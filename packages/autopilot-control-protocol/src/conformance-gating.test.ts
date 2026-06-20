import { describe, expect, test } from "bun:test"

import {
  decodeSessionEvent,
  hasCapability,
  projectionLevelOf,
  verbAllowedByCapabilities,
  type PairingCredentialClaims,
  type ProjectionLevel,
  type SessionEvent,
} from "./index.js"
import { sessionEventStreamFixture } from "./fixtures.js"

const projectionRank: Record<ProjectionLevel, number> = {
  public_safe: 0,
  team: 1,
  private: 2,
}

function eventVisibleAtProjection(event: SessionEvent, projectionLevel: ProjectionLevel): boolean {
  return projectionRank[event.projectionLevel] <= projectionRank[projectionLevel]
}

function claimsFixture(
  overrides: Partial<Pick<PairingCredentialClaims, "projectionLevel" | "capabilities">>,
): PairingCredentialClaims {
  return {
    pairingRef: "pairing.fixture.readonly",
    clientId: "client.fixture.public-viewer",
    deviceClass: "browser",
    issuer: "openagents.fixture",
    audience: "autopilot-control-protocol.conformance",
    expiresAt: "2026-06-13T13:00:00.000Z",
    jti: "jti.fixture.readonly",
    projectionLevel: "public_safe",
    capabilities: ["observe_public", "read_artifact"],
    ...overrides,
  }
}

describe("cross-client gating conformance", () => {
  test("projection-level gating hides private events from public-safe clients", () => {
    const publicClaims = claimsFixture({ projectionLevel: "public_safe" })
    const publicEvent = decodeSessionEvent(sessionEventStreamFixture[0]!)
    const privateEvent = decodeSessionEvent({
      ...sessionEventStreamFixture[1]!,
      eventId: "evt.private.fixture",
      sequence: 20,
      projectionLevel: "private",
      detailRef: "private.detail.fixture",
    })

    const visibleEvents = [publicEvent, privateEvent].filter((event) =>
      eventVisibleAtProjection(event, projectionLevelOf(publicClaims)),
    )

    expect(visibleEvents).toEqual([publicEvent])
    expect(visibleEvents).not.toContain(privateEvent)
  })

  test("read-only clients cannot resolve decisions or cancel sessions", () => {
    const readOnlyClaims = claimsFixture({
      capabilities: ["observe_public", "read_artifact"],
    })

    expect(hasCapability(readOnlyClaims, "answer_decision")).toBe(false)
    expect(hasCapability(readOnlyClaims, "cancel")).toBe(false)
    expect(verbAllowedByCapabilities("decision.resolve", readOnlyClaims.capabilities)).toBe(false)
    expect(verbAllowedByCapabilities("session.cancel", readOnlyClaims.capabilities)).toBe(false)
  })
})
