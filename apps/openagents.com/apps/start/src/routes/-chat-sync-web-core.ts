/**
 * Pure Khala Sync wire-protocol helpers for the web Start app — ported
 * near-verbatim from `clients/khala-mobile/src/sync/khala-sync-entities-core.ts`
 * and `khala-sync-push-core.ts` (issue #8413). No React/DOM dependency
 * beyond `URL`, so this stays unit-testable under `vitest`.
 *
 * Unlike the mobile version, these builders always point at THIS APP'S OWN
 * same-origin proxy paths (`/api/khala-sync/*`, see `../khala-sync-proxy.ts`)
 * rather than a remote `baseUrl` — the proxy is the thing that holds the
 * bearer token (in an httpOnly cookie) and forwards to the real
 * `openagents.com` Khala Sync API with an `Authorization` header, since a
 * standard browser `WebSocket` cannot set that header itself on a
 * cross-origin upgrade request.
 */

// Path constants are the single shared source between the server-side proxy
// (`../khala-sync-proxy.ts`, which has Node/Workers-only imports and must
// never be pulled into a browser bundle) and this module's client-safe
// builders/hooks — keep both sides importing from HERE, not redefining these
// literals in the proxy module.
export const KHALA_SYNC_WEB_SESSION_PATH = "/api/khala-sync/session"
export const KHALA_SYNC_WEB_BOOTSTRAP_PATH = "/api/khala-sync/bootstrap"
export const KHALA_SYNC_WEB_PUSH_PATH = "/api/khala-sync/push"
export const KHALA_SYNC_WEB_CONNECT_PATH = "/api/khala-sync/connect"

export type BootstrapEntityRow = Readonly<{
  entityType: string
  entityId: string
  postImageJson: string
}>

export type DeltaEntryRow = Readonly<{
  entityType: string
  entityId: string
  op: string
  postImageJson?: string
}>

export type DeltaFrameLike = Readonly<{
  _tag: string
  entries: ReadonlyArray<DeltaEntryRow>
}>

export type EntityDecoder<T> = (value: unknown) => T

export type BootstrapRequestBody = Readonly<{
  protocolVersion: number
  schemaVersion: number
  scope: string
  clientGroupId: string
}>

export const buildBootstrapRequestBody = (
  scope: string,
  clientGroupId: string,
): BootstrapRequestBody => ({
  clientGroupId,
  protocolVersion: 1,
  schemaVersion: 1,
  scope,
})

/**
 * Builds an absolute `ws(s)://` URL for the local same-origin connect proxy
 * from the page's own location — the browser `WebSocket` constructor
 * requires an absolute URL, unlike `fetch`, which accepts a relative path.
 */
export const buildConnectUrl = (
  scope: string,
  cursor: number,
  location: Readonly<{ protocol: string; host: string }>,
): string => {
  const wsProtocol = location.protocol === "https:" ? "wss:" : "ws:"
  const url = new URL(`${wsProtocol}//${location.host}${KHALA_SYNC_WEB_CONNECT_PATH}`)
  url.searchParams.set("scope", scope)
  url.searchParams.set("cursor", String(cursor))
  return url.toString()
}

export const entitiesOfType = <T>(
  entities: ReadonlyArray<BootstrapEntityRow>,
  entityType: string,
  decode: EntityDecoder<T>,
): ReadonlyArray<T> =>
  entities
    .filter(entity => entity.entityType === entityType)
    .map(entity => decode(JSON.parse(entity.postImageJson)))

export const applyDeltaFrameOfType = <T>(
  current: ReadonlyArray<T>,
  frame: DeltaFrameLike,
  entityType: string,
  idOf: (item: T) => string,
  decode: EntityDecoder<T>,
): ReadonlyArray<T> => {
  let next = current
  for (const entry of frame.entries) {
    if (entry.entityType !== entityType) continue
    if (entry.op === "delete") {
      next = next.filter(item => idOf(item) !== entry.entityId)
      continue
    }
    if (entry.postImageJson === undefined) continue
    const decoded = decode(JSON.parse(entry.postImageJson))
    const decodedId = idOf(decoded)
    const idx = next.findIndex(item => idOf(item) === decodedId)
    next = idx === -1 ? [...next, decoded] : next.map((item, i) => (i === idx ? decoded : item))
  }
  return next
}

export const sortByKeyAsc = <T>(
  items: ReadonlyArray<T>,
  keyOf: (item: T) => string,
): ReadonlyArray<T> => [...items].sort((a, b) => keyOf(a).localeCompare(keyOf(b)))

export const sortByKeyDesc = <T>(
  items: ReadonlyArray<T>,
  keyOf: (item: T) => string,
): ReadonlyArray<T> => [...items].sort((a, b) => keyOf(b).localeCompare(keyOf(a)))

// ---------------------------------------------------------------------------
// Push (mirrors khala-sync-push-core.ts)
// ---------------------------------------------------------------------------

export type MutationEnvelopeInput = Readonly<{
  mutationId: number
  name: string
  argsJson: string
}>

export type PushRequestBody = Readonly<{
  protocolVersion: number
  schemaVersion: number
  clientGroupId: string
  clientId: string
  mutations: ReadonlyArray<MutationEnvelopeInput>
}>

export const buildPushRequestBody = (input: {
  clientGroupId: string
  clientId: string
  mutations: ReadonlyArray<MutationEnvelopeInput>
}): PushRequestBody => ({
  clientGroupId: input.clientGroupId,
  clientId: input.clientId,
  mutations: input.mutations,
  protocolVersion: 1,
  schemaVersion: 1,
})

/** A minimal, dependency-free stable stringify — key order doesn't need to
 * match the server's canonicalJson exactly (argsJson is just decoded as
 * regular JSON on the server), but sorting keys keeps push payloads
 * deterministic for tests/logging. */
export const stableArgsJson = (value: Record<string, unknown>): string => {
  const sortedEntries = Object.entries(value)
    .filter(([, v]) => v !== undefined)
    .sort(([a], [b]) => a.localeCompare(b))
  return JSON.stringify(Object.fromEntries(sortedEntries))
}

const randomHex = (length: number): string =>
  Array.from({ length }, () => Math.floor(Math.random() * 16).toString(16)).join("")

/** Generates a Khala Sync safe-ref-shaped id (`^[A-Za-z0-9][A-Za-z0-9._:-]*$`)
 * unique enough for client-issued threadId/messageId values. */
export const makeSafeRef = (prefix: string): string => `${prefix}.${Date.now().toString(36)}${randomHex(10)}`
