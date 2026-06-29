import { describe, expect, test } from "bun:test"

import type { BridgeTransport } from "./bridge-transport.js"
import { createDecisionCloseoutCoordinator, type CoordinatedClient } from "./decision-closeout-coordinator.js"
import { createDecisionCloseoutLedger } from "./decision-closeout-ledger.js"
import type { DecisionClient } from "./decision-closeout-receipt.js"

// A transport whose resolveDecision returns a configurable node body. Records how
// many times each surface actually relayed to the node so we can prove the
// exactly-once gate keeps a second client off the wire.
function makeTransport(
  body: Record<string, unknown> = {},
): Pick<BridgeTransport, "resolveDecision"> & { calls: number } {
  const t = {
    calls: 0,
    async resolveDecision() {
      t.calls += 1
      return body
    },
  }
  return t as Pick<BridgeTransport, "resolveDecision"> & { calls: number }
}

const REQUESTED = {
  requestId: "dec-cross-1",
  actionRef: "approve_pr_draft",
  phase: "requested" as const,
  expiresAtMs: 10_000,
}

function threeClients(): {
  clients: CoordinatedClient[]
  transports: Record<DecisionClient, Pick<BridgeTransport, "resolveDecision"> & { calls: number }>
} {
  const desktop = makeTransport({ applied: true })
  const web = makeTransport({ applied: true })
  const expo = makeTransport({ applied: true })
  return {
    clients: [
      { client: "desktop", transport: desktop },
      { client: "web", transport: web },
      { client: "expo", transport: expo },
    ],
    transports: { desktop, web, expo },
  }
}

describe("decision closeout coordinator", () => {
  test("requires at least one client surface", () => {
    expect(() => createDecisionCloseoutCoordinator({ clients: [] })).toThrow()
  })

  test("rejects duplicate client surfaces", () => {
    const t = makeTransport()
    expect(() =>
      createDecisionCloseoutCoordinator({
        clients: [
          { client: "web", transport: t },
          { client: "web", transport: t },
        ],
      }),
    ).toThrow()
  })

  test("ingest fans the same pending card out to every surface", () => {
    const { clients } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients })
    coord.ingest(REQUESTED, 0)

    for (const surface of ["desktop", "web", "expo"] as const) {
      const pending = coord.queueFor(surface)?.pending() ?? []
      expect(pending.map((e) => e.record.requestId)).toEqual(["dec-cross-1"])
    }
  })

  test("resolving on one surface records exactly one closeout and disables the others", async () => {
    const { clients, transports } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients })
    coord.ingest(REQUESTED, 0)

    // Owner approves on web.
    const first = await coord.resolve({
      client: "web",
      requestId: "dec-cross-1",
      verb: "approve",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
      nowMs: 1_000,
    })

    expect(first.result.outcome).toBe("applied")
    expect(first.alreadyClosed).toBe(false)
    expect(first.receipt?.client).toBe("web")
    expect(first.append).toEqual({ accepted: true, deduped: false })

    // The shared ledger holds exactly one canonical closeout.
    expect(coord.ledger().list().map((r) => r.requestId)).toEqual(["dec-cross-1"])
    expect(coord.ledger().summary().count).toBe(1)

    // Only web relayed to the node.
    expect(transports.web.calls).toBe(1)
    expect(transports.desktop.calls).toBe(0)
    expect(transports.expo.calls).toBe(0)

    // The other surfaces saw the broadcast: the card is no longer pending.
    expect(coord.queueFor("desktop")?.pending()).toEqual([])
    expect(coord.queueFor("expo")?.pending()).toEqual([])
    expect(coord.queueFor("desktop")?.get("dec-cross-1")?.record.state).toBe("resolved")
  })

  test("a second surface resolving after broadcast produces no second closeout", async () => {
    const { clients, transports } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients })
    coord.ingest(REQUESTED, 0)

    await coord.resolve({
      client: "web",
      requestId: "dec-cross-1",
      verb: "approve",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
      nowMs: 1_000,
    })

    // Desktop now tries the same decision; the local exactly-once gate sees the
    // record already resolved (via broadcast) and never hits the wire.
    const second = await coord.resolve({
      client: "desktop",
      requestId: "dec-cross-1",
      verb: "approve",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:05.000Z",
      nowMs: 1_500,
    })

    expect(second.alreadyClosed).toBe(true)
    expect(second.receipt).toBeUndefined()
    expect(second.append).toBeUndefined()
    expect(transports.desktop.calls).toBe(0)

    // Still exactly one closeout, still attributed to web.
    const list = coord.ledger().list()
    expect(list).toHaveLength(1)
    expect(list[0]?.client).toBe("web")
  })

  test("an answer resolution carries the answer onto the closeout receipt", async () => {
    const { clients } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients })
    coord.ingest({ ...REQUESTED, requestId: "dec-answer-1", actionRef: "provide_context" }, 0)

    const res = await coord.resolve({
      client: "expo",
      requestId: "dec-answer-1",
      verb: "answer",
      answer: "use the staging bucket",
      actor: "owner",
      decidedAt: "2026-06-20T12:10:00.000Z",
      nowMs: 2_000,
    })

    expect(res.receipt?.verb).toBe("answer")
    expect(res.receipt?.hasAnswer).toBe(true)
    expect(res.receipt?.actionRef).toBe("provide_context")
  })

  test("uses an injected shared ledger so it can wrap a persistent store", async () => {
    const ledger = createDecisionCloseoutLedger()
    const { clients } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients, ledger })
    coord.ingest(REQUESTED, 0)

    await coord.resolve({
      client: "desktop",
      requestId: "dec-cross-1",
      verb: "approve",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
      nowMs: 1_000,
    })

    expect(ledger.get("dec-cross-1")?.client).toBe("desktop")
  })

  test("resolve throws on an unknown client surface", async () => {
    const { clients } = threeClients()
    const coord = createDecisionCloseoutCoordinator({ clients: [clients[0]!] })
    await expect(
      coord.resolve({
        client: "web",
        requestId: "dec-cross-1",
        verb: "approve",
        actor: "owner",
        decidedAt: "2026-06-20T12:00:00.000Z",
        nowMs: 1_000,
      }),
    ).rejects.toThrow()
  })

  test("a node-reported duplicate on the first surface still records one closeout", async () => {
    // Node says duplicate (e.g. the resolve was retried at the transport layer).
    const dup = makeTransport({ duplicate: true })
    const coord = createDecisionCloseoutCoordinator({
      clients: [{ client: "web", transport: dup }],
    })
    coord.ingest(REQUESTED, 0)

    const res = await coord.resolve({
      client: "web",
      requestId: "dec-cross-1",
      verb: "approve",
      actor: "owner",
      decidedAt: "2026-06-20T12:00:00.000Z",
      nowMs: 1_000,
    })

    expect(res.result.outcome).toBe("duplicate")
    expect(res.receipt?.outcome).toBe("duplicate")
    expect(coord.ledger().summary().count).toBe(1)
  })
})
