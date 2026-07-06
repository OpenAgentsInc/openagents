/**
 * Bun-SQL Layer for the Postgres KvStore backend.
 *
 * Split from kv-store-postgres.ts (CFG-3, issue #8518) so that module stays
 * importable under non-Bun type environments (the `openagents.com` Worker
 * reuses `makePostgresKvStore` over its postgres.js client); this file is
 * the only place the KvStore backend touches `OaInfraSql`/Bun.
 */
import { Effect, Layer } from "effect"
import { KvStore } from "./kv-store.ts"
import { makePostgresKvStore } from "./kv-store-postgres.ts"
import { OaInfraSql } from "./sql.ts"

/** Postgres KvStore Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<KvStore, never, OaInfraSql> = Layer.effect(
  KvStore,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    return makePostgresKvStore(sql)
  }),
)
