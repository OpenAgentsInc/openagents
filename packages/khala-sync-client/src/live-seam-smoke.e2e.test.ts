import {
  BootstrapRequest,
  ClientGroupId,
  ClientId,
  personalScope,
  SyncSchemaVersion,
  SyncScope,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { createOverlay } from "./overlay.js"
import { createKhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"
import { createHttpKhalaSyncTransport } from "./transport.js"

/**
 * ST-1 (#8507): standing live-seam smoke — the promoted incident repro.
 *
 * Drives the REAL `createHttpKhalaSyncTransport` (real fetch, real
 * WebSocket, no fakes anywhere) against a live deployment with a real
 * COOKIE-LESS bearer: bootstrap → logPage → connectLive, plus a real
 * `createKhalaSyncSession` reaching phase `live`. The cookie-less
 * condition is the whole point — browsers attach the session cookie on
 * same-origin WS upgrades, so a browser test passes for the wrong reason;
 * mobile (and every agent client) has ONLY the bearer, which `connectLive`
 * carries as a `?token=` query param because WebSocket clients cannot set
 * an Authorization header. The server must read it via
 * `withBearerFromQueryToken` (khala-sync-connect-routes.ts). Reverting
 * that promotion reintroduces the 2026-07-06 incident (builds 10–13:
 * infinite "Loading threads") and MUST fail this test's authenticated
 * legs. Incident audit:
 * docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md.
 *
 * Env gate (skips cleanly when absent — never blocks unrelated runs):
 *   KHALA_SYNC_LIVE_SMOKE_TOKEN          cookie-less bearer (agent or user);
 *                                        NEVER hardcoded, NEVER logged.
 *   KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID  user id owning scope.user.<id>
 *                                        readable by that bearer.
 *   KHALA_SYNC_LIVE_SMOKE_BASE_URL       optional; default
 *                                        https://openagents.com.
 *
 * Deploy-gate mode: apps/openagents.com
 * scripts/predeploy-khala-sync-live-seam-smoke.mjs self-registers a
 * throwaway staging agent (same mechanism as the parallel-dispatch smoke)
 * and runs this file against staging inside `deploy:safe`.
 */

const token = (process.env.KHALA_SYNC_LIVE_SMOKE_TOKEN ?? "").trim()
const ownerUserId = (
  process.env.KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID ?? ""
).trim()
const baseUrl = (
  process.env.KHALA_SYNC_LIVE_SMOKE_BASE_URL ?? "https://openagents.com"
).replace(/\/+$/, "")

const PUBLIC_SCOPE = SyncScope.make("scope.public.tokens-served")
const SCHEMA = SyncSchemaVersion.make(1)
const watermark = SyncVersionWatermark.make
const runRef = `live-seam-${Date.now().toString(36)}`

const transport = () =>
  createHttpKhalaSyncTransport({ baseUrl, authToken: () => token })

const CONNECT_TIMEOUT_MS = 20_000
const TEST_TIMEOUT_MS = 60_000

const withTimeout = <A>(
  promise: Promise<A>,
  ms: number,
  label: string,
): Promise<A> => {
  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () =>
        reject(
          new Error(
            `khala-sync live-seam smoke: ${label} did not complete within ${ms}ms`,
          ),
        ),
      ms,
    )
  })
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer))
}

/** connectLive with a crisp timeout so a hung upgrade fails loudly. */
const connectOrFail = async (scope: SyncScope, cursor: number, label: string) => {
  const socket = await withTimeout(
    Effect.runPromise(
      transport().connectLive(scope, watermark(cursor), {
        onFrame: () => {},
        onClose: () => {},
      }),
    ),
    CONNECT_TIMEOUT_MS,
    `${label} WebSocket upgrade`,
  )
  socket.close()
}

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

describe.skipIf(token === "" || ownerUserId === "")(
  `khala-sync live seam smoke against ${baseUrl} (set KHALA_SYNC_LIVE_SMOKE_TOKEN + KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID to run)`,
  () => {
    test(
      "public scope: bootstrap → logPage → connectLive opens (wiring sanity)",
      async () => {
        const t = transport()
        const bootstrap = await Effect.runPromise(
          t.bootstrap(
            new BootstrapRequest({
              protocolVersion: 1,
              schemaVersion: SCHEMA,
              scope: PUBLIC_SCOPE,
              clientGroupId: ClientGroupId.make(`cg-${runRef}-public`),
            }),
          ),
        )
        expect(bootstrap.scope).toBe(PUBLIC_SCOPE)
        const page = await Effect.runPromise(
          t.logPage(PUBLIC_SCOPE, watermark(bootstrap.cursor ?? 0), 100),
        )
        expect(page.scope).toBe(PUBLIC_SCOPE)
        // Public scopes never hit the bearer path — if THIS leg fails the
        // problem is infra/wiring, not the query-token auth seam.
        await connectOrFail(PUBLIC_SCOPE, page.nextCursor, "public scope")
      },
      TEST_TIMEOUT_MS,
    )

    test(
      "authenticated cookie-less bearer: bootstrap → logPage → connectLive opens (the incident invariant)",
      async () => {
        const scope = personalScope(ownerUserId)
        const t = transport()
        const bootstrap = await Effect.runPromise(
          t.bootstrap(
            new BootstrapRequest({
              protocolVersion: 1,
              schemaVersion: SCHEMA,
              scope,
              clientGroupId: ClientGroupId.make(`cg-${runRef}-user`),
            }),
          ),
        )
        expect(bootstrap.scope).toBe(scope)
        const page = await Effect.runPromise(
          t.logPage(scope, watermark(bootstrap.cursor ?? 0), 100),
        )
        expect(page.scope).toBe(scope)
        // THE assertion this file exists for: the WS upgrade authenticates
        // from the `?token=` query bearer alone (no cookie, no header).
        // Reverting withBearerFromQueryToken on the connect route 401s
        // this upgrade and fails here — exactly the shipped incident.
        await connectOrFail(scope, page.nextCursor, "authenticated user scope")
      },
      TEST_TIMEOUT_MS,
    )

    test(
      "a real createKhalaSyncSession over the real transport reaches phase live",
      async () => {
        const scope = personalScope(ownerUserId)
        const store = openKhalaSyncStore(":memory:")
        cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
        const overlay = Effect.runSync(createOverlay(store, []))
        const session = createKhalaSyncSession(
          {
            baseUrl,
            clientGroupId: ClientGroupId.make(`cg-${runRef}-session`),
            clientId: ClientId.make(`client-${runRef}`),
            schemaVersion: SCHEMA,
            authToken: () => token,
          },
          store,
          overlay,
          transport(),
        )
        cleanups.push(() => Effect.runSync(Effect.ignore(session.close())))

        await Effect.runPromise(session.subscribe(scope))
        const deadline = Date.now() + CONNECT_TIMEOUT_MS * 2
        while (Date.now() < deadline) {
          if (session.state(scope).phase === "live") break
          await new Promise((resolve) => setTimeout(resolve, 250))
        }
        // The incident presented as this exact phase never arriving: the
        // session looped bootstrap-ok → catch-up-ok → connect-401 forever.
        expect(session.state(scope).phase).toBe("live")
      },
      TEST_TIMEOUT_MS,
    )
  },
)
