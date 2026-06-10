import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { afterEach, describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  completePylonLink,
  degradeStalePresence,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
  sha256Base64Url,
  withPresenceRetry,
} from "../src/presence"
import { verifyNip98Authorization } from "../src/nostr-identity"
import { assertPublicProjectionSafe, ensurePylonLocalState, loadOrCreatePresenceState } from "../src/state"

const servers: ReturnType<typeof Bun.serve>[] = []

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-presence-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

function fakePresenceServer(input: { failHeartbeats?: number } = {}) {
  const requests: { path: string; body: any; headers: Headers }[] = []
  let heartbeatFailures = input.failHeartbeats ?? 0
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text ? JSON.parse(text) : {}
      requests.push({ path: url.pathname, body, headers: request.headers })

      const event = verifyNip98Authorization(request.headers.get("authorization"), {
        method: request.method,
        url: request.url,
        body: text,
        maxSkewSeconds: 300_000,
      })
      expect(request.headers.get("x-nip98-body-sha256")).toBeNull()
      expect(request.headers.get("x-nip98-signature")).toBeNull()
      expect(request.headers.get("x-nip98-pubkey")).toBeNull()
      expect(request.headers.get("x-pylon-ref")).toBe(body.pylonRef)
      if (body.identity?.publicKey) expect(event.pubkey).toBe(body.identity.publicKey)
      if (body.publicKey) expect(event.pubkey).toBe(body.publicKey)

      if (url.pathname.includes("/heartbeat") && heartbeatFailures > 0) {
        heartbeatFailures -= 1
        return Response.json({ errorRef: "error.fake.retry" }, { status: 503 })
      }

      if (url.pathname === "/api/pylons/register") {
        return Response.json({ registrationRef: `registration.${body.pylonRef}` })
      }
      if (url.pathname.includes("/heartbeat")) {
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname === "/api/pylon-links/complete") {
        expect(body.bodyHash).toBe(
          sha256Base64Url(
            JSON.stringify({
              schema: "openagents.pylon.link.v0.3",
              pylonRef: body.pylonRef,
              npub: body.npub,
              publicKey: body.publicKey,
            }),
          ),
        )
        return Response.json({ linkRef: `link.${body.pylonRef}` })
      }
      if (url.pathname === "/api/pylon-links/refresh") {
        return Response.json({ linkRef: `link.${body.pylonRef}.refresh` })
      }
      return Response.json({ errorRef: "error.not_found" }, { status: 404 })
    },
  })
  servers.push(server)
  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    requests,
  }
}

describe("Pylon presence registration and heartbeat", () => {
  test("uses bearer auth and idempotency keys when an agent token is supplied", async () => {
    await withTempHome(async (home) => {
      const requests: { path: string; body: any; headers: Headers }[] = []
      const server = Bun.serve({
        port: 0,
        async fetch(request) {
          const url = new URL(request.url)
          const text = await request.text()
          const body = text ? JSON.parse(text) : {}
          requests.push({ path: url.pathname, body, headers: request.headers })

          expect(request.headers.get("authorization")).toBe("Bearer test-agent-token")
          expect(request.headers.get("idempotency-key")).toMatch(/^pylon-presence:pylon\./)
          expect(request.headers.get("x-pylon-ref")).toBe(body.pylonRef)

          if (url.pathname === "/api/pylons/register") {
            return Response.json({ registrationRef: `registration.${body.pylonRef}` })
          }
          if (url.pathname.includes("/heartbeat")) {
            return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
          }
          return Response.json({ errorRef: "error.not_found" }, { status: 404 })
        },
      })
      servers.push(server)
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Bearer Presence Test"]),
        { PYLON_HOME: home },
        "darwin",
      )

      const registered = await registerPylon(summary, {
        agentToken: "test-agent-token",
        baseUrl: `http://127.0.0.1:${server.port}`,
        now: () => new Date("2026-06-10T12:30:00.000Z"),
      })
      const heartbeat = await sendHeartbeat(summary, {
        agentToken: "test-agent-token",
        baseUrl: `http://127.0.0.1:${server.port}`,
        now: () => new Date("2026-06-10T12:31:00.000Z"),
      })

      expect(registered.registered).toBe(true)
      expect(heartbeat.heartbeatSequence).toBe(1)
      expect(requests.map((request) => request.path)).toEqual([
        "/api/pylons/register",
        `/api/pylons/${encodeURIComponent(registered.pylonRef)}/heartbeat`,
      ])
    })
  })

  test("registers, heartbeats, completes link, and refreshes link against a fake server", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Presence Test", "--capability-ref", "cap.gepa.retained.v1"]),
        { PYLON_HOME: home },
        "darwin",
      )

      const registered = await registerPylon(summary, { baseUrl: fake.baseUrl })
      const heartbeat = await sendHeartbeat(summary, { baseUrl: fake.baseUrl })
      const linked = await completePylonLink(summary, { baseUrl: fake.baseUrl })
      const refreshed = await refreshPylonLink(summary, { baseUrl: fake.baseUrl })

      expect(registered.registered).toBe(true)
      expect(heartbeat.heartbeatSequence).toBe(1)
      expect(heartbeat.lastHeartbeatAt).toBeTruthy()
      expect(linked.linked).toBe(true)
      expect(refreshed.linkRef?.endsWith(".refresh")).toBe(true)
      expect(fake.requests.map((request) => request.path)).toEqual([
        "/api/pylons/register",
        `/api/pylons/${encodeURIComponent(registered.pylonRef)}/heartbeat`,
        "/api/pylon-links/complete",
        "/api/pylon-links/refresh",
      ])
    })
  })

  test("retries transient heartbeat failure and records fresh state after success", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer({ failHeartbeats: 1 })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--display-name", "Retry Test"]), { PYLON_HOME: home }, "linux")
      const retries: number[] = []

      const result = await withPresenceRetry(() => sendHeartbeat(summary, { baseUrl: fake.baseUrl }), {
        attempts: 2,
        onRetry: (_error, attempt) => retries.push(attempt),
      })

      expect(result.heartbeatSequence).toBe(1)
      expect(result.stale).toBe(false)
      expect(retries).toEqual([1])
      expect(fake.requests.filter((request) => request.path.includes("/heartbeat")).length).toBe(2)
    })
  })

  test("degrades stale heartbeat state to explicit blocker refs", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const localState = await ensurePylonLocalState(summary)
      const presence = await loadOrCreatePresenceState(localState.paths, localState.identity)

      const neverHeartbeat = degradeStalePresence(presence, { now: new Date("2026-06-09T00:00:00.000Z"), staleAfterMs: 10 })
      const staleHeartbeat = degradeStalePresence(
        { ...presence, lastHeartbeatAt: "2026-06-09T00:00:00.000Z" },
        { now: new Date("2026-06-09T00:05:00.000Z"), staleAfterMs: 30_000 },
      )

      expect(neverHeartbeat.stale).toBe(true)
      expect(neverHeartbeat.blockerRefs).toContain("blocker.presence.never_heartbeat")
      expect(staleHeartbeat.stale).toBe(true)
      expect(staleHeartbeat.blockerRefs).toContain("blocker.presence.stale_heartbeat")
    })
  })

  test("rejects unsafe heartbeat and link projection fields", () => {
    expect(() => assertPublicProjectionSafe({ heartbeat: { providerAuth: "secret" } })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ link: { bearer: "token" } })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ heartbeat: { note: "raw prompt should not be public" } })).toThrow(
      "private-data-shaped",
    )
  })
})
