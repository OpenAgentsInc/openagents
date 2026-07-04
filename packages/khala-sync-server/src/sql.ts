/**
 * Structural SQL driver seam for the Khala Sync server substrate.
 *
 * The substrate runs in TWO runtimes (SPEC §1/§4): under Bun for tests,
 * migrations, and the capture worker (Bun's `SQL`), and inside the
 * `openagents.com` Worker via Hyperdrive, where Bun does not exist and the
 * driver is postgres.js. Both drivers expose the same postgres.js-shaped
 * surface — a tagged-template query function plus `begin` for transactions —
 * so this module types that shared shape STRUCTURALLY instead of importing
 * `bun` types, keeping the package consumable from workerd.
 *
 * Only the subset the substrate actually uses is typed here: tagged-template
 * parameterized queries and callback-scoped transactions. Anything else
 * (cursors, LISTEN, COPY, session state) is deliberately absent — the
 * Hyperdrive request path forbids it (SPEC §4).
 */

/**
 * A tagged-template SQL query handle: `` sql`SELECT … ${param}` `` runs ONE
 * parameterized statement and resolves with the result rows. Both Bun's
 * `SQL`/`TransactionSQL` and postgres.js `Sql`/`TransactionSql` satisfy this.
 *
 * The resolved row type is intentionally `any`: both drivers return
 * driver-specific thenable query objects, and call sites annotate the row
 * shape they expect (the same discipline the substrate already used with
 * Bun's types).
 */
export interface SqlTag {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>): Promise<any>
}

/** The SQL handle scoped to one open transaction. */
export type SyncTransactionSql = SqlTag

/**
 * A root SQL handle that can open callback-scoped transactions:
 * `sql.begin(fn)` (optionally `sql.begin(options, fn)`, e.g.
 * `"isolation level repeatable read"`) runs `fn` inside BEGIN…COMMIT and
 * rolls back when `fn` rejects. Matches Bun `SQL` and postgres.js `Sql`
 * structurally.
 */
export interface SyncSql extends SqlTag {
  readonly begin: {
    <A>(fn: (tx: SyncTransactionSql) => Promise<A>): Promise<A>
    <A>(
      options: string,
      fn: (tx: SyncTransactionSql) => Promise<A>,
    ): Promise<A>
  }
}
