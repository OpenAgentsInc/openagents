import { describe, expect, test } from "bun:test"
import { fleetOperatorMutators } from "@openagentsinc/khala-sync-server"
import { fleetClientMutators } from "../src/bun/khala-sync-service"

/**
 * Cross-package registry completeness (KS-3.2, #8292).
 *
 * The wire contract between the desktop's optimistic fleet mutators and
 * the server registry is the MUTATOR NAME (SPEC §2.4): a client mutator
 * whose name has no server counterpart pushes envelopes that become
 * recorded `unknown_mutator` rejections; a server mutator with no client
 * counterpart is unreachable from this surface. This test pins the two
 * sets to each other, in the direction the dependency graph allows
 * (desktop → khala-sync-server as a devDependency; the server package can
 * never import a client app).
 */

describe("khala sync fleet mutator registry completeness", () => {
  test("every desktop fleet client mutator has a server counterpart, and vice versa", () => {
    const clientNames = fleetClientMutators.map((m) => String(m.name)).sort()
    const serverNames = fleetOperatorMutators
      .map((m) => String(m.name))
      .sort()
    expect(clientNames).toEqual(serverNames)
  })

  test("names are unique on both sides", () => {
    const clientNames = fleetClientMutators.map((m) => String(m.name))
    const serverNames = fleetOperatorMutators.map((m) => String(m.name))
    expect(new Set(clientNames).size).toBe(clientNames.length)
    expect(new Set(serverNames).size).toBe(serverNames.length)
  })

  test("the KS-3.2 catalog is present by name on both sides", () => {
    const expected = [
      "fleet.setDesiredSlots",
      "fleet.pauseRun",
      "fleet.resumeRun",
      "fleet.pauseWorker",
      "fleet.resumeWorker",
      "fleet.acknowledgeInboxFlag",
      "fleet.stopRun",
    ]
    const clientNames = new Set(fleetClientMutators.map((m) => String(m.name)))
    const serverNames = new Set(
      fleetOperatorMutators.map((m) => String(m.name)),
    )
    for (const name of expected) {
      expect(clientNames.has(name)).toBe(true)
      expect(serverNames.has(name)).toBe(true)
    }
  })
})
