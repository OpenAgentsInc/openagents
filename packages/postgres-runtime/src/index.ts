/** Stock-Node Postgres pool constructor replacing Bun's built-in SQL client. */
import postgres from "postgres"

// This is the deliberately narrow, public structural contract the retained
// services used from the former runtime SQL client. postgres.js is the
// implementation; keeping its private PendingQuery types out of the seam lets
// shared Worker/Node modules
// continue to accept a normal Promise-shaped tagged template.
export interface SQL {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>): Promise<any>
  readonly begin: {
    <A>(fn: (tx: SQL) => A | Promise<A>): Promise<A>
    <A>(options: string, fn: (tx: SQL) => A | Promise<A>): Promise<A>
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  unsafe(query: string, parameters?: ReadonlyArray<unknown>): Promise<any>
  // Driver-specific consumers narrow the reserved connection further.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reserve(): Promise<any>
  end(options?: { readonly timeout?: number }): Promise<void>
}

export type SqlOptions = {
  readonly url?: string
  readonly host?: string
  readonly port?: number
  readonly database?: string
  readonly username?: string
  readonly password?: string
  readonly max?: number
  readonly idle_timeout?: number
  readonly connect_timeout?: number
  readonly ssl?: boolean | "require" | "allow" | "prefer" | "verify-full"
  readonly prepare?: boolean
  readonly onnotice?: (notice: unknown) => void
}

export const SQL = (
  input: string | SqlOptions,
  options: Parameters<typeof postgres>[1] = {},
): SQL => {
  if (typeof input === "string") return postgres(input, options) as unknown as SQL
  const { url, ...rest } = input
  if (url !== undefined) return postgres(url, rest as never) as unknown as SQL
  if (rest.host === undefined) throw new TypeError("Postgres URL or host is required")
  return postgres(rest as never) as unknown as SQL
}
