import { createRequire } from "node:module"

export type SqliteTestBinding = string | number | bigint | boolean | null | Uint8Array

type Bindings = ReadonlyArray<SqliteTestBinding> | Readonly<Record<string, SqliteTestBinding>>

interface StatementSync {
  run(...params: ReadonlyArray<unknown>): { readonly changes: number | bigint; readonly lastInsertRowid: number | bigint }
  get(...params: ReadonlyArray<unknown>): unknown
  all(...params: ReadonlyArray<unknown>): ReadonlyArray<unknown>
}

interface DatabaseSync {
  exec(sql: string): void
  prepare(sql: string): StatementSync
  close(): void
}

interface NodeSqlite {
  DatabaseSync: new (path: string, options?: { readonly readOnly?: boolean }) => DatabaseSync
}

const requireModule = createRequire(import.meta.url)

const normalize = (value: SqliteTestBinding): Exclude<SqliteTestBinding, boolean> =>
  typeof value === "boolean" ? (value ? 1 : 0) : value

const normalizeParams = (params: ReadonlyArray<unknown>): ReadonlyArray<unknown> =>
  params.map((value) => {
    if (value !== null && typeof value === "object" && !ArrayBuffer.isView(value)) {
      return Object.fromEntries(
        Object.entries(value).map(([key, binding]) => [key, normalize(binding as SqliteTestBinding)]),
      )
    }
    return normalize(value as SqliteTestBinding)
  })

/** Stock-Node SQLite harness used by retained VP-3 suites. */
export class NodeTestDatabase {
  readonly #database: DatabaseSync
  readonly #statements = new Map<string, StatementSync>()
  #transactionDepth = 0

  constructor(path: string, options: { readonly create?: boolean; readonly readonly?: boolean } = {}) {
    const { DatabaseSync } = requireModule("node:sqlite") as NodeSqlite
    this.#database = new DatabaseSync(path, { readOnly: options.readonly ?? false })
  }

  exec(sql: string): void {
    this.#database.exec(sql)
  }

  query<Row = Record<string, unknown>, Params extends Array<SqliteTestBinding> = Array<SqliteTestBinding>>(
    sql: string,
  ) {
    const statement = this.#statements.get(sql) ?? this.#database.prepare(sql)
    this.#statements.set(sql, statement)
    return {
      run: (...params: Params) => statement.run(...normalizeParams(params)),
      get: (...params: Params) => (statement.get(...normalizeParams(params)) as Row | undefined) ?? null,
      all: (...params: Params) => [...statement.all(...normalizeParams(params))] as Array<Row>,
    }
  }

  transaction<A extends ReadonlyArray<unknown>, B>(body: (...args: A) => B): (...args: A) => B {
    return (...args) => {
      const depth = this.#transactionDepth
      const savepoint = `vp3_test_${depth}`
      this.#database.exec(depth === 0 ? "BEGIN" : `SAVEPOINT ${savepoint}`)
      this.#transactionDepth = depth + 1
      try {
        const result = body(...args)
        this.#transactionDepth = depth
        this.#database.exec(depth === 0 ? "COMMIT" : `RELEASE SAVEPOINT ${savepoint}`)
        return result
      } catch (error) {
        this.#transactionDepth = depth
        this.#database.exec(
          depth === 0
            ? "ROLLBACK"
            : `ROLLBACK TO SAVEPOINT ${savepoint}; RELEASE SAVEPOINT ${savepoint}`,
        )
        throw error
      }
    }
  }

  close(): void {
    this.#statements.clear()
    this.#database.close()
  }
}
