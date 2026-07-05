// `bun:sqlite` adapter over `node:sqlite` for this Worker's vitest suites.
//
// The Khala Sync client's desktop store
// (`@openagentsinc/khala-sync-client/sqlite-store`) runs on `bun:sqlite`.
// The KS-4.4 stitch-seam test drives that REAL store inside this package's
// vitest (Node) runtime, so `vitest.config.ts` aliases `bun:sqlite` to this
// module — the same test-shim idiom as
// `src/test/cloudflare-workers.ts`. Only the surface the client store
// actually uses is implemented: `Database` construction, `exec`, prepared
// `query` handles with `get`/`all`/`run`, callable `transaction` wrappers
// (BEGIN/COMMIT/ROLLBACK — the store does not nest), and `close`.
//
// Semantics matched to bun:sqlite where they differ from node:sqlite:
// `Statement.get` returns `null` (not `undefined`) when no row matches.

import { DatabaseSync } from 'node:sqlite'

type SqlParam = null | number | bigint | string | Uint8Array

const asParams = (params: ReadonlyArray<unknown>): Array<SqlParam> =>
  params.map(value => {
    if (
      value === null ||
      typeof value === 'number' ||
      typeof value === 'bigint' ||
      typeof value === 'string' ||
      value instanceof Uint8Array
    ) {
      return value
    }
    throw new TypeError(
      `unsupported sqlite parameter of type ${typeof value} in bun:sqlite test adapter`,
    )
  })

export interface Statement<
  Row = unknown,
  Params extends ReadonlyArray<unknown> = ReadonlyArray<unknown>,
> {
  get(...params: Params): Row | null
  all(...params: Params): Array<Row>
  run(...params: ReadonlyArray<unknown>): void
}

export class Database {
  private readonly db: DatabaseSync

  constructor(filename: string, _options?: { create?: boolean }) {
    this.db = new DatabaseSync(filename)
  }

  exec(sql: string): void {
    this.db.exec(sql)
  }

  query<
    Row = unknown,
    Params extends ReadonlyArray<unknown> = ReadonlyArray<unknown>,
  >(sql: string): Statement<Row, Params> {
    const statement = this.db.prepare(sql)
    return {
      all: (...params) =>
        statement.all(...asParams(params)) as unknown as Array<Row>,
      get: (...params) => {
        const row = statement.get(...asParams(params))
        return row === undefined ? null : (row as unknown as Row)
      },
      run: (...params) => {
        statement.run(...asParams(params))
      },
    }
  }

  transaction<Args extends ReadonlyArray<unknown>, R>(
    fn: (...args: Args) => R,
  ): (...args: Args) => R {
    return (...args: Args): R => {
      this.db.exec('BEGIN')
      try {
        const result = fn(...args)
        this.db.exec('COMMIT')
        return result
      } catch (error) {
        try {
          this.db.exec('ROLLBACK')
        } catch {
          // the transaction may already have aborted; the throw below wins
        }
        throw error
      }
    }
  }

  close(): void {
    this.db.close()
  }
}
