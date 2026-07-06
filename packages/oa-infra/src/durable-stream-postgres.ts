/**
 * Postgres DurableStream backend — Bun SQL Layer
 * (migrations/0003_oa_infra_durable_stream.sql).
 *
 * The driver-agnostic SQL core lives in ./durable-stream-postgres-core.ts
 * (no Bun types) so the openagents.com Worker can run the SAME backend over
 * postgres.js through its KHALA_SYNC_DB Hyperdrive binding (CFG-6 #8521).
 * This module keeps the Bun-native `OaInfraSql` Layer.
 */
import { Effect, Layer } from "effect"
import { DurableStream } from "./durable-stream.ts"
import {
  type DurableStreamSqlClient,
  makePostgresDurableStream,
} from "./durable-stream-postgres-core.ts"
import { OaInfraSql } from "./sql.ts"

export {
  type DurableStreamSqlClient,
  type DurableStreamSqlTx,
  deleteExpiredStreams,
  makePostgresDurableStream,
} from "./durable-stream-postgres-core.ts"

/** Postgres DurableStream Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<DurableStream, never, OaInfraSql> = Layer.effect(
  DurableStream,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    // Bun's SQL exposes the same tagged-template + `begin` surface the core
    // needs; the cast is the single deliberate driver seam (the conformance
    // runs against this Layer prove equivalence).
    return makePostgresDurableStream(sql as unknown as DurableStreamSqlClient)
  }),
)
