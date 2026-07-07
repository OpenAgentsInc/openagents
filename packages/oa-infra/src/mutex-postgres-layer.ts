/**
 * Bun-SQL Layer for the Postgres Mutex backend.
 *
 * Split from mutex-postgres.ts (CFG-17, issue #8533; mirrors
 * kv-store-postgres-layer.ts and durable-stream-postgres.ts) so that module
 * stays importable under non-Bun type environments — the Cloud Run monolith
 * / `openagents.com` Worker seam reuses `makePostgresMutex` over its
 * postgres.js client. This file is the only place the Mutex backend touches
 * `OaInfraSql`/Bun.
 */
import { Effect, Layer } from "effect"
import { Mutex } from "./mutex.ts"
import { makePostgresMutex, type MutexSqlClient } from "./mutex-postgres.ts"
import { OaInfraSql } from "./sql.ts"

/** Postgres Mutex Layer; requires `OaInfraSql` (see src/sql.ts). */
export const layerPostgres: Layer.Layer<Mutex, never, OaInfraSql> = Layer.effect(
  Mutex,
  Effect.gen(function* () {
    const { sql } = yield* OaInfraSql
    // Bun's SQL exposes the same `reserve()` surface the core needs; the cast
    // is the single deliberate driver seam.
    return makePostgresMutex(sql as unknown as MutexSqlClient)
  }),
)
