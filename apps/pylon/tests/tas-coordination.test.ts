import { describe, expect, test } from "bun:test"

import {
  claimWorkItem,
  isExpired,
  releaseClaim,
  type ClaimWorkItemInput,
} from "../src/tas/coordination"

describe("tas multi-agent coordination claim ledger", () => {
  test("first claim grants a lease", () => {
    const result = claimWorkItem({}, claimInput())

    expect(result.ok).toBe(true)
    expect(result.ledger["work.fixture.1"]).toEqual({
      workItemRef: "work.fixture.1",
      agentRef: "agent.fixture.alpha",
      claimedAtMs: 1_000,
      leaseExpiresAtMs: 6_000,
    })
  })

  test("concurrent double-claim is rejected while the lease is live", () => {
    const first = claimWorkItem({}, claimInput())
    const second = claimWorkItem(first.ledger, {
      ...claimInput(),
      agentRef: "agent.fixture.beta",
      nowMs: 1_000,
    })

    expect(second).toEqual({
      ok: false,
      ledger: first.ledger,
      reason: "live_claim_exists",
    })
  })

  test("claim after lease expiry grants ownership to the later agent", () => {
    const first = claimWorkItem({}, claimInput())
    const second = claimWorkItem(first.ledger, {
      ...claimInput(),
      agentRef: "agent.fixture.beta",
      nowMs: 6_000,
    })

    expect(isExpired(first.ledger["work.fixture.1"], 6_000)).toBe(true)
    expect(second.ok).toBe(true)
    expect(second.ledger["work.fixture.1"]).toEqual({
      workItemRef: "work.fixture.1",
      agentRef: "agent.fixture.beta",
      claimedAtMs: 6_000,
      leaseExpiresAtMs: 11_000,
    })
  })

  test("release frees a work item for a later claim", () => {
    const first = claimWorkItem({}, claimInput())
    const released = releaseClaim(first.ledger, {
      workItemRef: "work.fixture.1",
    })
    const second = claimWorkItem(released.ledger, {
      ...claimInput(),
      agentRef: "agent.fixture.beta",
      nowMs: 2_000,
    })

    expect(released).toEqual({
      ok: true,
      ledger: {},
    })
    expect(second.ok).toBe(true)
    expect(second.ledger["work.fixture.1"]?.agentRef).toBe(
      "agent.fixture.beta",
    )
  })
})

function claimInput(
  overrides: Partial<ClaimWorkItemInput> = {},
): ClaimWorkItemInput {
  return {
    workItemRef: "work.fixture.1",
    agentRef: "agent.fixture.alpha",
    nowMs: 1_000,
    leaseMs: 5_000,
    ...overrides,
  }
}
