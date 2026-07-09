// Minimal ambient bridge for Cloudflare Worker runtime globals.
//
// The Start app is a Cloud Run / Bun server and deliberately does NOT depend on
// the `@cloudflare/workers-types` package (removed from this app's tsconfig
// `types` during the GCP evacuation — see CLAUDE.md "Cloudflare exit → GCP
// evacuation"). However, Start's server entry transitively imports shared
// helpers from `@openagentsinc/sync-worker` and `workers/api`, which are still
// Cloudflare Worker code that references these ambient runtime globals and
// carry their own `@cloudflare/workers-types` tsconfigs.
//
// Rather than re-add the whole workers-types package to the Cloud Run app, we
// declare only the handful of global names that leak into Start's program, with
// just enough shape to typecheck the shared worker source Start pulls in. Start
// itself never runs these Cloudflare surfaces.
//
// Remove this file once `@openagentsinc/sync-worker` / `workers/api` finish
// migrating off Cloudflare Worker runtime types.
declare global {
  interface D1PreparedStatement {
    bind(...values: ReadonlyArray<unknown>): D1PreparedStatement
    first<T = Record<string, unknown>>(colName?: string): Promise<T | null>
    all<T = Record<string, unknown>>(): Promise<{
      readonly results: ReadonlyArray<T>
      readonly success: boolean
      readonly meta: Record<string, unknown>
    }>
    run<T = Record<string, unknown>>(): Promise<{
      readonly results: ReadonlyArray<T>
      readonly success: boolean
      readonly meta: Record<string, unknown>
    }>
    raw<T = ReadonlyArray<unknown>>(): Promise<ReadonlyArray<T>>
  }

  interface D1Database {
    prepare(query: string): D1PreparedStatement
    batch<T = Record<string, unknown>>(
      statements: ReadonlyArray<D1PreparedStatement>,
    ): Promise<ReadonlyArray<{ readonly results: ReadonlyArray<T> }>>
    exec(query: string): Promise<unknown>
    dump(): Promise<ArrayBuffer>
  }

  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface DurableObjectId {}

  interface DurableObjectStub {
    fetch(input: Request | string, init?: RequestInit): Promise<Response>
  }

  interface DurableObjectNamespace {
    idFromName(name: string): DurableObjectId
    idFromString(id: string): DurableObjectId
    newUniqueId(): DurableObjectId
    get(id: DurableObjectId): DurableObjectStub
  }

  interface R2Bucket {
    get(key: string): Promise<unknown | null>
    put(key: string, value: unknown): Promise<unknown>
    delete(key: string): Promise<void>
    head(key: string): Promise<unknown | null>
    list(options?: unknown): Promise<unknown>
  }

  interface Fetcher {
    fetch(input: Request | string, init?: RequestInit): Promise<Response>
  }

  interface ExecutionContext {
    waitUntil(promise: Promise<unknown>): void
    passThroughOnException(): void
  }
}

export {}
