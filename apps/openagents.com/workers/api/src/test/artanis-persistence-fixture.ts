type PersistenceRow = Readonly<{
  active: number
  action_json?: string | undefined
  agent_id: string
  approval_gate_ref?: string | null | undefined
  closed_at: string | null
  closeout_json: string | null
  context_json?: string | undefined
  content_hash: string
  created_at: string
  health_snapshot_ref?: string | null | undefined
  id: string
  idempotency_key: string
  parent_ref: string | null
  public_projection_json: string
  record_json: string
  record_ref: string
  scope_ref: string | null
  source_kind: string
  state: string
  updated_at: string
}>

export class ArtanisPersistenceTestStore {
  readonly tables = new Map<string, Array<PersistenceRow>>()

  rows(table: string): Array<PersistenceRow> {
    const existing = this.tables.get(table)

    if (existing !== undefined) {
      return existing
    }

    const rows: Array<PersistenceRow> = []
    this.tables.set(table, rows)

    return rows
  }
}

class ArtanisPersistenceTestStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ArtanisPersistenceTestStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(colName?: string): Promise<T | null> {
    void colName

    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        rows.find(item => item.idempotency_key === idempotencyKey) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE record_ref = ?')) {
      const recordRef = String(this.values[0])
      const row = rows.find(item => item.record_ref === recordRef) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first query: ${this.query}`))
  }

  raw<T = unknown[]>(options: { columnNames: true }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(
    options?: { columnNames?: boolean },
  ): Promise<T[] | [string[], ...T[]]> {
    return options?.columnNames === true
      ? Promise.resolve([[]] as [string[], ...T[]])
      : Promise.resolve([] as T[])
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('INSERT INTO')) {
      if (table === 'artanis_fleet_overseer_decisions') {
        rows.push({
          action_json: String(this.values[2]),
          active: 0,
          agent_id: 'agent_artanis',
          approval_gate_ref:
            this.values[4] === null ? null : String(this.values[4]),
          closed_at: null,
          closeout_json: null,
          context_json: String(this.values[3]),
          content_hash: '',
          created_at: String(this.values[6]),
          health_snapshot_ref:
            this.values[5] === null ? null : String(this.values[5]),
          id: String(this.values[0]),
          idempotency_key: String(this.values[0]),
          parent_ref: null,
          public_projection_json: '{}',
          record_json: String(this.values[2]),
          record_ref: String(this.values[0]),
          scope_ref: null,
          source_kind: 'artanis_fleet_overseer',
          state: String(this.values[1]),
          updated_at: String(this.values[6]),
        })

        return Promise.resolve({ success: true } as D1Result<T>)
      }

      if (
        rows.every(
          row =>
            row.record_ref !== String(this.values[2]) &&
            row.idempotency_key !== String(this.values[3]),
        )
      ) {
        rows.push({
          active: Number(this.values[5]),
          agent_id: String(this.values[1]),
          closed_at: this.values[15] === null ? null : String(this.values[15]),
          closeout_json:
            this.values[12] === null ? null : String(this.values[12]),
          content_hash: String(this.values[11]),
          created_at: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: String(this.values[3]),
          parent_ref: this.values[8] === null ? null : String(this.values[8]),
          public_projection_json: String(this.values[10]),
          record_json: String(this.values[9]),
          record_ref: String(this.values[2]),
          scope_ref: this.values[7] === null ? null : String(this.values[7]),
          source_kind: String(this.values[6]),
          state: String(this.values[4]),
          updated_at: String(this.values[14]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE artanis_loop_ticks')) {
      // Bind order mirrors closeArtanisPersistedLoopTick:
      //   state, closeout_json, record_json, public_projection_json,
      //   content_hash, updated_at, closed_at, record_ref (WHERE).
      const recordRef = String(this.values[7])
      const index = rows.findIndex(
        row => row.record_ref === recordRef && row.closed_at === null,
      )

      if (index !== -1) {
        const existing = rows[index]!
        rows[index] = {
          ...existing,
          closed_at: String(this.values[6]),
          closeout_json: String(this.values[1]),
          content_hash: String(this.values[4]),
          public_projection_json: String(this.values[3]),
          record_json: String(this.values[2]),
          state: String(this.values[0]),
          updated_at: String(this.values[5]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE artanis_forum_publication_intents')) {
      const recordRef = String(this.values[6])
      const index = rows.findIndex(row => row.record_ref === recordRef)

      if (index !== -1) {
        const existing = rows[index]!
        rows[index] = {
          ...existing,
          active: Number(this.values[1]),
          content_hash: String(this.values[4]),
          public_projection_json: String(this.values[3]),
          record_json: String(this.values[2]),
          state: String(this.values[0]),
          updated_at: String(this.values[5]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run query: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const table = tableName(this.query)
    const rows = this.store.rows(table)

    if (this.query.includes('ORDER BY updated_at DESC')) {
      const limit = Number(this.values[0])
      const sorted = [...rows].sort((left, right) =>
        right.updated_at.localeCompare(left.updated_at),
      )

      return Promise.resolve({
        results: sorted.slice(0, Number.isFinite(limit) ? limit : sorted.length),
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }
}

const tableName = (query: string): string => {
  const match =
    /\bFROM\s+([a-z_]+)\b/i.exec(query) ??
    /\bINTO\s+([a-z_]+)\b/i.exec(query) ??
    /\bUPDATE\s+([a-z_]+)\b/i.exec(query)

  if (match === null) {
    throw new Error(`No table name found in query: ${query}`)
  }

  return match[1]!
}

export const artanisPersistenceTestDb = (
  store: ArtanisPersistenceTestStore,
): D1Database =>
  ({
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run())) as Promise<
        Array<D1Result<T>>
      >,
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) =>
      new ArtanisPersistenceTestStatement(query, store),
    withSession: () => artanisPersistenceTestDb(store),
  }) as unknown as D1Database
