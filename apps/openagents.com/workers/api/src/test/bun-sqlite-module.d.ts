// Ambient `bun:sqlite` module declaration for THIS Worker's typecheck only.
//
// `@openagentsinc/khala-sync-client` (a Bun package) imports `bun:sqlite`;
// this Worker's vitest suites drive that real client store through the
// `node:sqlite`-backed adapter in `src/test/bun-sqlite.ts` (aliased in
// `vitest.config.ts`). The Worker typechecks against
// `@cloudflare/workers-types` — no `bun-types` ambients (they collide with
// the workers globals) — so the module is declared here with exactly the
// structural surface the client store uses. The declaration mirrors the
// adapter's types; the adapter itself is real code and stays in sync by
// construction (both are reviewed together).

declare module 'bun:sqlite' {
  export interface Statement<
    Row = unknown,
    Params extends ReadonlyArray<unknown> = ReadonlyArray<unknown>,
  > {
    get(...params: Params): Row | null
    all(...params: Params): Array<Row>
    run(...params: ReadonlyArray<unknown>): void
  }

  export class Database {
    constructor(filename: string, options?: { create?: boolean })
    exec(sql: string): void
    query<
      Row = unknown,
      Params extends ReadonlyArray<unknown> = ReadonlyArray<unknown>,
    >(sql: string): Statement<Row, Params>
    transaction<Args extends ReadonlyArray<unknown>, R>(
      fn: (...args: Args) => R,
    ): (...args: Args) => R
    close(): void
  }
}
