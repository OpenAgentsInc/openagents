import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { afterEach, describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  codexAccountCapacityKey,
  completePylonLink,
  codingServiceCapacityFromRuntime,
  degradeStalePresence,
  presenceClientOptionsFromEnv,
  recordAccountLinkInPresence,
  refreshPylonLink,
  registerPylon,
  sendHeartbeat,
  sha256Base64Url,
  withPresenceRetry,
} from "../src/presence"
import { hashPylonAccountRef } from "../src/account-registry"
import { verifyNip98Authorization } from "../src/nostr-identity"
import { PYLON_NIP90_PROVIDER_CAPABILITY_REF, providerNip90LaneRefs } from "../src/provider-nip90"
import { assertPublicProjectionSafe, ensurePylonLocalState, loadOrCreatePresenceState, writePresenceState } from "../src/state"
import { registerSparkPayoutTarget, sparkPayoutTargetRef } from "../src/wallet"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { CLAUDE_AGENT_CAPABILITY_REF } from "../src/claude-agent"
import { registerActiveCodingRun } from "../src/active-assignment-runs"

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")

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

function fakePresenceCliServer() {
  const requests: { path: string; body: any; headers: Headers }[] = []
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text ? JSON.parse(text) : {}
      requests.push({ path: url.pathname, body, headers: request.headers })

      expect(request.headers.get("authorization")).toBe("Bearer oa_agent_test_agent_token")
      if (typeof body.pylonRef === "string") {
        expect(request.headers.get("x-pylon-ref")).toBe(body.pylonRef)
      }

      if (url.pathname === "/api/pylons/register") {
        return Response.json({ registrationRef: `registration.${body.pylonRef}` })
      }
      if (url.pathname.includes("/heartbeat")) {
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname === "/api/pylon-links/complete") {
        return Response.json({ linkRef: `link.${body.pylonRef}` })
      }
      if (url.pathname === "/api/pylon-links/refresh") {
        return Response.json({ linkRef: `link.${body.pylonRef}.refresh` })
      }
      if (url.pathname.endsWith("/assignments")) {
        return Response.json({ assignments: [] })
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

const nonAssignmentRequestPaths = (requests: { path: string }[]) =>
  requests.map((request) => request.path).filter((path) => !path.endsWith("/assignments"))

async function runPresenceCli(input: {
  args: string[]
  env: Record<string, string>
  timeoutMs?: number
}): Promise<{ exitCode: number | null; stdout: string; stderr: string; timedOut: boolean }> {
  const proc = Bun.spawn(["bun", INDEX, "presence", ...input.args], {
    cwd: CWD,
    env: {
      ...process.env,
      OPENAGENTS_AGENT_TOKEN: "oa_agent_test_agent_token",
      PYLON_DISABLE_DAEMON_ROUTING: "1",
      PYLON_DISABLE_OPENCODE_STARTUP: "1",
      PYLON_SPARK_BACKUP_DISABLED: "1",
      ...input.env,
    },
    stderr: "pipe",
    stdout: "pipe",
  })
  let timeout: ReturnType<typeof setTimeout> | undefined
  const exit = await Promise.race([
    proc.exited.then((exitCode) => ({ exitCode, timedOut: false as const })),
    new Promise<{ exitCode: null; timedOut: true }>((resolve) => {
      timeout = setTimeout(() => {
        proc.kill()
        resolve({ exitCode: null, timedOut: true })
      }, input.timeoutMs ?? 10_000)
    }),
  ])
  if (timeout !== undefined) clearTimeout(timeout)
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ])
  return { ...exit, stderr, stdout }
}

async function runProviderCli(input: {
  args: string[]
  env: Record<string, string>
  timeoutMs?: number
}): Promise<{ exitCode: number | null; stdout: string; stderr: string; timedOut: boolean }> {
  const proc = Bun.spawn(["bun", INDEX, "provider", ...input.args], {
    cwd: CWD,
    env: {
      ...process.env,
      PYLON_DISABLE_DAEMON_ROUTING: "1",
      PYLON_DISABLE_OPENCODE_STARTUP: "1",
      PYLON_SPARK_BACKUP_DISABLED: "1",
      ...input.env,
    },
    stderr: "pipe",
    stdout: "pipe",
  })
  let timeout: ReturnType<typeof setTimeout> | undefined
  const exit = await Promise.race([
    proc.exited.then((exitCode) => ({ exitCode, timedOut: false as const })),
    new Promise<{ exitCode: null; timedOut: true }>((resolve) => {
      timeout = setTimeout(() => {
        proc.kill()
        resolve({ exitCode: null, timedOut: true })
      }, input.timeoutMs ?? 10_000)
    }),
  ])
  if (timeout !== undefined) clearTimeout(timeout)
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ])
  return { ...exit, stderr, stdout }
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

  test("heartbeat defaults Codex capacity to connected account homes", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceServer()
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--display-name",
          "Multi-Codex Capacity Test",
          "--capability-ref",
          CODEX_AGENT_CAPABILITY_REF,
        ]),
        { PYLON_HOME: home },
        "darwin",
      )
      const codexOne = join(home, "codex-one")
      const codexTwo = join(home, "codex-two")
      const codexThree = join(home, "codex-three")
      for (const accountHome of [codexOne, codexTwo, codexThree]) {
        await mkdir(accountHome, { recursive: true })
        await writeFile(join(accountHome, "auth.json"), "{}\n")
      }
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                { ref: "codex-one", provider: "codex", home: codexOne },
                { ref: "codex-two", provider: "codex", home: codexTwo },
                { ref: "codex-three", provider: "codex", home: codexThree },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )
      const env = {
        CODEX_HOME: join(home, "missing-default-codex"),
        PYLON_ACCOUNT_HOME_ROOT: join(home, "no-sibling-scan"),
        PYLON_HOME: home,
      } as NodeJS.ProcessEnv

      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        env,
        walletProbe: offlineWalletProbe,
      })

      const heartbeat = fake.requests.filter(r => r.path.includes("/heartbeat")).at(-1)!
      expect(heartbeat.body.capacityRefs).toEqual(
        expect.arrayContaining([
          "capacity.coding.codex.ready=3",
          "capacity.coding.codex.available=3",
        ]),
      )
      expect(heartbeat.body.loadRefs).toEqual(
        expect.arrayContaining([
          "load.coding.codex.busy=0",
          "load.coding.codex.queued=0",
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

  // #6331: `accounts connect codex --openagents-link` establishes a server-side
  // account->owner link but never goes through completePylonLink, so the
  // presence state the heartbeat reads stayed linked: false / linkRef: null.
  // recordAccountLinkInPresence reconciles that so the next heartbeat reports
  // linked: true with a stable link ref.
  test("recordAccountLinkInPresence marks presence linked with a stable linkRef", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--display-name", "Account Link Reconcile"]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      const before = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(before.linked).toBe(false)
      expect(before.linkRef).toBeNull()

      const linked = await recordAccountLinkInPresence(summary, {
        providerAccountRef: "provider-account-abc",
      })
      expect(linked.linked).toBe(true)
      expect(linked.linkRef).toBeTruthy()
      expect(linked.linkRef?.startsWith("link.account.")).toBe(true)

      // Persisted: a fresh read sees the linked state the heartbeat will report.
      const persisted = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(persisted.linked).toBe(true)
      expect(persisted.linkRef).toBe(linked.linkRef)

      // Idempotent: re-running keeps the existing linkRef.
      const again = await recordAccountLinkInPresence(summary, {
        providerAccountRef: "provider-account-abc",
      })
      expect(again.linkRef).toBe(linked.linkRef)

      // Public-projection safe (no raw account ref or local paths leak).
      assertPublicProjectionSafe({ linkRef: linked.linkRef })
      expect(linked.linkRef).not.toContain("provider-account-abc")
    })
  })

  test("CLI presence register, heartbeat, link-complete, and link-refresh accept --json as a no-op", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceCliServer()
      const env = {
        PYLON_HOME: home,
        PYLON_OPENAGENTS_BASE_URL: fake.baseUrl,
      }
      let pylonRef = ""

      for (const command of ["register", "heartbeat", "link-complete", "link-refresh"]) {
        const result = await runPresenceCli({
          args: [command, "--json", "--display-name", "Presence CLI JSON Test"],
          env,
        })
        expect(result.timedOut).toBe(false)
        expect(result.exitCode).toBe(0)
        expect(result.stderr).toBe("")
        const body = JSON.parse(result.stdout)
        expect(body.pylonRef).toStartWith("pylon.")
        pylonRef = body.pylonRef
      }

      expect(nonAssignmentRequestPaths(fake.requests)).toEqual([
        "/api/pylons/register",
        `/api/pylons/${encodeURIComponent(pylonRef)}/heartbeat`,
        "/api/pylon-links/complete",
        "/api/pylon-links/refresh",
      ])
    })
  }, 30_000)

  test("CLI one-shot presence heartbeat exits after JSON even when a runtime handle is left open", async () => {
    await withTempHome(async (home) => {
      const fake = fakePresenceCliServer()
      const result = await runPresenceCli({
        args: ["heartbeat", "--json", "--display-name", "Presence One Shot Exit Test"],
        env: {
          PYLON_HOME: home,
          PYLON_OPENAGENTS_BASE_URL: fake.baseUrl,
          PYLON_PRESENCE_ONESHOT_TEST_HOLD_HANDLE: "1",
        },
        timeoutMs: 5_000,
      })

      expect(result.timedOut).toBe(false)
      expect(result.exitCode).toBe(0)
      expect(result.stderr).toBe("")
      const body = JSON.parse(result.stdout)
      expect(body.heartbeatSequence).toBe(1)
      expect(nonAssignmentRequestPaths(fake.requests)).toEqual([
        `/api/pylons/${encodeURIComponent(body.pylonRef)}/heartbeat`,
      ])
    })
  }, 15_000)

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

  test("provider go-online JSON includes pylonRef and no local evidence path (#6341)", async () => {
    await withTempHome(async (home) => {
      const result = await runProviderCli({
        args: ["go-online", "--json"],
        env: { PYLON_HOME: home },
        timeoutMs: 20_000,
      })

      expect(result.timedOut).toBe(false)
      expect(result.exitCode).toBe(0)
      const json = JSON.parse(result.stdout)
      expect(json.ok).toBe(true)
      expect(json.pylonRef).toMatch(/^pylon\.[a-f0-9]{20}$/)
      expect(json.tassadar.evidencePath).toBeUndefined()
      expect(json.tassadar.evidenceRef === null || typeof json.tassadar.evidenceRef === "string").toBe(true)
      expect(result.stdout).not.toContain(home)
      assertPublicProjectionSafe(json)
    })
  })

  test("provider go-online JSON distinguishes own Codex dispatch slots from NIP-90 policy", async () => {
    await withTempHome(async (home) => {
      const codexHome = join(home, "codex-home")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), "{}")

      const result = await runProviderCli({
        args: ["go-online", "--json"],
        env: {
          CODEX_HOME: codexHome,
          OPENAGENTS_PYLON_CODEX_BUSY: "0",
          OPENAGENTS_PYLON_CODEX_CONCURRENCY: "5",
          OPENAGENTS_PYLON_CODEX_QUEUED: "0",
          PYLON_HOME: home,
          PYLON_NIP90_MAX_INFLIGHT: "1",
          PYLON_NIP90_PER_BUYER_MAX_INFLIGHT: "1",
        },
        timeoutMs: 20_000,
      })

      expect(result.timedOut).toBe(false)
      expect(result.exitCode).toBe(0)
      const json = JSON.parse(result.stdout)
      expect(json.codexAgent.state).toBe("ready")
      expect(json.policy.maxInflight).toBe(1)
      expect(json.policy.perBuyerMaxInflight).toBe(1)
      expect(json.ownCapacityDispatch).toMatchObject({
        assignmentGateRef: "gate.public.pylon.assignment_dispatch.controlled.v1",
        availableCodexAssignments: 5,
        maxCodexAssignments: 5,
        policyRefs: ["policy.public.khala_coding.own_capacity_only"],
        requiredCapabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
      })
      // #6354: the default Codex account (no registry entries) advertises its
      // own per-account slots alongside the pooled refs.
      const defaultKey = codexAccountCapacityKey(
        hashPylonAccountRef("codex", "default"),
      )
      expect(json.ownCapacityDispatch.capacityRefs).toEqual([
        "capacity.coding.codex.ready=5",
        "capacity.coding.codex.available=5",
        `capacity.coding.codex.account.${defaultKey}.ready=5`,
        `capacity.coding.codex.account.${defaultKey}.available=5`,
      ])
      expect(json.ownCapacityDispatch.loadRefs).toEqual([
        "load.coding.codex.busy=0",
        "load.coding.codex.queued=0",
        `load.coding.codex.account.${defaultKey}.busy=0`,
        `load.coding.codex.account.${defaultKey}.queued=0`,
      ])
      expect(json.ownCapacityDispatch.codexAccounts).toEqual([
        { accountKey: defaultKey, available: 5, busy: 0, queued: 0, ready: 5 },
      ])
      expect(json.ownCapacityDispatch.totalAvailableCodexAssignments).toBe(5)
      expect(json.codingCapacity).toContainEqual({
        available: 5,
        busy: 0,
        queued: 0,
        ready: 5,
        service: "codex",
      })
      assertPublicProjectionSafe(json)
    })
  })

  test("provider go-online JSON projects active local Codex assignment runs as busy (#6354)", async () => {
    await withTempHome(async (home) => {
      const codexHome = join(home, "codex-home")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), "{}")
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--display-name",
          "Active Codex Busy Test",
          "--capability-ref",
          CODEX_AGENT_CAPABILITY_REF,
        ]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      // #6354: tag the active runs with the default Codex account so per-account
      // busy load reflects the same two in-flight assignments as pooled busy.
      const defaultAccountHash = hashPylonAccountRef("codex", "default")
      await registerActiveCodingRun(state.paths, {
        accountRefHash: defaultAccountHash,
        assignmentRef: "assignment.public.no_spend.codex_busy_a",
        leaseRef: "lease.public.no_spend.codex_busy_a",
        service: "codex",
      })
      await registerActiveCodingRun(state.paths, {
        accountRefHash: defaultAccountHash,
        assignmentRef: "assignment.public.no_spend.codex_busy_b",
        leaseRef: "lease.public.no_spend.codex_busy_b",
        service: "codex",
      })

      const result = await runProviderCli({
        args: ["go-online", "--json"],
        env: {
          CODEX_HOME: codexHome,
          OPENAGENTS_PYLON_CODEX_BUSY: "0",
          OPENAGENTS_PYLON_CODEX_CONCURRENCY: "5",
          OPENAGENTS_PYLON_CODEX_QUEUED: "0",
          PYLON_HOME: home,
        },
        timeoutMs: 20_000,
      })

      expect(result.timedOut).toBe(false)
      expect(result.exitCode).toBe(0)
      const json = JSON.parse(result.stdout)
      expect(json.ownCapacityDispatch).toMatchObject({
        availableCodexAssignments: 3,
        maxCodexAssignments: 5,
      })
      const defaultKey = codexAccountCapacityKey(defaultAccountHash)
      expect(json.ownCapacityDispatch.loadRefs).toEqual([
        "load.coding.codex.busy=2",
        "load.coding.codex.queued=0",
        `load.coding.codex.account.${defaultKey}.busy=2`,
        `load.coding.codex.account.${defaultKey}.queued=0`,
      ])
      expect(json.ownCapacityDispatch.codexAccounts).toEqual([
        { accountKey: defaultKey, available: 3, busy: 2, queued: 0, ready: 5 },
      ])
      expect(json.codingCapacity).toContainEqual({
        available: 3,
        busy: 2,
        queued: 0,
        ready: 5,
        service: "codex",
      })
      expect(result.stdout).not.toContain(home)
      assertPublicProjectionSafe(json)
    })
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
