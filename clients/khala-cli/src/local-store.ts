import { randomBytes } from "node:crypto"
import { mkdir, readFile, rename, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

// Device-local Khala store for the zero-install front door (`khala up`,
// issue #8784). This is client-side convenience + grant state ONLY — it is
// never account authority and requires no signup. Vocabulary follows ENV-1
// (docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md):
// pairing creates a device-local KnownEnvironment entry plus an opaque
// device grant for the paired browser client.

export const KHALA_LOCAL_STORE_SCHEMA_VERSION = 1

export interface KhalaKnownEnvironment {
  // ENV-1 KnownEnvironment: a client-local saved entry for an environment
  // this device knows how to reach. Device-local convenience, never authority.
  readonly environmentRef: string
  readonly url: string
  readonly pairedAt: string
  readonly grantId: string
}

export interface KhalaDeviceGrant {
  readonly grantId: string
  // Only a SHA-256 hash of the grant secret is persisted; the secret itself
  // is returned exactly once in the pairing response.
  readonly secretHash: string
  readonly scope: "local-control"
  // TODO(ENV-2, #8780): upgrade this opaque device grant to a scoped,
  // DPoP-bound capability token from packages/environment-auth once the
  // broker lane hardens client-held grants. The seam is this record: swap
  // `kind` to a capability-token kind and carry the token's claims here
  // instead of an opaque secret hash. Never a hardcoded shared secret.
  readonly kind: "opaque-device-grant"
  readonly createdAt: string
}

export interface KhalaLocalStore {
  readonly schemaVersion: number
  readonly deviceId: string
  readonly createdAt: string
  readonly migratedAt?: string
  readonly knownEnvironments: ReadonlyArray<KhalaKnownEnvironment>
  readonly grants: ReadonlyArray<KhalaDeviceGrant>
}

export interface KhalaLocalStoreOpenResult {
  readonly store: KhalaLocalStore
  readonly path: string
  readonly outcome: "initialized" | "migrated" | "loaded"
}

export class KhalaLocalStoreVersionError extends Error {
  constructor(readonly foundVersion: number) {
    super(
      `Local Khala store schema v${foundVersion} is newer than this CLI understands (v${KHALA_LOCAL_STORE_SCHEMA_VERSION}). ` +
        "Upgrade the CLI: npm install -g @openagentsinc/khala",
    )
    this.name = "KhalaLocalStoreVersionError"
  }
}

export function khalaLocalStorePath(env: Record<string, string | undefined>): string {
  const override = env.KHALA_LOCAL_STORE_PATH?.trim()
  if (override) return override
  // Same root as the CLI's other device-local state (see token-store.ts).
  const configHome = env.XDG_CONFIG_HOME?.trim() || join(homedir(), ".config")
  return join(configHome, "khala", "local-store.json")
}

// Registered migrations: index N upgrades a raw store from schema version N to
// N+1. Version 0 covers legacy/partial files written without a schemaVersion.
const MIGRATIONS: ReadonlyArray<(raw: Record<string, unknown>) => Record<string, unknown>> = [
  raw => ({
    schemaVersion: 1,
    deviceId: typeof raw.deviceId === "string" && raw.deviceId.length > 0 ? raw.deviceId : newDeviceId(),
    createdAt: typeof raw.createdAt === "string" ? raw.createdAt : new Date().toISOString(),
    knownEnvironments: Array.isArray(raw.knownEnvironments) ? raw.knownEnvironments : [],
    grants: Array.isArray(raw.grants) ? raw.grants : [],
  }),
]

// Initializes the store when absent, migrates older schemas forward, and
// refuses schemas newer than this CLI. Corrupt JSON is an error (we never
// silently destroy existing device state).
export async function openKhalaLocalStore(
  env: Record<string, string | undefined>,
): Promise<KhalaLocalStoreOpenResult> {
  const path = khalaLocalStorePath(env)
  let text: string | undefined
  try {
    text = await readFile(path, "utf8")
  } catch {
    text = undefined
  }

  if (text === undefined) {
    const store: KhalaLocalStore = {
      schemaVersion: KHALA_LOCAL_STORE_SCHEMA_VERSION,
      deviceId: newDeviceId(),
      createdAt: new Date().toISOString(),
      knownEnvironments: [],
      grants: [],
    }
    await writeKhalaLocalStore(path, store)
    return { store, path, outcome: "initialized" }
  }

  let raw: Record<string, unknown>
  try {
    const parsed: unknown = JSON.parse(text)
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new Error("not an object")
    }
    raw = parsed as Record<string, unknown>
  } catch {
    throw new Error(
      `Local Khala store at ${path} is not valid JSON. Move it aside and re-run, or fix it by hand — it is never overwritten silently.`,
    )
  }

  const foundVersion = typeof raw.schemaVersion === "number" && Number.isInteger(raw.schemaVersion) && raw.schemaVersion >= 0
    ? raw.schemaVersion
    : 0
  if (foundVersion > KHALA_LOCAL_STORE_SCHEMA_VERSION) {
    throw new KhalaLocalStoreVersionError(foundVersion)
  }

  if (foundVersion === KHALA_LOCAL_STORE_SCHEMA_VERSION) {
    return { store: raw as unknown as KhalaLocalStore, path, outcome: "loaded" }
  }

  let migrated = raw
  for (let version = foundVersion; version < KHALA_LOCAL_STORE_SCHEMA_VERSION; version += 1) {
    const migrate = MIGRATIONS[version]
    if (migrate === undefined) {
      throw new Error(`No migration registered from local store schema v${version}.`)
    }
    migrated = migrate(migrated)
  }
  const store: KhalaLocalStore = {
    ...(migrated as unknown as KhalaLocalStore),
    migratedAt: new Date().toISOString(),
  }
  await writeKhalaLocalStore(path, store)
  return { store, path, outcome: "migrated" }
}

export async function writeKhalaLocalStore(path: string, store: KhalaLocalStore): Promise<void> {
  await mkdir(dirname(path), { mode: 0o700, recursive: true })
  // Write-then-rename so a crash mid-write never leaves a corrupt store.
  const tmp = `${path}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tmp, `${JSON.stringify(store, null, 2)}\n`, { mode: 0o600 })
  await rename(tmp, path)
}

function newDeviceId(): string {
  return `khala-local-${randomBytes(8).toString("hex")}`
}
