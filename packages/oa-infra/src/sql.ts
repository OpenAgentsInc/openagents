/**
 * Shared Postgres access for the oa-infra Postgres backends (CFG-2,
 * issue #8517).
 *
 * Uses Bun's built-in `SQL` client over a DIRECT connection URL — the same
 * client approach as packages/khala-sync-server (see its migrate.ts). All
 * oa-infra Postgres Layers depend on this one service so a single pool is
 * shared across primitives (Layer memoization keeps it single).
 */
import { SQL } from "bun"
import { Config, Context, Effect, Layer } from "effect"

export interface OaInfraSqlShape {
  readonly sql: SQL
}

export class OaInfraSql extends Context.Service<OaInfraSql, OaInfraSqlShape>()(
  "@openagentsinc/oa-infra/OaInfraSql",
) {
  /** Wrap an existing Bun SQL pool (tests, embedding apps). */
  static readonly fromSql = (sql: SQL): Layer.Layer<OaInfraSql> =>
    Layer.succeed(OaInfraSql, { sql })

  /**
   * Build the pool from config: `OA_INFRA_DATABASE_URL` (direct Postgres
   * URL, never a vendor pooler binding) and optional
   * `OA_INFRA_DATABASE_POOL_MAX` (default 10). The pool is closed when the
   * layer's scope closes.
   */
  static readonly layerConfig: Layer.Layer<OaInfraSql, Config.ConfigError> = Layer.effect(
    OaInfraSql,
    Effect.gen(function* () {
      const url = yield* Config.string("OA_INFRA_DATABASE_URL")
      const max = yield* Config.schema(Config.Port, "OA_INFRA_DATABASE_POOL_MAX").pipe(
        Config.withDefault(10),
      )
      const sql = yield* Effect.acquireRelease(
        Effect.sync(() => new SQL({ url, max })),
        (pool) => Effect.promise(() => pool.end()),
      )
      return { sql }
    }),
  )
}
