// LiveHubService tests (CFG-5, #8520): scope map + single-flight
// best-effort window rebuild (the Cloud-Run-restart replacement for the
// DO's storage persistence).

import { describe, expect, test } from "bun:test"
import { decodeChangelogEntry } from "@openagentsinc/khala-sync"

import { LiveHubService } from "./service.js"

const SCOPE = "scope.thread.service-test" as never

const entry = (version: number) =>
  decodeChangelogEntry({
    committedAt: "2026-07-06T00:00:00.000Z",
    entityId: `entity-${version}`,
    entityType: "thread",
    op: "upsert",
    postImageJson: `{"id":"entity-${version}","v":${version}}`,
    scope: SCOPE,
    version,
  })

describe("LiveHubService", () => {
  test("rebuilds a scope's window from the loader on first touch, exactly once", async () => {
    let loads = 0
    const service = new LiveHubService({
      loadWindow: async () => {
        loads += 1
        return [entry(5), entry(6)]
      },
    })

    const [a, b] = await Promise.all([
      service.hubFor(SCOPE),
      service.hubFor(SCOPE),
    ])
    expect(a).toBe(b)
    expect(loads).toBe(1)
    expect(a.window()).toEqual({ lastVersion: 6, windowStartVersion: 5 })

    // The rebuilt window serves log pages immediately.
    const page = a.log(
      new URLSearchParams({ cursor: "4", scope: SCOPE as string }),
    )
    expect(page.status).toBe(200)

    // A capture append continues seamlessly from the rebuilt edge.
    const response = a.append({
      entries: [
        {
          committedAt: "2026-07-06T00:00:00.000Z",
          entityId: "entity-7",
          entityType: "thread",
          op: "upsert",
          postImageJson: `{"id":"entity-7","v":7}`,
          scope: SCOPE,
          version: 7,
        },
      ],
      scope: SCOPE,
    })
    expect(response.status).toBe(200)
    expect(a.window().lastVersion).toBe(7)
    expect(service.scopeCount()).toBe(1)
  })

  test("a failing loader leaves the hub empty (fresh-hub semantics), never throws", async () => {
    const service = new LiveHubService({
      loadWindow: async () => {
        throw new Error("postgres unreachable")
      },
    })
    const hub = await service.hubFor(SCOPE)
    expect(hub.window()).toEqual({ lastVersion: 0, windowStartVersion: 0 })
    // Empty window → behind-window on any cursor (route serves Postgres).
    const page = hub.log(
      new URLSearchParams({ cursor: "0", scope: SCOPE as string }),
    )
    expect(page.status).toBe(410)
  })

  test("no loader configured: hubs start empty and hydrate from appends", async () => {
    const service = new LiveHubService({})
    const hub = await service.hubFor(SCOPE)
    expect(hub.window().lastVersion).toBe(0)
  })
})
