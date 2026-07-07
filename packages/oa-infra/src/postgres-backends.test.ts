/**
 * Conformance runs for the Postgres backends (CFG-2, issue #8517), plus
 * Postgres-specific behavior (lazy KV expiry, migration idempotency,
 * SKIP LOCKED concurrency).
 *
 * Skips cleanly on machines without local Postgres binaries — the same
 * `hasLocalPostgres()` gate as packages/khala-sync-server. One throwaway
 * server + one migrated database host all suites; conformance tests
 * namespace their keys/topics/streams/locks so they never interfere.
 */
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import { Effect, Layer } from "effect"
import postgres from "postgres"
import {
  hasLocalPostgres,
  startLocalPostgres,
} from "@openagentsinc/khala-sync-server/test/local-postgres"
import type { LocalPostgres } from "@openagentsinc/khala-sync-server/test/local-postgres"
import { runBlobStoreConformance } from "./conformance/blob-store.ts"
import { runDurableStreamConformance } from "./conformance/durable-stream.ts"
import { runJobQueueConformance } from "./conformance/job-queue.ts"
import { runKvStoreConformance } from "./conformance/kv-store.ts"
import { runMutexConformance } from "./conformance/mutex.ts"
import { DurableStream } from "./durable-stream.ts"
import * as DurableStreamPostgres from "./durable-stream-postgres.ts"
import * as JobQueuePostgres from "./job-queue-postgres.ts"
import { KvStore } from "./kv-store.ts"
import * as KvStorePostgresLayer from "./kv-store-postgres-layer.ts"
import { Mutex } from "./mutex.ts"
import { runOaInfraMigrations } from "./migrate.ts"
import * as MutexPostgres from "./mutex-postgres.ts"
import * as MutexPostgresLayer from "./mutex-postgres-layer.ts"
import * as BlobStoreGcs from "./blob-store-gcs.ts"
import { OaInfraSql } from "./sql.ts"

setDefaultTimeout(240_000)

const pgAvailable = hasLocalPostgres()

let pg: LocalPostgres
let sql: SQL
let pgJsSql: ReturnType<typeof postgres>

if (pgAvailable) {
  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE oa_infra_conformance")
    await admin.end()
    const url = pg.urlFor("oa_infra_conformance")
    const result = await runOaInfraMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_oa_infra_kv.sql")
    expect(result.applied).toContain("0002_oa_infra_job_queue.sql")
    expect(result.applied).toContain("0003_oa_infra_durable_stream.sql")
    sql = new SQL({ url, max: 10 })
    // postgres.js client with the openagents.com Worker's Hyperdrive
    // transaction-mode discipline (`prepare: false`; see the KHALA_SYNC_DB
    // comment in that app's wrangler.jsonc) — proves the driver seam the
    // Worker uses for the DurableStream backend (CFG-6 #8521).
    pgJsSql = postgres(url, { max: 5, prepare: false })
  })

  afterAll(async () => {
    await sql?.end()
    await pgJsSql?.end({ timeout: 5 })
    await pg?.stop()
  })
}

const sqlLayer = () => OaInfraSql.fromSql(sql)

runKvStoreConformance({
  label: "postgres",
  skip: !pgAvailable,
  makeLayer: () => Layer.provide(KvStorePostgresLayer.layerPostgres, sqlLayer()),
})
runJobQueueConformance({
  label: "postgres",
  skip: !pgAvailable,
  makeLayer: () => Layer.provide(JobQueuePostgres.layerPostgres, sqlLayer()),
})
runDurableStreamConformance({
  label: "postgres",
  skip: !pgAvailable,
  makeLayer: () => Layer.provide(DurableStreamPostgres.layerPostgres, sqlLayer()),
})
// Same backend, postgres.js driver — the workerd-compatible client the
// openagents.com Worker uses through Hyperdrive (CFG-6 #8521). Both drivers
// must pass the identical suite for the driver seam to be a config-time swap.
runDurableStreamConformance({
  label: "postgres.js",
  skip: !pgAvailable,
  makeLayer: () =>
    Layer.sync(DurableStream, () =>
      DurableStreamPostgres.makePostgresDurableStream(
        pgJsSql as unknown as DurableStreamPostgres.DurableStreamSqlClient,
      ),
    ),
})
runMutexConformance({
  label: "postgres",
  skip: !pgAvailable,
  makeLayer: () => Layer.provide(MutexPostgresLayer.layerPostgres, sqlLayer()),
})
// Same backend, postgres.js driver — the client the Cloud Run monolith /
// openagents.com Worker uses over KHALA_SYNC_DB (CFG-17 #8533). Both drivers
// must pass the identical suite for the `reserve()` driver seam to be a
// config-time swap.
runMutexConformance({
  label: "postgres.js",
  skip: !pgAvailable,
  makeLayer: () =>
    Layer.sync(Mutex, () =>
      MutexPostgres.makePostgresMutex(
        pgJsSql as unknown as MutexPostgres.MutexSqlClient,
      ),
    ),
})

// The GCS BlobStore cannot run against local Postgres; exercise the
// conformance suite against a real bucket only when one is configured
// (OA_INFRA_GCS_TEST_BUCKET + ambient GCP credentials). CI without cloud
// access skips, same pattern as the Postgres gate.
const gcsTestBucket = process.env["OA_INFRA_GCS_TEST_BUCKET"]
runBlobStoreConformance({
  label: "gcs",
  skip: gcsTestBucket === undefined || gcsTestBucket === "",
  makeLayer: () => {
    process.env["OA_INFRA_GCS_BUCKET"] = gcsTestBucket as string
    process.env["OA_INFRA_GCS_PREFIX"] = `oa-infra-conformance/${crypto.randomUUID()}/`
    return Layer.orDie(BlobStoreGcs.layerGcs)
  },
})

describe.skipIf(!pgAvailable)("Postgres-specific behavior", () => {
  test("migrations are idempotent (second run applies nothing)", async () => {
    const url = pg.urlFor("oa_infra_conformance")
    const second = await runOaInfraMigrations({ databaseUrl: url })
    expect(second.applied).toEqual([])
    expect(second.alreadyApplied).toEqual([
      "0001_oa_infra_kv.sql",
      "0002_oa_infra_job_queue.sql",
      "0003_oa_infra_durable_stream.sql",
      "0004_oa_infra_kv_key_prefix.sql",
    ])
  })

  test("KV lazy expiry physically deletes the expired row on read", async () => {
    const key = `pg-lazy-expiry-${crypto.randomUUID()}`
    const layer = Layer.provide(KvStorePostgresLayer.layerPostgres, sqlLayer())
    await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const kv = yield* KvStore
          yield* kv.put(key, "soon gone", { ttlMs: 100 })
        }),
        layer,
      ),
    )
    await new Promise((resolve) => setTimeout(resolve, 250))
    // Row still physically present before any read touches it.
    const beforeRead: Array<{ n: number }> = await sql`
      SELECT count(*)::int AS n FROM oa_infra_kv WHERE key = ${key}
    `
    expect(beforeRead[0]?.n).toBe(1)
    const value = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const kv = yield* KvStore
          return yield* kv.get(key)
        }),
        layer,
      ),
    )
    expect(value).toBeNull()
    const afterRead: Array<{ n: number }> = await sql`
      SELECT count(*)::int AS n FROM oa_infra_kv WHERE key = ${key}
    `
    expect(afterRead[0]?.n).toBe(0)
  })

  test("concurrent lessees never double-claim (FOR UPDATE SKIP LOCKED)", async () => {
    const topic = `pg-skip-locked-${crypto.randomUUID()}`
    const queue = JobQueuePostgres.makePostgresJobQueue(sql)
    const total = 20
    for (let index = 0; index < total; index++) {
      await Effect.runPromise(queue.enqueue(topic, `job-${index}`))
    }
    // Many concurrent lessees racing on the same topic.
    const claims = await Promise.all(
      Array.from({ length: 8 }, () =>
        Effect.runPromise(queue.lease(topic, { batch: 5 })),
      ),
    )
    const claimed = claims.flat().map((job) => job.id)
    expect(claimed.length).toBe(total)
    expect(new Set(claimed).size).toBe(total)
  })
})
