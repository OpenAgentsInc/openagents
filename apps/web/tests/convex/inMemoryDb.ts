type OrderDirection = "asc" | "desc";

type Constraint = {
  readonly field: string;
  readonly value: unknown;
};

type AnyDoc = Record<string, any> & { _id: string };

export type InMemoryDb = {
  readonly query: (table: string) => InMemoryQuery;
  readonly insert: (table: string, doc: Record<string, any>) => Promise<string>;
  readonly patch: (id: string, patch: Record<string, any>) => Promise<void>;
  readonly delete: (id: string) => Promise<void>;
  /** Clears all tables and resets internal id counters. */
  readonly reset: () => void;
  /** Test-only table access. */
  readonly __tables: Record<string, AnyDoc[]>;
};

class EqBuilder {
  constructor(private onEq: (field: string, value: unknown) => void) {}

  eq(field: string, value: unknown): EqBuilder {
    this.onEq(field, value);
    return this;
  }
}

export class InMemoryQuery {
  private readonly constraints: Array<Constraint> = [];
  private orderDirection: OrderDirection | null = null;

  constructor(
    private readonly tables: Record<string, AnyDoc[]>,
    private readonly table: string,
  ) {}

  withIndex(_index: string, f: (q: EqBuilder) => unknown): InMemoryQuery {
    f(
      new EqBuilder((field, value) => {
        this.constraints.push({ field, value });
      }),
    );
    return this;
  }

  order(direction: OrderDirection): InMemoryQuery {
    this.orderDirection = direction;
    return this;
  }

  async take(n: number): Promise<AnyDoc[]> {
    return this.exec().slice(0, Math.max(0, n));
  }

  async collect(): Promise<AnyDoc[]> {
    return this.exec();
  }

  async first(): Promise<AnyDoc | null> {
    return this.exec()[0] ?? null;
  }

  async unique(): Promise<AnyDoc | null> {
    const rows = this.exec();
    if (rows.length === 0) return null;
    if (rows.length > 1) {
      throw new Error(`Expected unique result for table=${this.table}, got ${rows.length}`);
    }
    return rows[0] ?? null;
  }

  private exec(): AnyDoc[] {
    const rows = [...(this.tables[this.table] ?? [])];
    const filtered = rows.filter((row) =>
      this.constraints.every((c) => Object.is((row as any)[c.field], c.value)),
    );

    if (!this.orderDirection) return filtered;

    const sortKey = (row: AnyDoc): number => {
      const created = (row as any).createdAtMs;
      const updated = (row as any).updatedAtMs;
      if (typeof created === "number" && Number.isFinite(created)) return created;
      if (typeof updated === "number" && Number.isFinite(updated)) return updated;
      return 0;
    };

    filtered.sort((a, b) => {
      const diff = sortKey(a) - sortKey(b);
      if (diff !== 0) return diff;
      return a._id.localeCompare(b._id);
    });

    if (this.orderDirection === "desc") filtered.reverse();
    return filtered;
  }
}

export const makeInMemoryDb = (options?: { readonly seedTables?: Record<string, AnyDoc[]> }): InMemoryDb => {
  const tables: Record<string, AnyDoc[]> = {
    users: [],
    threads: [],
    blueprints: [],
    autopilotFeatureRequests: [],
    messages: [],
    messageParts: [],
    runs: [],
    receipts: [],
    dseArtifacts: [],
    dseActiveArtifacts: [],
    dseActiveArtifactHistory: [],
    dseExamples: [],
    dseCompileReports: [],
    dseEvalReports: [],
    dseCanaries: [],
    dseCanaryHistory: [],
    dseOpsRuns: [],
    dseOpsRunEvents: [],
    lightningTasks: [],
    lightningTaskEvents: [],
    l402Paywalls: [],
    l402PaywallPolicies: [],
    l402PaywallRoutes: [],
    l402GatewayDeployments: [],
    l402GatewayEvents: [],
    l402Invoices: [],
    l402Settlements: [],
    l402SecurityGlobal: [],
    l402OwnerSecurityControls: [],
    l402CredentialRoles: [],
    l402Payouts: [],
    ...(options?.seedTables ?? {}),
  };

  let nextId = 1;
  const allocId = (table: string) => `${table}:${nextId++}`;

  const insert = async (table: string, doc: Record<string, any>): Promise<string> => {
    const id = allocId(table);
    const row: AnyDoc = { ...doc, _id: id };
    if (!tables[table]) tables[table] = [];
    tables[table].push(row);
    return id;
  };

  const patch = async (id: string, patchDoc: Record<string, any>): Promise<void> => {
    for (const [table, rows] of Object.entries(tables)) {
      const idx = rows.findIndex((r) => r._id === id);
      if (idx < 0) continue;
      rows[idx] = { ...rows[idx], ...patchDoc };
      tables[table] = rows;
      return;
    }
    throw new Error(`patch: missing id ${id}`);
  };

  const del = async (id: string): Promise<void> => {
    for (const rows of Object.values(tables)) {
      const idx = rows.findIndex((r) => r._id === id);
      if (idx < 0) continue;
      rows.splice(idx, 1);
      return;
    }
    throw new Error(`delete: missing id ${id}`);
  };

  return {
    query: (table: string) => new InMemoryQuery(tables, table),
    insert,
    patch,
    delete: del,
    reset: () => {
      for (const [k, v] of Object.entries(tables)) {
        if (Array.isArray(v)) tables[k] = [];
      }
      nextId = 1;
    },
    __tables: tables,
  };
};
