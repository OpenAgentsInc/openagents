import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  agentRuntimeRemainderRowHash,
  d1AgentRuntimeRemainderNewestHashes,
  orderingDensityFromRows,
  postgresAgentRuntimeRemainderNewestHashes,
  postgresAgentRuntimeRemainderRowCount,
  postgresEventLedgerOrderingDensity,
  upsertAgentRuntimeRemainderRows,
  type D1SourceRow,
} from "./agent-runtime-remainder-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const credentialRow = (
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  created_at: "2026-07-04T18:00:00.000Z",
  expires_at: null,
  id,
  last_used_at: null,
  name: "Programmatic token",
  openauth_user_id: null,
  revoked_at: null,
  status: "active",
  token_hash: `sha256:${id}:private`,
  token_prefix: "oa_agent_abcd",
  user_id: `agent-user-${id}`,
  ...overrides,
})

const profileRow = (
  userId: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  created_at: "2026-07-04T18:00:00.000Z",
  metadata_json: "{}",
  slug: `agent-${userId}`,
  updated_at: "2026-07-04T18:00:00.000Z",
  user_id: userId,
  ...overrides,
})

const eventLedgerRow = (
  owner: string,
  sequence: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  actor_ref: `github:user:${sequence}`,
  content_ref: `github:comment:${sequence}`,
  created_at: `2026-07-04T18:00:0${sequence}.000Z`,
  entry_id: `${owner}:event:${sequence}`,
  event_type: "issue_comment.created",
  external_ref: `comment:${sequence}`,
  handled_at: null,
  handled_by_definition_id: null,
  handled_by_run_id: null,
  handled_reason_ref: null,
  handled_state: "open",
  occurred_at: `2026-07-04T18:00:0${sequence}.000Z`,
  ordering_key: `github:comment:${sequence}`,
  ordering_sequence: sequence,
  owner_agent_user_id: owner,
  owner_ref: `agent:${owner}`,
  payload_summary_json: "{}",
  received_at: `2026-07-04T18:00:0${sequence}.000Z`,
  source: "github",
  source_refs_json: "[]",
  subject_ref: "github:issue:8334",
  training_consent: 0,
  updated_at: `2026-07-04T18:00:0${sequence}.000Z`,
  ...overrides,
})

describe("agent runtime remainder pure verification helpers", () => {
  test("credential hashes are hash-only and keyed by credential id", () => {
    const row = credentialRow("cred-a")
    const changedSecret = { ...row, token_hash: "sha256:different" }
    expect(agentRuntimeRemainderRowHash("agent_credentials", row)).not.toBe(
      agentRuntimeRemainderRowHash("agent_credentials", changedSecret),
    )
    expect(d1AgentRuntimeRemainderNewestHashes("agent_credentials", [row])).toEqual([
      {
        hash: agentRuntimeRemainderRowHash("agent_credentials", row),
        key: "cred-a",
      },
    ])
  })

  test("event ledger density catches per-owner ordering gaps", () => {
    const contiguous = orderingDensityFromRows([
      {
        distinct_sequences: 3,
        entries: 3,
        max_sequence: 3,
        min_sequence: 1,
        owner_agent_user_id: "owner-a",
      },
    ])
    expect(contiguous.gappedOwners).toBe(0)

    const gapped = orderingDensityFromRows([
      {
        distinct_sequences: 2,
        entries: 2,
        max_sequence: 3,
        min_sequence: 1,
        owner_agent_user_id: "owner-a",
      },
    ])
    expect(gapped.gappedOwners).toBe(1)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "agent runtime remainder backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_agent_runtime_remainder_backfill")
      await admin.end()
      const url = pg.urlFor("khala_agent_runtime_remainder_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0012_agent_runtime_remainder.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("state tables converge and event ledger density remains exact", async () => {
      const profile = profileRow("agent-user-1")
      const credential = credentialRow("credential-1", {
        openauth_user_id: "owner-user-1",
      })
      const events = [
        eventLedgerRow("owner-1", 1),
        eventLedgerRow("owner-1", 2),
        eventLedgerRow("owner-1", 3),
      ]

      expect(
        await upsertAgentRuntimeRemainderRows(sql, "agent_profiles", [profile]),
      ).toBe(1)
      expect(
        await upsertAgentRuntimeRemainderRows(sql, "agent_credentials", [
          credential,
        ]),
      ).toBe(1)
      expect(
        await upsertAgentRuntimeRemainderRows(sql, "event_ledger_entries", events),
      ).toBe(3)

      expect(
        await upsertAgentRuntimeRemainderRows(sql, "agent_profiles", [
          { ...profile, metadata_json: "{\"updated\":true}" },
        ]),
      ).toBe(1)
      expect(await postgresAgentRuntimeRemainderRowCount(sql, "agent_profiles")).toBe(
        1,
      )
      expect(
        await postgresAgentRuntimeRemainderRowCount(sql, "agent_credentials"),
      ).toBe(1)
      expect(
        await postgresAgentRuntimeRemainderRowCount(sql, "event_ledger_entries"),
      ).toBe(3)

      const density = await postgresEventLedgerOrderingDensity(sql)
      expect(density.totalEntries).toBe(3)
      expect(density.gappedOwners).toBe(0)
      expect(
        await postgresAgentRuntimeRemainderNewestHashes(
          sql,
          "agent_credentials",
          1,
        ),
      ).toEqual(
        d1AgentRuntimeRemainderNewestHashes("agent_credentials", [credential]),
      )
    })
  },
)
