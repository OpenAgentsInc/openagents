import {
  BootstrapRequest,
  ClientGroupId,
  ClientId,
  FleetPublicRef,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutatorName,
  PushRequest,
  SyncSchemaVersion,
  SyncScope,
  SyncVersionWatermark,
  canonicalJson,
  decodeBootstrapResponse,
  decodeLogPage,
  decodePushResponse,
  decodeSyncError,
  encodeBootstrapRequest,
  encodePushRequest,
  fleetRunScope,
  type BootstrapResponse,
  type LogPage,
} from "@openagentsinc/khala-sync"
import {
  decodeKhalaFleetIntent,
  type KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"
import { Schema } from "effect"

export const SARAH_FLEET_CURSOR_STATE_SCHEMA =
  "sarah.fleet_sync_cursor_state.v1" as const

/** Mirrors khala-sync-server `MAX_BOOTSTRAP_PAGE_SIZE` / `MAX_LOG_PAGE_LIMIT`. */
export const MAX_SARAH_FLEET_SYNC_PAGE_SIZE = 1_000
/** Local loop ceilings; callers may lower them but cannot disable the bound. */
export const MAX_SARAH_FLEET_BOOTSTRAP_PAGES = 128
export const MAX_SARAH_FLEET_LOG_PAGES = 256

export const SarahFleetSyncCursorState = Schema.Struct({
  schema: Schema.Literal(SARAH_FLEET_CURSOR_STATE_SCHEMA),
  scope: SyncScope,
  cursor: SyncVersionWatermark,
})
export type SarahFleetSyncCursorState =
  typeof SarahFleetSyncCursorState.Type

export type SarahFleetSyncClientErrorReason =
  | "invalid_client_identity"
  | "invalid_request"
  | "invalid_scope"
  | "invalid_cursor_state"
  | "invalid_intent"
  | "network_unavailable"
  | "request_aborted"
  | "server_rejected"
  | "malformed_response"
  | "foreign_scope"
  | "pagination_cycle"
  | "pagination_limit"
  | "cursor_no_progress"
  | "mutation_rejected"

const ERROR_MESSAGES = {
  invalid_client_identity: "Fleet Sync client identity is invalid.",
  invalid_request: "Fleet Sync request parameters are invalid.",
  invalid_scope: "Fleet Sync scope is not an exact fleet-run scope.",
  invalid_cursor_state: "Fleet Sync cursor state is invalid.",
  invalid_intent: "Fleet intent failed its typed target contract.",
  network_unavailable: "Fleet Sync network request failed.",
  request_aborted: "Fleet Sync request was aborted.",
  server_rejected: "Fleet Sync server rejected the request.",
  malformed_response: "Fleet Sync response failed to decode.",
  foreign_scope: "Fleet Sync response did not match the requested scope.",
  pagination_cycle: "Fleet Sync pagination repeated a continuation token.",
  pagination_limit: "Fleet Sync pagination exceeded its bounded page limit.",
  cursor_no_progress: "Fleet Sync cursor did not make required progress.",
  mutation_rejected: "Fleet intent mutation was rejected.",
} as const satisfies Record<SarahFleetSyncClientErrorReason, string>

export class SarahFleetSyncClientError extends Error {
  readonly _tag = "SarahFleetSyncClientError"
  override readonly name = "SarahFleetSyncClientError"

  constructor(
    readonly reason: SarahFleetSyncClientErrorReason,
    readonly status?: number,
    readonly serverCode?: string,
    readonly retryable?: boolean,
  ) {
    super(ERROR_MESSAGES[reason])
  }
}

export type SarahFleetFetch = (
  input: string,
  init: RequestInit,
) => Promise<Response>

export type SarahFleetHttpRequest = Readonly<{
  path: string
  init: RequestInit
}>

export type SarahFleetSyncRequestOptions = Readonly<{
  signal?: AbortSignal
}>

type ClientIdentity = Readonly<{
  clientGroupId: typeof ClientGroupId.Type
  clientId: typeof ClientId.Type
}>

const fail = (
  reason: SarahFleetSyncClientErrorReason,
  status?: number,
  serverCode?: string,
  retryable?: boolean,
): never => {
  throw new SarahFleetSyncClientError(
    reason,
    status,
    serverCode,
    retryable,
  )
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const assertExactKeys = (
  value: unknown,
  allowed: ReadonlyArray<string>,
): Record<string, unknown> => {
  if (!isRecord(value)) return fail("malformed_response")
  const allowedKeys = new Set(allowed)
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) {
    return fail("malformed_response")
  }
  return value
}

const exactFleetScope = (
  raw: string,
): Readonly<{
  scope: typeof SyncScope.Type
  runRef: typeof FleetPublicRef.Type
}> => {
  const match =
    /^scope\.fleet_run\.([A-Za-z0-9](?:[A-Za-z0-9._:-]*[A-Za-z0-9])?)$/.exec(
      raw,
    )
  if (match?.[1] === undefined) return fail("invalid_scope")
  try {
    const runRef = Schema.decodeUnknownSync(FleetPublicRef)(match[1])
    const scope = Schema.decodeUnknownSync(SyncScope)(raw)
    if (fleetRunScope(runRef) !== scope) return fail("invalid_scope")
    return { scope, runRef }
  } catch (error) {
    if (error instanceof SarahFleetSyncClientError) throw error
    return fail("invalid_scope")
  }
}

const sameOriginInit = (init: RequestInit): RequestInit => ({
  ...init,
  credentials: "same-origin",
})

export const buildSarahFleetBootstrapRequest = (
  identity: ClientIdentity,
  input: Readonly<{
    scope: string
    pageSize: number
    pageToken?: string
  }>,
): SarahFleetHttpRequest => {
  const { scope } = exactFleetScope(input.scope)
  if (
    !Number.isSafeInteger(input.pageSize) ||
    input.pageSize < 1 ||
    input.pageSize > MAX_SARAH_FLEET_SYNC_PAGE_SIZE ||
    (input.pageToken !== undefined && input.pageToken.length > 4_096)
  ) {
    return fail("invalid_request")
  }
  const request = new BootstrapRequest({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion: SyncSchemaVersion.make(1),
    clientGroupId: identity.clientGroupId,
    scope,
    pageSize: input.pageSize,
    ...(input.pageToken === undefined ? {} : { pageToken: input.pageToken }),
  })
  return {
    path: "/api/sync/bootstrap",
    init: sameOriginInit({
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(encodeBootstrapRequest(request)),
    }),
  }
}

export const buildSarahFleetLogRequest = (input: Readonly<{
  scope: string
  cursor: number
  limit: number
}>): SarahFleetHttpRequest => {
  const { scope } = exactFleetScope(input.scope)
  if (
    !Number.isSafeInteger(input.limit) ||
    input.limit < 1 ||
    input.limit > MAX_SARAH_FLEET_SYNC_PAGE_SIZE
  ) {
    return fail("invalid_request")
  }
  let cursor: typeof SyncVersionWatermark.Type
  try {
    cursor = SyncVersionWatermark.make(input.cursor)
  } catch {
    return fail("invalid_cursor_state")
  }
  const query = new URLSearchParams({
    scope,
    cursor: String(cursor),
    limit: String(input.limit),
  })
  return {
    path: `/api/sync/log?${query.toString()}`,
    init: sameOriginInit({
      method: "GET",
      headers: { accept: "application/json" },
    }),
  }
}

const MUTATOR_BY_KIND = {
  fleet_run_control: "fleet.dispatchRunControl",
  approval_decision: "fleet.dispatchApprovalDecision",
  steer_message: "fleet.dispatchSteerMessage",
} as const

export const buildSarahFleetPushRequest = (
  identity: ClientIdentity,
  input: Readonly<{
    scope: string
    mutationId: number
    intent: KhalaFleetIntent
  }>,
): SarahFleetHttpRequest => {
  const { runRef } = exactFleetScope(input.scope)
  let intent: KhalaFleetIntent
  let mutationId: typeof MutationId.Type
  try {
    intent = decodeKhalaFleetIntent(input.intent)
    mutationId = MutationId.make(input.mutationId)
    Schema.decodeUnknownSync(FleetPublicRef)(intent.intentId)
    Schema.decodeUnknownSync(FleetPublicRef)(intent.idempotencyKey)
  } catch {
    return fail("invalid_intent")
  }
  if (intent.runRef !== runRef || intent.kind === "worker_selection") {
    return fail("invalid_intent")
  }
  try {
    if (
      (intent.kind === "fleet_run_control" ||
        intent.kind === "approval_decision") &&
      intent.reasonRef !== undefined
    ) {
      Schema.decodeUnknownSync(FleetPublicRef)(intent.reasonRef)
    }
    if (intent.kind === "approval_decision") {
      Schema.decodeUnknownSync(FleetPublicRef)(intent.approvalRef)
    }
    if (intent.kind === "steer_message") {
      if (intent.targetRef === undefined) return fail("invalid_intent")
      Schema.decodeUnknownSync(FleetPublicRef)(intent.targetRef)
      if (intent.bodyRef !== undefined) {
        Schema.decodeUnknownSync(FleetPublicRef)(intent.bodyRef)
      }
    }
  } catch (error) {
    if (error instanceof SarahFleetSyncClientError) throw error
    return fail("invalid_intent")
  }
  const mutator = MUTATOR_BY_KIND[intent.kind]
  const request = new PushRequest({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion: SyncSchemaVersion.make(1),
    clientGroupId: identity.clientGroupId,
    clientId: identity.clientId,
    mutations: [
      new MutationEnvelope({
        mutationId,
        name: MutatorName.make(mutator),
        argsJson: canonicalJson(intent),
      }),
    ],
  })
  return {
    path: "/api/sync/push",
    init: sameOriginInit({
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(encodePushRequest(request)),
    }),
  }
}

const decodeBootstrapPage = (
  raw: unknown,
  expectedScope: typeof SyncScope.Type,
): BootstrapResponse => {
  const record = assertExactKeys(raw, [
    "protocolVersion",
    "scope",
    "entities",
    "cursor",
    "nextPageToken",
  ])
  if (!Array.isArray(record.entities)) return fail("malformed_response")
  for (const entity of record.entities) {
    assertExactKeys(entity, ["entityType", "entityId", "postImageJson"])
  }
  let page: BootstrapResponse
  try {
    page = decodeBootstrapResponse(record)
  } catch {
    return fail("malformed_response")
  }
  if (page.scope !== expectedScope) return fail("foreign_scope")
  if (
    (page.nextPageToken === undefined) ===
    (page.cursor === undefined)
  ) {
    return fail("malformed_response")
  }
  return page
}

const decodeLogPageForScope = (
  raw: unknown,
  expectedScope: typeof SyncScope.Type,
  cursor: number,
): LogPage => {
  const record = assertExactKeys(raw, [
    "protocolVersion",
    "scope",
    "entries",
    "nextCursor",
    "upToDate",
  ])
  if (!Array.isArray(record.entries)) return fail("malformed_response")
  for (const entry of record.entries) {
    assertExactKeys(entry, [
      "scope",
      "version",
      "entityType",
      "entityId",
      "op",
      "postImageJson",
      "mutationRef",
      "committedAt",
    ])
  }
  let page: LogPage
  try {
    page = decodeLogPage(record)
  } catch {
    return fail("malformed_response")
  }
  if (
    page.scope !== expectedScope ||
    page.entries.some((entry) => entry.scope !== expectedScope)
  ) {
    return fail("foreign_scope")
  }
  let previous = cursor
  for (const entry of page.entries) {
    if (entry.version <= cursor || entry.version < previous) {
      return fail("cursor_no_progress")
    }
    previous = entry.version
  }
  const expectedCursor = page.entries.at(-1)?.version ?? cursor
  if (page.nextCursor !== expectedCursor) return fail("cursor_no_progress")
  if (!page.upToDate && page.nextCursor === cursor) {
    return fail("cursor_no_progress")
  }
  return page
}

const decodeCursorState = (
  raw: SarahFleetSyncCursorState,
): SarahFleetSyncCursorState => {
  assertExactKeys(raw, ["schema", "scope", "cursor"])
  let state: SarahFleetSyncCursorState
  try {
    state = Schema.decodeUnknownSync(SarahFleetSyncCursorState)(raw)
  } catch {
    return fail("invalid_cursor_state")
  }
  exactFleetScope(state.scope)
  return state
}

export type SarahFleetSyncClient = Readonly<{
  bootstrap: (
    scope: string,
    options?: SarahFleetSyncRequestOptions,
  ) => Promise<
    Readonly<{
      pages: ReadonlyArray<BootstrapResponse>
      state: SarahFleetSyncCursorState
    }>
  >
  resume: (
    state: SarahFleetSyncCursorState,
    options?: SarahFleetSyncRequestOptions,
  ) => Promise<
    Readonly<{
      pages: ReadonlyArray<LogPage>
      state: SarahFleetSyncCursorState
    }>
  >
  submitIntent: (input: Readonly<{
    scope: string
    mutationId: number
    intent: KhalaFleetIntent
  }>) => Promise<Readonly<{
    intentId: string
    mutationId: number
    status: "applied" | "duplicate"
    lastMutationId: number
  }>>
}>

export function makeSarahFleetSyncClient(input: Readonly<{
  fetch: SarahFleetFetch
  clientGroupId: string
  clientId: string
  pageSize?: number
  maxBootstrapPages?: number
  maxLogPages?: number
}>): SarahFleetSyncClient {
  let identity: ClientIdentity
  try {
    identity = {
      clientGroupId: ClientGroupId.make(input.clientGroupId),
      clientId: ClientId.make(input.clientId),
    }
  } catch {
    return fail("invalid_client_identity")
  }
  const pageSize = input.pageSize ?? 200
  const maxBootstrapPages = input.maxBootstrapPages ?? 32
  const maxLogPages = input.maxLogPages ?? 64
  if (
    !Number.isSafeInteger(pageSize) ||
    pageSize < 1 ||
    pageSize > MAX_SARAH_FLEET_SYNC_PAGE_SIZE ||
    !Number.isSafeInteger(maxBootstrapPages) ||
    maxBootstrapPages < 1 ||
    maxBootstrapPages > MAX_SARAH_FLEET_BOOTSTRAP_PAGES ||
    !Number.isSafeInteger(maxLogPages) ||
    maxLogPages < 1 ||
    maxLogPages > MAX_SARAH_FLEET_LOG_PAGES
  ) {
    return fail("invalid_request")
  }

  const fetchJson = async (
    request: SarahFleetHttpRequest,
    options?: SarahFleetSyncRequestOptions,
  ): Promise<unknown> => {
    const aborted = (): boolean => options?.signal?.aborted === true
    if (aborted()) return fail("request_aborted")
    let response: Response
    try {
      response = await input.fetch(
        request.path,
        options?.signal === undefined
          ? request.init
          : { ...request.init, signal: options.signal },
      )
    } catch {
      if (aborted()) return fail("request_aborted")
      return fail("network_unavailable")
    }
    if (aborted()) return fail("request_aborted")
    let raw: unknown
    try {
      raw = await response.json()
    } catch {
      if (aborted()) return fail("request_aborted")
      return fail("malformed_response")
    }
    if (aborted()) return fail("request_aborted")
    if (!response.ok) {
      try {
        const error = decodeSyncError(raw)
        return fail(
          "server_rejected",
          response.status,
          error.code,
          error.retryable,
        )
      } catch (error) {
        if (error instanceof SarahFleetSyncClientError) throw error
        return fail("server_rejected", response.status)
      }
    }
    return raw
  }

  const bootstrap: SarahFleetSyncClient["bootstrap"] = async (
    rawScope,
    options,
  ) => {
    const { scope } = exactFleetScope(rawScope)
    const pages: BootstrapResponse[] = []
    const seenTokens = new Set<string>()
    let pageToken: string | undefined
    for (let pageIndex = 0; pageIndex < maxBootstrapPages; pageIndex += 1) {
      const request = buildSarahFleetBootstrapRequest(identity, {
        scope,
        pageSize,
        ...(pageToken === undefined ? {} : { pageToken }),
      })
      const page = decodeBootstrapPage(
        await fetchJson(request, options),
        scope,
      )
      pages.push(page)
      if (page.nextPageToken !== undefined) {
        if (seenTokens.has(page.nextPageToken)) return fail("pagination_cycle")
        seenTokens.add(page.nextPageToken)
        pageToken = page.nextPageToken
        continue
      }
      if (page.cursor === undefined) return fail("malformed_response")
      return {
        pages,
        state: Schema.decodeUnknownSync(SarahFleetSyncCursorState)({
          schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
          scope,
          cursor: page.cursor,
        }),
      }
    }
    return fail("pagination_limit")
  }

  const resume: SarahFleetSyncClient["resume"] = async (
    rawState,
    options,
  ) => {
    const state = decodeCursorState(rawState)
    const pages: LogPage[] = []
    let cursor = state.cursor
    for (let pageIndex = 0; pageIndex < maxLogPages; pageIndex += 1) {
      const request = buildSarahFleetLogRequest({
        scope: state.scope,
        cursor,
        limit: pageSize,
      })
      const page = decodeLogPageForScope(
        await fetchJson(request, options),
        state.scope,
        cursor,
      )
      pages.push(page)
      cursor = page.nextCursor
      if (page.upToDate) {
        return {
          pages,
          state: Schema.decodeUnknownSync(SarahFleetSyncCursorState)({
            schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
            scope: state.scope,
            cursor,
          }),
        }
      }
    }
    return fail("pagination_limit")
  }

  const submitIntent: SarahFleetSyncClient["submitIntent"] = async (value) => {
    const request = buildSarahFleetPushRequest(identity, value)
    const raw = await fetchJson(request)
    const record = assertExactKeys(raw, [
      "protocolVersion",
      "results",
      "lastMutationId",
    ])
    if (!Array.isArray(record.results)) return fail("malformed_response")
    for (const result of record.results) {
      assertExactKeys(result, [
        "mutationId",
        "status",
        "errorCode",
        "errorMessageSafe",
      ])
    }
    let response
    try {
      response = decodePushResponse(record)
    } catch {
      return fail("malformed_response")
    }
    if (
      response.results.length !== 1 ||
      response.results[0]?.mutationId !== value.mutationId
    ) {
      return fail("malformed_response")
    }
    const result = response.results[0]
    if (result.status === "rejected") {
      const serverCode =
        result.errorCode !== undefined &&
        /^[a-z][a-z0-9_]{0,63}$/.test(result.errorCode)
          ? result.errorCode
          : undefined
      return fail("mutation_rejected", undefined, serverCode)
    }
    return {
      intentId: value.intent.intentId,
      mutationId: value.mutationId,
      status: result.status,
      lastMutationId: response.lastMutationId,
    }
  }

  return { bootstrap, resume, submitIntent }
}
