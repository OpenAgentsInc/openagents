/**
 * Pure Khala Sync wire-protocol helpers, generic over entity type — shared
 * by the mobile thread list (chat_thread entities in scope.user.<owner>)
 * and the message view (chat_message entities in scope.thread.<id>). No
 * native/RN imports so this stays unit-testable under `bun test`.
 */

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

export const buildBootstrapUrl = (baseUrl: string): string =>
  `${baseUrl.replace(/\/$/, "")}/api/sync/bootstrap`

export type BootstrapRequestBody = Readonly<{
  protocolVersion: number
  schemaVersion: number
  scope: string
  clientGroupId: string
}>

export const buildBootstrapRequestBody = (
  scope: string,
  clientGroupId: string
): BootstrapRequestBody => ({
  clientGroupId,
  protocolVersion: 1,
  schemaVersion: 1,
  scope
})

export const buildConnectUrl = (
  baseUrl: string,
  scope: string,
  cursor: number
): string => {
  const httpUrl = new URL(`${baseUrl.replace(/\/$/, "")}/api/sync/connect`)
  httpUrl.searchParams.set("scope", scope)
  httpUrl.searchParams.set("cursor", String(cursor))
  const wsUrl = new URL(httpUrl.toString())
  wsUrl.protocol = wsUrl.protocol === "https:" ? "wss:" : "ws:"
  return wsUrl.toString()
}

export const entitiesOfType = <T>(
  entities: ReadonlyArray<BootstrapEntityRow>,
  entityType: string,
  decode: EntityDecoder<T>
): ReadonlyArray<T> =>
  entities
    .filter(entity => entity.entityType === entityType)
    .map(entity => decode(JSON.parse(entity.postImageJson)))

export const applyDeltaFrameOfType = <T>(
  current: ReadonlyArray<T>,
  frame: DeltaFrameLike,
  entityType: string,
  idOf: (item: T) => string,
  decode: EntityDecoder<T>
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
  keyOf: (item: T) => string
): ReadonlyArray<T> => [...items].sort((a, b) => keyOf(a).localeCompare(keyOf(b)))

export const sortByKeyDesc = <T>(
  items: ReadonlyArray<T>,
  keyOf: (item: T) => string
): ReadonlyArray<T> => [...items].sort((a, b) => keyOf(b).localeCompare(keyOf(a)))
