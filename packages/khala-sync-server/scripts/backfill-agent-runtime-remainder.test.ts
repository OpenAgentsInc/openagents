import { describe, expect, test } from "bun:test"
import { runVerifyAcrossTables } from "./backfill-agent-runtime-remainder.js"
import type { AgentRuntimeRemainderTable } from "../src/agent-runtime-remainder-backfill.js"

/**
 * Regression for the Promise.all cron-landmine audit (finding #5): the
 * `--verify` mode used to run every table's D1-vs-Postgres tally comparison
 * in a plain `for` loop with no per-table isolation, so one table's query
 * failure (including a rejection surfaced from `verifyTable`'s own internal
 * `Promise.all` over scalar tallies) aborted the whole verify run and
 * discarded visibility into every OTHER table's result. Pure decision-core
 * test only — no real D1/Postgres connection, mirroring
 * `check-pending-migrations.test.ts`'s split between the pure gate logic and
 * the live CLI entry point.
 */
describe("runVerifyAcrossTables", () => {
  const tables: AgentRuntimeRemainderTable[] = [
    "agent_profiles",
    "event_ledger_entries",
    "khala_acceptance_jobs",
  ]

  test("all clean: reports every table ok and overall clean", async () => {
    const { clean, outcomes } = await runVerifyAcrossTables(tables, async () => true)
    expect(clean).toBe(true)
    expect(outcomes).toEqual(tables.map((table) => ({ clean: true, status: "ok", table })))
  })

  test("a middle table's rejection does not discard the other tables' real results", async () => {
    const { clean, outcomes } = await runVerifyAcrossTables(tables, async (table) => {
      if (table === "event_ledger_entries") throw new Error("postgres tally query failed: connection reset")
      return true
    })

    expect(clean).toBe(false)
    expect(outcomes).toHaveLength(3)
    expect(outcomes[0]).toEqual({ clean: true, status: "ok", table: "agent_profiles" })
    expect(outcomes[1]).toEqual({
      error: "postgres tally query failed: connection reset",
      status: "error",
      table: "event_ledger_entries",
    })
    // The table AFTER the failing one still ran and reported its own real
    // result — proving the failure did not abort the rest of the loop.
    expect(outcomes[2]).toEqual({ clean: true, status: "ok", table: "khala_acceptance_jobs" })
  })

  test("a table that reports drift (returns false) is not conflated with an error", async () => {
    const { clean, outcomes } = await runVerifyAcrossTables(tables, async (table) => table !== "event_ledger_entries")

    expect(clean).toBe(false)
    expect(outcomes[1]).toEqual({ clean: false, status: "ok", table: "event_ledger_entries" })
  })

  test("never throws, even when every table fails", async () => {
    const { clean, outcomes } = await runVerifyAcrossTables(tables, async () => {
      throw new Error("wrangler d1 execute failed")
    })

    expect(clean).toBe(false)
    expect(outcomes.every((outcome) => outcome.status === "error")).toBe(true)
  })
})
