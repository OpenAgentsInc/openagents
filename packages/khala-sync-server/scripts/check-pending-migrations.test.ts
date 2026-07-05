import { describe, expect, test } from "bun:test"
import { decidePendingKhalaSyncMigrations } from "./check-pending-migrations.js"

/**
 * Guard for the Khala Sync migration deploy gate (#8410 follow-up) — pure
 * decision core only, no real Postgres connection. Wired into
 * `apps/openagents.com`'s `check:deploy` sweep so the decision logic stays
 * covered even where no direct Postgres URL is available; the LIVE check
 * (`bun scripts/check-pending-migrations.ts`, requiring
 * `KHALA_SYNC_DATABASE_URL`) is wired into the owner-run `deploy:safe` path
 * only, mirroring `apps/openagents.com/scripts/check-pending-migrations.mjs`'s
 * D1 guard/live-check split.
 */
describe("decidePendingKhalaSyncMigrations", () => {
  test("ok, exit 0, when nothing is pending", () => {
    const decision = decidePendingKhalaSyncMigrations([])
    expect(decision.ok).toBe(true)
    expect(decision.exitCode).toBe(0)
    expect(decision.message).toContain("OK — 0 pending")
  })

  test("fails, exit 1, and names every pending migration when something is pending", () => {
    const decision = decidePendingKhalaSyncMigrations([
      "0032_khala_sync_runtime_control_intents_seq.sql",
      "0033_something_else.sql",
    ])
    expect(decision.ok).toBe(false)
    expect(decision.exitCode).toBe(1)
    expect(decision.message).toContain("2 Khala Sync migration(s) PENDING")
    expect(decision.message).toContain("0032_khala_sync_runtime_control_intents_seq.sql")
    expect(decision.message).toContain("0033_something_else.sql")
    expect(decision.message).toContain("bun run migrate")
  })
})
