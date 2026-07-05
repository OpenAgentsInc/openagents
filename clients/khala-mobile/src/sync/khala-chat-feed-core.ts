import { threadScope, type SyncScope } from "@openagentsinc/khala-sync"

const PROTOCOL_VERSION = 1
const SCHEMA_VERSION = 1

export const chatFeedScope = (threadId: string): SyncScope => threadScope(threadId)

export type BootstrapRequestBody = Readonly<{
  protocolVersion: number
  schemaVersion: number
  scope: string
  clientGroupId: string
}>

export const buildBootstrapRequestBody = (
  scope: SyncScope,
  clientGroupId: string
): BootstrapRequestBody => ({
  clientGroupId,
  protocolVersion: PROTOCOL_VERSION,
  schemaVersion: SCHEMA_VERSION,
  scope: String(scope)
})

export const buildBootstrapUrl = (baseUrl: string): string =>
  `${baseUrl.replace(/\/$/, "")}/api/sync/bootstrap`

export const buildConnectUrl = (
  baseUrl: string,
  scope: SyncScope,
  cursor: number
): string => {
  const httpUrl = new URL(`${baseUrl.replace(/\/$/, "")}/api/sync/connect`)
  httpUrl.searchParams.set("scope", String(scope))
  httpUrl.searchParams.set("cursor", String(cursor))
  const wsUrl = new URL(httpUrl.toString())
  wsUrl.protocol = wsUrl.protocol === "https:" ? "wss:" : "ws:"
  return wsUrl.toString()
}

export type ChatFeedEvent = Readonly<{
  id: string
  receivedAt: string
  kind: "bootstrap" | "frame" | "error"
  raw: string
}>

export const makeFeedEvent = (
  kind: ChatFeedEvent["kind"],
  payload: unknown,
  receivedAt: string,
  seq: number
): ChatFeedEvent => ({
  id: `${kind}-${seq}`,
  kind,
  raw: JSON.stringify(payload, null, 2),
  receivedAt
})
