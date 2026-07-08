// Credit-balance live-delivery guard (Khala-Sync realtime thoroughness pass;
// siblings #8554/#8555/#8556/#8557).
//
// Behavior contract: khala_sync.credit_balance.change_delivers_live.v1
//
// This closes a specific gap: prod proved a credit_balance change is DELIVERED
// live (0 backlog), but nothing automated asserted that a credit_balance delta
// actually reaches a subscriber — only that it is WRITTEN. This test drives the
// REAL live pipe end to end with real modules and no external network:
//
//   producer writes a credit_balance change
//     (repairUserCreditBalance / applyUserCreditBalanceDelta,
//      packages/khala-sync-server/src/user-credit-balance-projection.ts)
//   → real Postgres khala_sync_changelog (version bump)
//   → real capture pass (runCapturePass, @openagentsinc/khala-sync-server/capture)
//   → HTTP /append into the REAL LiveHub server (startLiveHubServer, ./server.ts)
//   → real ScopeHub fan-out of a DeltaFrame to an attached subscriber socket
//
// The subscriber is a structural HubSocketLike observer attached through the
// SAME ScopeHub.attachSocket call the WebSocket `open` handler uses in
// server.ts, so the observed fan-out is the production delivery path — not a
// stand-in. If the delivery path is ever dropped or broken, the live-delivery
// guard below fails.

import {
  CREDIT_BALANCE_ENTITY_TYPE,
  DeltaFrame,
  decodeCreditBalanceEntity,
  decodeLiveFrame,
  personalScope,
} from "@openagentsinc/khala-sync"
import type { SyncScope } from "@openagentsinc/khala-sync"
import {
  applyUserCreditBalanceDelta,
  applyUserCreditBalanceDeltaBestEffort,
  repairUserCreditBalance,
} from "@openagentsinc/khala-sync-server"
import { runCapturePass, type CaptureConfig } from "@openagentsinc/khala-sync-server/capture"
import { runMigrations } from "@openagentsinc/khala-sync-server/migrate"
import type { SyncSql } from "@openagentsinc/khala-sync-server"
import {
  hasLocalPostgres,
  startLocalPostgres,
  type LocalPostgres,
} from "@openagentsinc/khala-sync-server/test/local-postgres"
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"

import type { HubSocketLike } from "./scope-hub.js"
import { startLiveHubServer, type LiveHubServer } from "./server.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// A structural subscriber that records the frames the hub fans out to it.
// This is exactly the HubSocketLike adapter server.ts builds for a live WS,
// so what it observes is the real DeltaFrame delivery.
// ---------------------------------------------------------------------------

class ObserverSocket implements HubSocketLike {
  readonly sent: Array<string> = []
  closed: { code?: number | undefined; reason?: string | undefined } | undefined

  send(message: string): void {
    if (this.closed !== undefined) throw new Error("observer socket closed")
    this.sent.push(message)
  }

  close(code?: number, reason?: string): void {
    this.closed = { code, reason }
  }

  /** Decoded live frames delivered so far, in order. */
  liveFrames() {
    return this.sent.map((text) => decodeLiveFrame(JSON.parse(text) as unknown))
  }

  /**
   * Every credit_balance post-image delivered as a DeltaFrame, paired with the
   * delivering frame's version cursor. Decodes through the SAME boundary codec
   * the mobile drawer uses (decodeCreditBalanceEntity).
   */
  deliveredCreditBalances() {
    return this.liveFrames()
      .filter((frame): frame is DeltaFrame => frame._tag === "DeltaFrame")
      .flatMap((frame) =>
        frame.entries
          .filter((entry) => String(entry.entityType) === CREDIT_BALANCE_ENTITY_TYPE)
          .map((entry) => ({
            version: Number(frame.cursor),
            entity: decodeCreditBalanceEntity(
              JSON.parse(entry.postImageJson ?? "null") as unknown,
            ),
          })),
      )
  }
}

const LIVE_HUB_TOKEN = "credit-balance-live-delivery-test-token"

let userCounter = 0
const freshUserId = (): string => `credit-live-user-${Date.now()}-${++userCounter}`

describe.skipIf(!hasLocalPostgres())(
  "credit_balance change delivers live over Khala Sync",
  () => {
    let pg: LocalPostgres
    let sql: SQL
    let databaseUrl: string
    let server: LiveHubServer

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_credit_live")
      await admin.end()
      databaseUrl = pg.urlFor("khala_sync_credit_live")
      const result = await runMigrations({ databaseUrl })
      // Core changelog (0001), capture checkpoints (0002), and the credit
      // balance projection tables (0038) all apply through the one runner.
      expect(result.applied).toContain("0001_khala_sync_core.sql")
      expect(result.applied).toContain("0002_khala_sync_capture.sql")
      expect(result.applied).toContain("0038_khala_sync_user_credit_balances.sql")
      sql = new SQL({ url: databaseUrl, max: 10 })

      // The REAL LiveHub server (no rebuild loader: hubs hydrate from capture
      // appends, and a subscriber attaches to the live edge — the production
      // fresh-hub path). Ping interval far beyond the test so keepalive frames
      // never pollute the observed delta stream.
      server = startLiveHubServer({
        token: LIVE_HUB_TOKEN,
        port: 0,
        pingIntervalMs: 3_600_000,
      })
    })

    afterAll(async () => {
      await server?.stop()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    const captureConfig = (): CaptureConfig => ({
      databaseUrl,
      hubAppendUrl: `http://127.0.0.1:${server.port}/append`,
      adminToken: LIVE_HUB_TOKEN,
      pushRetryBackoffMs: 1,
    })

    // -----------------------------------------------------------------------

    test("a >=1c credit_balance delta is DELIVERED to a live subscriber with the new balance (khala_sync.credit_balance.change_delivers_live.v1)", async () => {
      const userId = freshUserId()
      const scope: SyncScope = personalScope(userId)

      // Seed the projection at 1000c (the admin backfill: the only path that
      // initializes a user's balance row before deltas may apply).
      const backfill = await repairUserCreditBalance(s(), {
        userId,
        exactBalanceUsdCents: 1000,
        source: "backfill",
        auditNote: "credit-balance live-delivery guard seed",
      })
      const backfillVersion = Number(backfill.entry.version)
      expect(backfill.balance.balanceUsdCents).toBe(1000)

      // Drain the seed to the hub so the subscriber attaches at the LIVE edge
      // (nothing to catch up) — proving the NEXT delta arrives live, not as
      // catch-up history.
      const seedPass = await runCapturePass(sql, captureConfig())
      expect(seedPass.scopes.find((r) => r.scope === scope)?.error).toBeUndefined()
      const hub = await server.service.hubFor(scope)
      expect(hub.window().lastVersion).toBe(backfillVersion)

      // Subscribe: attach exactly as server.ts's WebSocket open handler does.
      const observer = new ObserverSocket()
      hub.attachSocket(observer, backfillVersion)
      // At the edge: no catch-up frames yet.
      expect(observer.sent).toHaveLength(0)

      // Apply a -5c delta through the REAL producer → changelog version bump.
      const delta = await applyUserCreditBalanceDelta(s(), {
        userId,
        idempotencyKey: "evt-credit-live-delivery-1",
        deltaUsdCents: -5,
        observedAt: "2026-07-08T00:00:00.000Z",
      })
      expect(delta.applied).toBe(true)
      if (!delta.applied) throw new Error("delta should have applied")
      expect(delta.balance.balanceUsdCents).toBe(995)
      const deltaVersion = Number(delta.entry.version)
      expect(deltaVersion).toBeGreaterThan(backfillVersion)

      // Capture pass + hub append: the live pipe carries the delta to the
      // subscriber.
      const livePass = await runCapturePass(sql, captureConfig())
      expect(livePass.scopes.find((r) => r.scope === scope)?.error).toBeUndefined()

      // The DeltaFrame was DELIVERED with the new decoded balance and a higher
      // version. This is the assertion that fails if live delivery breaks.
      const delivered = observer.deliveredCreditBalances()
      expect(delivered.length).toBeGreaterThanOrEqual(1)
      const latest = delivered[delivered.length - 1]!
      expect(latest.entity.userId).toBe(userId)
      expect(latest.entity.balanceUsdCents).toBe(995)
      expect(latest.version).toBe(deltaVersion)
      expect(latest.version).toBeGreaterThan(backfillVersion)
    })

    // -----------------------------------------------------------------------

    test("a sub-cent delta that rounds to 0c produces NO version bump and NO spurious frame (documents intentional behavior)", async () => {
      const userId = freshUserId()
      const scope: SyncScope = personalScope(userId)

      const backfill = await repairUserCreditBalance(s(), {
        userId,
        exactBalanceUsdCents: 1000,
        source: "backfill",
        auditNote: "credit-balance sub-cent guard seed",
      })
      const backfillVersion = Number(backfill.entry.version)

      const seedPass = await runCapturePass(sql, captureConfig())
      expect(seedPass.scopes.find((r) => r.scope === scope)?.error).toBeUndefined()
      const hub = await server.service.hubFor(scope)
      expect(hub.window().lastVersion).toBe(backfillVersion)

      const observer = new ObserverSocket()
      hub.attachSocket(observer, backfillVersion)
      expect(observer.sent).toHaveLength(0)

      // A sub-cent charge that rounds to 0c at the shared BTC/USD conversion
      // arrives here as a 0c delta. The cents-native producer refuses it
      // (invalid_input) — no idempotency guard row, no balance UPDATE, no
      // changelog append, and therefore no version bump. So the honest,
      // intentional outcome is: nothing is delivered to the subscriber.
      const outcome = await applyUserCreditBalanceDeltaBestEffort(s(), {
        userId,
        idempotencyKey: "evt-credit-subcent-zero-1",
        deltaUsdCents: 0,
        observedAt: "2026-07-08T00:00:00.000Z",
      })
      expect(outcome.ok).toBe(false)
      if (!outcome.ok) expect(outcome.diagnostic.reason).toBe("invalid_input")

      // Capture has nothing new to push; the subscriber gets no frame.
      const afterPass = await runCapturePass(sql, captureConfig())
      expect(afterPass.scopes.find((r) => r.scope === scope)).toBeUndefined()
      expect(hub.window().lastVersion).toBe(backfillVersion)
      expect(observer.sent).toHaveLength(0)
      expect(observer.deliveredCreditBalances()).toHaveLength(0)
    })
  },
)
