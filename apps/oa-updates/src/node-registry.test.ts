import { describe, expect, test } from "bun:test"

import { createNodeRegistry, type NodeRegistration } from "./node-registry"

const registration = (
  nodeRef: string,
  updatedAt: number,
  overrides: Partial<NodeRegistration> = {},
): NodeRegistration => ({
  nodeRef,
  updatedAt,
  tailnetRef: "tailnet_alpha",
  controlToken: `control_${nodeRef}_${updatedAt}`,
  ...overrides,
})

describe("node registry", () => {
  test("registers and lists registrations per owner", () => {
    const registry = createNodeRegistry()
    const first = registration("node_one", 1000)
    const second = registration("node_two", 1100)

    registry.register("owner_alpha", first)
    registry.register("owner_alpha", second)

    expect(registry.listForOwner("owner_alpha")).toEqual([first, second])
  })

  test("deduplicates by nodeRef with last write winning by updatedAt", () => {
    const registry = createNodeRegistry()
    const first = registration("node_one", 1000, {
      controlToken: "old_token",
    })
    const newer = registration("node_one", 2000, {
      controlToken: "new_token",
    })
    const older = registration("node_one", 1500, {
      controlToken: "ignored_token",
    })

    registry.register("owner_alpha", first)
    registry.register("owner_alpha", newer)
    registry.register("owner_alpha", older)

    expect(registry.listForOwner("owner_alpha")).toEqual([newer])
  })

  test("prunes stale registrations older than maxAge", () => {
    const registry = createNodeRegistry()

    registry.register("owner_alpha", registration("fresh", 9_500))
    registry.register("owner_alpha", registration("borderline", 9_000))
    registry.register("owner_alpha", registration("stale", 8_999))
    registry.pruneStale(10_000, 1_000)

    expect(registry.listForOwner("owner_alpha")).toEqual([
      registration("fresh", 9_500),
      registration("borderline", 9_000),
    ])
  })

  test("handles ISO-string timestamps for dedup and prune", () => {
    const registry = createNodeRegistry()
    // Nodes register with Date#toISOString — earlier code did numeric math on
    // the string (NaN), so dedup-by-newer and prune both silently failed.
    registry.register("owner_alpha", {
      nodeRef: "n",
      updatedAt: "2026-06-13T12:00:00.000Z",
      controlToken: "old",
    })
    registry.register("owner_alpha", {
      nodeRef: "n",
      updatedAt: "2026-06-13T12:00:30.000Z",
      controlToken: "new",
    })
    expect(registry.listForOwner("owner_alpha")[0].controlToken).toBe("new")

    // Stale (older than maxAge) and unparseable timestamps are both pruned.
    registry.register("owner_alpha", {
      nodeRef: "bad",
      updatedAt: "not-a-date",
      controlToken: "x",
    })
    registry.pruneStale(Date.parse("2026-06-13T12:05:00.000Z"), 120_000)
    expect(registry.listForOwner("owner_alpha")).toEqual([])
  })

  test("isolates owners from each other", () => {
    const registry = createNodeRegistry()
    const ownerARegistration = registration("shared_node", 1000, {
      controlToken: "owner_a_token",
    })
    const ownerBRegistration = registration("shared_node", 1000, {
      controlToken: "owner_b_token",
    })

    registry.register("owner_alpha", ownerARegistration)
    registry.register("owner_beta", ownerBRegistration)

    expect(registry.listForOwner("owner_alpha")).toEqual([ownerARegistration])
    expect(registry.listForOwner("owner_beta")).toEqual([ownerBRegistration])
  })
})
