/**
 * @openagentsinc/oa-infra — owned, hot-swappable infrastructure primitives
 * (CFG-2, issue #8517; Cloudflare→GCP consolidation audit §5, epic #8515).
 *
 * Doctrine: app code depends on THESE Effect service interfaces, never a
 * vendor SDK. Every backend is a config-time Layer swap, and every backend
 * must pass the conformance suite for its interface
 * (src/conformance/*) — that suite IS the hot-swap guarantee.
 *
 * | Interface       | Backends here             | Swap targets (audit)    |
 * |-----------------|---------------------------|-------------------------|
 * | `KvStore`       | memory, Postgres          | Memorystore/Redis       |
 * | `BlobStore`     | memory, GCS               | S3, MinIO on SHC        |
 * | `JobQueue`      | memory, PG SKIP LOCKED    | Pub/Sub, SQS, NATS      |
 * | `DurableStream` | memory, PG append table   | Redis Streams, Kafka    |
 * | `Mutex`         | memory, PG advisory locks | Redis locks             |
 * | `Scheduler`     | pure table + HTTP handler | any cron + HTTP tick    |
 *
 * Postgres backends share one `OaInfraSql` service (src/sql.ts) and are
 * installed by the migration runner (src/migrate.ts, migrations/).
 */

// Contracts
export * from "./kv-store.ts"
export * from "./blob-store.ts"
export * from "./job-queue.ts"
export * from "./durable-stream.ts"
export * from "./mutex.ts"
export * from "./scheduler.ts"
export * from "./cron.ts"

// Shared Postgres access + migrations
export * from "./sql.ts"
export {
  defaultMigrationsDir,
  runOaInfraMigrations,
  type RunOaInfraMigrationsOptions,
  type RunOaInfraMigrationsResult,
} from "./migrate.ts"

// Backends (namespaced: two Layers per interface would collide otherwise)
export * as KvStoreMemory from "./kv-store-memory.ts"
export * as KvStorePostgres from "./kv-store-postgres.ts"
export * as BlobStoreMemory from "./blob-store-memory.ts"
export * as BlobStoreGcs from "./blob-store-gcs.ts"
// Workerd/Bun-portable HMAC-interop backend (no SDK). Also importable via
// the SIDE-EFFECT-FREE subpath `@openagentsinc/oa-infra/blob-store-gcs-hmac`
// — use the subpath from Workers bundles so the SDK-backed sibling above
// never enters the graph.
export * as BlobStoreGcsHmac from "./blob-store-gcs-hmac.ts"
export * as JobQueueMemory from "./job-queue-memory.ts"
export * as JobQueuePostgres from "./job-queue-postgres.ts"
export * as DurableStreamMemory from "./durable-stream-memory.ts"
export * as DurableStreamPostgres from "./durable-stream-postgres.ts"
export * as MutexMemory from "./mutex-memory.ts"
export * as MutexPostgres from "./mutex-postgres.ts"
