import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { afterEach, describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  completePylonLink,
  codingServiceCapacityFromRuntime,
  degradeStalePresence,
  presenceClientOptionsFromEnv,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
  sha256Base64Url,
  withPresenceRetry,
} from "../src/presence"
import { verifyNip98Authorization } from "../src/nostr-identity"
import { PYLON_NIP90_PROVIDER_CAPABILITY_REF, providerNip90LaneRefs } from "../src/provider-nip90"
import { assertPublicProjectionSafe, ensurePylonLocalState, loadOrCreatePresenceState, writePresenceState } from "../src/state"
import { registerSparkPayoutTarget, sparkPayoutTargetRef } from "../src/wallet"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { CLAUDE_AGENT_CAPABILITY_REF } from "../src/claude-agent"

// Inject a deterministic wallet probe for heartbeat tests that exercise the
// #5151 readiness path. Without one, heartbeats omit wallet readiness instead
// of spawning a local wallet backend.
const offlineWalletProbe = async () => ({
  configured: false,
  daemonOnline: false,
  receiveReady: false,
  sendReady: false,
})

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
  test("builds headless presence client options from OPENAGENTS_AGENT_TOKEN (#5122)", () => {
    const tokenEnv = {
      OPENAGENTS_AGENT_TOKEN: "  test-headless-agent-token  ",
      OPENAGENTS_MARKET_RELAY_URL: "wss://relay.openagents.com",
    } as NodeJS.ProcessEnv
    const authenticated = presenceClientOptionsFromEnv({
      baseUrl: "https://openagents.test",
      env: tokenEnv,
    })
    const unauthenticated = presenceClientOptionsFromEnv({
      baseUrl: "https://openagents.test",
      env: {} as NodeJS.ProcessEnv,
    })

    expect(authenticated.baseUrl).toBe("https://openagents.test")
    expect(authenticated.agentToken).toBe("test-headless-agent-token")
    expect(authenticated.env).toBe(tokenEnv)
    expect(unauthenticated.agentToken).toBeUndefined()
    expect(unauthenticated.baseUrl).toBe("https://openagents.test")
  })

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
        walletProbe: offlineWalletProbe,
      })

      expect(registered.registered).toBe(true)
      expect(heartbeat.heartbeatSequence).toBe(1)
      expect(requests.map((request) => request.path)).toEqual([
        "/api/pylons/register",
        `/api/pylons/${encodeURIComponent(registered.pylonRef)}/heartbeat`,
      ])
    })
  })

  test("a server response whose reason contains a path does NOT fail-close presence (#5268)", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "5268 inbound guard"]),
        { PYLON_HOME: home },
        "darwin",
      )
      // Before #5268 the client ran its OUTBOUND public-projection guard against the
      // INBOUND response; a server error envelope carrying a path-shaped `reason`
      // (e.g. a filesystem path) made register/heartbeat throw and the node go offline.
      // The node must now register successfully despite the path in the response.
      const fetchImpl = (async () =>
        Response.json({
          registrationRef: "registration.pylon.5268",
          reason: "/Users/someone/.cache/breez/spark/storage.sql",
        })) as typeof fetch
      const registered = await registerPylon(summary, {
        agentToken: "test-agent-token",
        baseUrl: "http://127.0.0.1:1/",
        fetch: fetchImpl,
        now: () => new Date("2026-06-17T22:00:00.000Z"),
      })
      expect(registered.registered).toBe(true)
    })
  })

  test("heartbeat publishes live wallet receive-readiness (#5151)", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Readiness Test"]),
        { PYLON_HOME: home },
        "darwin",
      )
      await registerPylon(summary, { baseUrl: fake.baseUrl })

      // A receive-ready local wallet must publish walletReady:true so the public
      // walletReadyNow projection flips without a separate report-readiness.
      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        walletProbe: async () => ({
          configured: true,
          daemonOnline: true,
          receiveReady: true,
          sendReady: false,
        }),
      })
      const ready = fake.requests.filter(r => r.path.includes("/heartbeat")).at(-1)!
      expect(ready.body.walletReadiness).toBe("receive-ready")
      expect(ready.body.walletReady).toBe(true)

      // A probe failure leaves walletReadiness "unknown" and OMITS walletReady,
      // so the server keeps the last known value (no flap to false).
      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        walletProbe: async () => {
          throw new Error("daemon unreachable")
        },
      })
      const failed = fake.requests.filter(r => r.path.includes("/heartbeat")).at(-1)!
      expect(failed.body.walletReadiness).toBe("unknown")
      expect("walletReady" in failed.body).toBe(false)
    })
  })

  test("heartbeat publishes per-service coding capacity dimensions (#6276)", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--display-name",
          "Coding Capacity Test",
          "--capability-ref",
          CODEX_AGENT_CAPABILITY_REF,
          "--capability-ref",
          CLAUDE_AGENT_CAPABILITY_REF,
        ]),
        { PYLON_HOME: home },
        "darwin",
      )
      const env = {
        OPENAGENTS_PYLON_CLAUDE_BUSY: "0",
        OPENAGENTS_PYLON_CLAUDE_CONCURRENCY: "1",
        OPENAGENTS_PYLON_CLAUDE_QUEUED: "3",
        OPENAGENTS_PYLON_CODEX_BUSY: "1",
        OPENAGENTS_PYLON_CODEX_CONCURRENCY: "4",
        OPENAGENTS_PYLON_CODEX_QUEUED: "2",
      } as NodeJS.ProcessEnv
      const state = await ensurePylonLocalState(summary)

      expect(codingServiceCapacityFromRuntime(state, env)).toEqual([
        { available: 3, busy: 1, queued: 2, ready: 4, service: "codex" },
        { available: 1, busy: 0, queued: 3, ready: 1, service: "claude" },
      ])

      await registerPylon(summary, { baseUrl: fake.baseUrl, env })
      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        env,
        walletProbe: offlineWalletProbe,
      })

      const heartbeat = fake.requests.filter(r => r.path.includes("/heartbeat")).at(-1)!
      expect(heartbeat.body.capacityRefs).toEqual(
        expect.arrayContaining([
          "capacity.coding.codex.ready=4",
          "capacity.coding.codex.available=3",
          "capacity.coding.claude.ready=1",
          "capacity.coding.claude.available=1",
        ]),
      )
      expect(heartbeat.body.loadRefs).toEqual(
        expect.arrayContaining([
          "load.coding.codex.busy=1",
          "load.coding.codex.queued=2",
          "load.coding.claude.busy=0",
          "load.coding.claude.queued=3",
        ]),
      )
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
      const heartbeat = await sendHeartbeat(summary, { baseUrl: fake.baseUrl, walletProbe: offlineWalletProbe })
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

      const result = await withPresenceRetry(() => sendHeartbeat(summary, { baseUrl: fake.baseUrl, walletProbe: offlineWalletProbe }), {
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

  test("carries provider discovery fields when the NIP-90 provider lane is declared (#4864)", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--display-name",
          "Provider Discovery Test",
          "--capability-ref",
          PYLON_NIP90_PROVIDER_CAPABILITY_REF,
        ]),
        { PYLON_HOME: home },
        "darwin",
      )
      const localState = await ensurePylonLocalState(summary)
      const env = { OPENAGENTS_MARKET_RELAY_URL: "wss://relay.openagents.com" }

      await registerPylon(summary, { baseUrl: fake.baseUrl, env })
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, env, walletProbe: offlineWalletProbe })

      const register = fake.requests.find((request) => request.path === "/api/pylons/register")
      const heartbeat = fake.requests.find((request) => request.path.includes("/heartbeat"))
      for (const request of [register, heartbeat]) {
        expect(request?.body.providerNostrPubkey).toBe(localState.identity.publicKey)
        expect(request?.body.providerNostrNpub).toBe(localState.identity.npub)
        expect(request?.body.providerMarketRelayRefs).toEqual(["wss://relay.openagents.com"])
        expect(request?.body.providerNip90LaneRefs).toEqual(providerNip90LaneRefs())
      }
      expect(register?.body.providerNip90LaneRefs).toContain("lane.public.nip90.5050.text_generation")
    })
  })

  test("omits provider discovery fields when no NIP-90 provider lane is declared (#4864)", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Non Provider Test"]),
        { PYLON_HOME: home },
        "darwin",
      )

      await registerPylon(summary, { baseUrl: fake.baseUrl })
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, walletProbe: offlineWalletProbe })

      for (const request of fake.requests) {
        expect(request.body.providerNostrPubkey).toBeUndefined()
        expect(request.body.providerNostrNpub).toBeUndefined()
        expect(request.body.providerMarketRelayRefs).toBeUndefined()
        expect(request.body.providerNip90LaneRefs).toBeUndefined()
      }
    })
  })

  test("rejects unsafe heartbeat and link projection fields", () => {
    expect(() => assertPublicProjectionSafe({ heartbeat: { providerAuth: "secret" } })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ link: { bearer: "token" } })).toThrow("not public-safe")
    expect(() => assertPublicProjectionSafe({ heartbeat: { note: "raw prompt should not be public" } })).toThrow(
      "private-data-shaped",
    )
    expect(() => assertPublicProjectionSafe({ heartbeat: { reason: "Use bearer abc123 to inspect status" } })).toThrow(
      "private-data-shaped",
    )
  })

  test("accepts post-start heartbeat diagnostics that name absent private material", () => {
    expect(() =>
      assertPublicProjectionSafe({
        heartbeat: {
          phase: "node-heartbeat-after-start",
          reason: "node heartbeat after startup was blocked with: projection.reason contains private-data-shaped text",
          notes:
            "No mnemonic, bearer token, raw wallet material, raw offer, invoice, preimage, private local config, or private logs are included here.",
          resultRefs: ["receipt.pylon.cli.training.lease.claim.20260616T0738333"],
        },
      }),
    ).not.toThrow()
  })
})


describe("auto-register Spark payout target idempotency + redaction (#5305)", () => {
  test("presence state round-trips sparkPayoutTargetRef and defaults to null", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Payout Target Idempotency"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      const fresh = await loadOrCreatePresenceState(state.paths, state.identity)
      // A fresh node has not registered a payout target yet.
      expect(fresh.sparkPayoutTargetRef).toBeNull()

      const rawSparkAddress = "spark1qpqqqqqq000000000000000000000000autoregister"
      const digestRef = sparkPayoutTargetRef(rawSparkAddress)
      await writePresenceState(state.paths, { ...fresh, sparkPayoutTargetRef: digestRef })

      // The digest persists, so a later boot SKIPS re-registration (idempotent).
      const reloaded = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(reloaded.sparkPayoutTargetRef).toBe(digestRef)

      // Redaction: the persisted presence state holds ONLY the digest ref, never
      // the raw spark1… address.
      expect(JSON.stringify(reloaded)).not.toContain(rawSparkAddress)
      expect(JSON.stringify(reloaded)).not.toContain("spark1")
      // The persisted presence state is public-projection safe.
      expect(() => assertPublicProjectionSafe(reloaded)).not.toThrow()
    })
  })

  test("idempotent register: once sparkPayoutTargetRef is set, the register call is skipped", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Payout Target Skip"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      const rawSparkAddress = "spark1qpqqqqqq000000000000000000000000skipsecond"
      const digestRef = sparkPayoutTargetRef(rawSparkAddress)

      // Simulate the idempotent guard the auto-register closure uses: register
      // only when the presence state has no recorded digest yet.
      let registerCalls = 0
      const fetchImpl: typeof fetch = async () => {
        registerCalls += 1
        return new Response(JSON.stringify({ ok: true, payoutTargetRef: digestRef }), { status: 200 })
      }
      const maybeRegister = async () => {
        const presence = await loadOrCreatePresenceState(state.paths, state.identity)
        if (presence.sparkPayoutTargetRef && presence.sparkPayoutTargetRef.trim() !== "") return
        const result = await registerSparkPayoutTarget(
          { rawSparkAddress },
          {
            agentToken: "oa_agent_test",
            baseUrl: "https://openagents.test",
            fetch: fetchImpl,
            pylonRef: state.identity.pylonRef,
          },
        )
        const next = await loadOrCreatePresenceState(state.paths, state.identity)
        await writePresenceState(state.paths, { ...next, sparkPayoutTargetRef: result.payoutTargetRef })
      }

      await maybeRegister() // first online -> registers
      await maybeRegister() // second boot -> skipped (idempotent)
      expect(registerCalls).toBe(1)

      const finalState = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(finalState.sparkPayoutTargetRef).toBe(digestRef)
      // Redaction holds end-to-end.
      expect(JSON.stringify(finalState)).not.toContain("spark1")
    })
  })
})
