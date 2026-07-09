import {
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_INBOX_FLAG_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  FleetPublicRef,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  canonicalJson,
  decodeBootstrapResponse,
  decodeFleetAccountEntity,
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetSteerEntity,
  decodeFleetWorkerEntity,
  decodeLogPage,
  encodeFleetAccountEntity,
  encodeFleetApprovalEntity,
  encodeFleetAssignmentEntity,
  encodeFleetInboxFlagEntity,
  encodeFleetRunEntity,
  encodeFleetSteerEntity,
  encodeFleetWorkerEntity,
  fleetRunScope,
  type FleetApprovalEntity,
  type FleetAssignmentEntity,
  type FleetInboxFlagEntity,
  type FleetRunEntity,
  type FleetWorkerEntity,
  type BootstrapResponse,
  type LogPage,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import {
  projectSarahFleetOwnerRun,
  type SarahFleetOwnerProjection,
} from "../contracts/fleet-owner-projection.ts"
import {
  MAX_SARAH_FLEET_BOOTSTRAP_PAGES,
  MAX_SARAH_FLEET_LOG_PAGES,
  SARAH_FLEET_CURSOR_STATE_SCHEMA,
  type SarahFleetSyncClient,
  type SarahFleetSyncRequestOptions,
} from "./fleet-sync-client.ts"

export const SARAH_FLEET_PROJECTION_STATE_SCHEMA =
  "sarah.fleet_projection_state.v1" as const

/** A fleet is operational metadata, not an unbounded event archive. */
export const MAX_SARAH_FLEET_PROJECTION_ENTITIES = 4_096
/** Hard aggregate ceiling checked before state post-images are parsed or saved. */
export const MAX_SARAH_FLEET_PROJECTION_STATE_BYTES = 8 * 1_024 * 1_024
/** Fleet contracts are small allowlisted records; this also bounds JSON parsing. */
export const MAX_SARAH_FLEET_POST_IMAGE_LENGTH = 16_384

export const SarahFleetProjectionEntityType = Schema.Literals([
  FLEET_RUN_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_INBOX_FLAG_ENTITY_TYPE,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
])
export type SarahFleetProjectionEntityType =
  typeof SarahFleetProjectionEntityType.Type

/**
 * One durable tuple-keyed row. A null post-image is a tombstone, so a replayed
 * delete retains the version needed for idempotency and conflict detection.
 */
export const SarahFleetProjectionEntity = Schema.Struct({
  entityType: SarahFleetProjectionEntityType,
  entityId: FleetPublicRef,
  version: SyncVersion,
  postImageJson: Schema.NullOr(
    Schema.String.check(Schema.isMaxLength(MAX_SARAH_FLEET_POST_IMAGE_LENGTH)),
  ),
})
export type SarahFleetProjectionEntity =
  typeof SarahFleetProjectionEntity.Type

/**
 * The single persistence format for FC-3. `entities` is canonicalized and
 * unique by `(entityType, entityId)`; each row carries its exact applied
 * version and `cursor` is the exact scope resume watermark.
 */
export const SarahFleetProjectionState = Schema.Struct({
  schema: Schema.Literal(SARAH_FLEET_PROJECTION_STATE_SCHEMA),
  scope: SyncScope,
  cursor: SyncVersionWatermark,
  entities: Schema.Array(SarahFleetProjectionEntity),
})
export type SarahFleetProjectionState =
  typeof SarahFleetProjectionState.Type

export type SarahFleetProjectionReducerErrorReason =
  | "invalid_scope"
  | "foreign_scope"
  | "invalid_state"
  | "invalid_page_sequence"
  | "invalid_post_image"
  | "unknown_entity_type"
  | "entity_key_mismatch"
  | "duplicate_conflict"
  | "version_regression"
  | "cursor_mismatch"
  | "missing_run"
  | "multiple_runs"
  | "projection_failed"
  | "persistence_failed"

const ERROR_MESSAGES = {
  invalid_scope: "Fleet projection scope is not an exact fleet-run scope.",
  foreign_scope: "Fleet projection input belongs to a different scope.",
  invalid_state: "Fleet projection state failed its durable contract.",
  invalid_page_sequence: "Fleet projection pages are not one bounded sequence.",
  invalid_post_image: "Fleet projection post-image failed its public contract.",
  unknown_entity_type: "Fleet projection entity type is not supported.",
  entity_key_mismatch: "Fleet projection entity key does not match its post-image.",
  duplicate_conflict: "Fleet projection received a conflicting duplicate entity.",
  version_regression: "Fleet projection refused an entity version regression.",
  cursor_mismatch: "Fleet projection cursor does not match the applied pages.",
  missing_run: "Fleet projection has no matching fleet run.",
  multiple_runs: "Fleet projection has more than one fleet run.",
  projection_failed: "Fleet projection could not produce Sarah owner state.",
  persistence_failed: "Fleet projection persistence failed.",
} as const satisfies Record<SarahFleetProjectionReducerErrorReason, string>

export class SarahFleetProjectionReducerError extends Error {
  readonly _tag = "SarahFleetProjectionReducerError"
  override readonly name = "SarahFleetProjectionReducerError"

  constructor(readonly reason: SarahFleetProjectionReducerErrorReason) {
    super(ERROR_MESSAGES[reason])
  }
}

const fail = (reason: SarahFleetProjectionReducerErrorReason): never => {
  throw new SarahFleetProjectionReducerError(reason)
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const assertExactKeys = (
  value: unknown,
  allowed: ReadonlyArray<string>,
  reason: SarahFleetProjectionReducerErrorReason,
): Record<string, unknown> => {
  if (!isRecord(value)) return fail(reason)
  const allowedKeys = new Set(allowed)
  if (Object.keys(value).some((key) => !allowedKeys.has(key))) {
    return fail(reason)
  }
  return value
}

const serializedByteLength = (
  value: unknown,
  reason: SarahFleetProjectionReducerErrorReason,
): number => {
  try {
    const serialized = JSON.stringify(value)
    if (serialized === undefined) return fail(reason)
    return new TextEncoder().encode(serialized).byteLength
  } catch {
    return fail(reason)
  }
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
    if (error instanceof SarahFleetProjectionReducerError) throw error
    return fail("invalid_scope")
  }
}

const keyOf = (entityType: string, entityId: string): string =>
  `${entityType.length}:${entityType}${entityId.length}:${entityId}`

const compareEntities = (
  left: SarahFleetProjectionEntity,
  right: SarahFleetProjectionEntity,
): number =>
  left.entityType.localeCompare(right.entityType) ||
  left.entityId.localeCompare(right.entityId)

const POST_IMAGE_KEYS = {
  [FLEET_RUN_ENTITY_TYPE]: [
    "runId",
    "status",
    "desiredSlots",
    "workerKind",
    "startedAt",
    "counters",
    "updatedAt",
  ],
  [FLEET_WORKER_ENTITY_TYPE]: [
    "workerId",
    "phase",
    "harnessKind",
    "assignmentRef",
    "accountRefHash",
    "lastProgressAt",
    "updatedAt",
  ],
  [FLEET_ASSIGNMENT_ENTITY_TYPE]: [
    "assignmentRef",
    "issueRef",
    "status",
    "closeoutClass",
    "updatedAt",
  ],
  [FLEET_ACCOUNT_ENTITY_TYPE]: [
    "accountRefHash",
    "readiness",
    "rateLimitClass",
    "provider",
    "capacityAvailable",
    "capacityBusy",
    "capacityQueued",
    "updatedAt",
  ],
  [FLEET_INBOX_FLAG_ENTITY_TYPE]: [
    "flagRef",
    "kind",
    "status",
    "openedAt",
    "acknowledgedAt",
    "updatedAt",
  ],
  [FLEET_APPROVAL_ENTITY_TYPE]: [
    "approvalRef",
    "status",
    "workerId",
    "toolClass",
    "openedAt",
    "decidedAt",
    "updatedAt",
  ],
  [FLEET_STEER_ENTITY_TYPE]: [
    "steerRef",
    "targetRef",
    "bodyCarrier",
    "createdAt",
    "updatedAt",
  ],
} as const satisfies Record<
  SarahFleetProjectionEntityType,
  ReadonlyArray<string>
>

const decodeAndCanonicalizePostImage = (
  rawEntityType: string,
  rawEntityId: string,
  postImageJson: string,
): string => {
  if (postImageJson.length > MAX_SARAH_FLEET_POST_IMAGE_LENGTH) {
    return fail("invalid_post_image")
  }
  let entityType: SarahFleetProjectionEntityType
  let entityId: typeof FleetPublicRef.Type
  let raw: unknown
  try {
    entityType = Schema.decodeUnknownSync(SarahFleetProjectionEntityType)(
      rawEntityType,
    )
    entityId = Schema.decodeUnknownSync(FleetPublicRef)(rawEntityId)
    raw = JSON.parse(postImageJson) as unknown
  } catch {
    if (
      !Schema.is(SarahFleetProjectionEntityType)(rawEntityType)
    ) {
      return fail("unknown_entity_type")
    }
    return fail("invalid_post_image")
  }
  assertExactKeys(raw, POST_IMAGE_KEYS[entityType], "invalid_post_image")
  if (entityType === FLEET_RUN_ENTITY_TYPE) {
    assertExactKeys(
      (raw as Record<string, unknown>).counters,
      [
        "workUnitsTotal",
        "activeAssignments",
        "completedAssignments",
        "failedAssignments",
        "blockedAssignments",
      ],
      "invalid_post_image",
    )
  }

  try {
    switch (entityType) {
      case FLEET_RUN_ENTITY_TYPE: {
        const entity = decodeFleetRunEntity(raw)
        if (entity.runId !== entityId) return fail("entity_key_mismatch")
        return canonicalJson(encodeFleetRunEntity(entity))
      }
      case FLEET_WORKER_ENTITY_TYPE: {
        const entity = decodeFleetWorkerEntity(raw)
        if (entity.workerId !== entityId) return fail("entity_key_mismatch")
        return canonicalJson(encodeFleetWorkerEntity(entity))
      }
      case FLEET_ASSIGNMENT_ENTITY_TYPE: {
        const entity = decodeFleetAssignmentEntity(raw)
        if (entity.assignmentRef !== entityId) {
          return fail("entity_key_mismatch")
        }
        return canonicalJson(encodeFleetAssignmentEntity(entity))
      }
      case FLEET_ACCOUNT_ENTITY_TYPE: {
        const entity = decodeFleetAccountEntity(raw)
        if (entity.accountRefHash !== entityId) {
          return fail("entity_key_mismatch")
        }
        return canonicalJson(encodeFleetAccountEntity(entity))
      }
      case FLEET_INBOX_FLAG_ENTITY_TYPE: {
        const entity = decodeFleetInboxFlagEntity(raw)
        if (entity.flagRef !== entityId) return fail("entity_key_mismatch")
        return canonicalJson(encodeFleetInboxFlagEntity(entity))
      }
      case FLEET_APPROVAL_ENTITY_TYPE: {
        const entity = decodeFleetApprovalEntity(raw)
        if (entity.approvalRef !== entityId) {
          return fail("entity_key_mismatch")
        }
        return canonicalJson(encodeFleetApprovalEntity(entity))
      }
      case FLEET_STEER_ENTITY_TYPE: {
        const entity = decodeFleetSteerEntity(raw)
        if (entity.steerRef !== entityId) return fail("entity_key_mismatch")
        return canonicalJson(encodeFleetSteerEntity(entity))
      }
    }
  } catch (error) {
    if (error instanceof SarahFleetProjectionReducerError) throw error
    return fail("invalid_post_image")
  }
}

const assertExactlyOneMatchingRun = (
  scope: typeof SyncScope.Type,
  entities: ReadonlyArray<SarahFleetProjectionEntity>,
): void => {
  const { runRef } = exactFleetScope(scope)
  const runRows = entities.filter(
    (entity) =>
      entity.entityType === FLEET_RUN_ENTITY_TYPE &&
      entity.postImageJson !== null,
  )
  if (runRows.length === 0) return fail("missing_run")
  if (runRows.length !== 1) return fail("multiple_runs")
  if (runRows[0]?.entityId !== runRef) return fail("foreign_scope")
}

const canonicalState = (raw: unknown): SarahFleetProjectionState => {
  if (
    serializedByteLength(raw, "invalid_state") >
    MAX_SARAH_FLEET_PROJECTION_STATE_BYTES
  ) {
    return fail("invalid_state")
  }
  const record = assertExactKeys(
    raw,
    ["schema", "scope", "cursor", "entities"],
    "invalid_state",
  )
  if (!Array.isArray(record.entities)) return fail("invalid_state")
  if (record.entities.length > MAX_SARAH_FLEET_PROJECTION_ENTITIES) {
    return fail("invalid_state")
  }
  for (const entity of record.entities) {
    assertExactKeys(
      entity,
      ["entityType", "entityId", "version", "postImageJson"],
      "invalid_state",
    )
  }
  let state: SarahFleetProjectionState
  try {
    state = Schema.decodeUnknownSync(SarahFleetProjectionState)(record)
  } catch {
    return fail("invalid_state")
  }
  const { scope } = exactFleetScope(state.scope)
  const seen = new Set<string>()
  const entities = state.entities.map((entity) => {
    if (entity.version > state.cursor) return fail("invalid_state")
    const key = keyOf(entity.entityType, entity.entityId)
    if (seen.has(key)) return fail("invalid_state")
    seen.add(key)
    return {
      ...entity,
      postImageJson:
        entity.postImageJson === null
          ? null
          : decodeAndCanonicalizePostImage(
              entity.entityType,
              entity.entityId,
              entity.postImageJson,
            ),
    }
  })
  if (state.cursor === 0 && entities.length > 0) return fail("invalid_state")
  entities.sort(compareEntities)
  assertExactlyOneMatchingRun(scope, entities)
  try {
    return Schema.decodeUnknownSync(SarahFleetProjectionState)({
      schema: SARAH_FLEET_PROJECTION_STATE_SCHEMA,
      scope,
      cursor: state.cursor,
      entities,
    })
  } catch {
    return fail("invalid_state")
  }
}

export const decodeSarahFleetProjectionState = (
  raw: unknown,
): SarahFleetProjectionState => canonicalState(raw)

const decodeBootstrapPageStrict = (raw: unknown): BootstrapResponse => {
  const record = assertExactKeys(
    raw,
    ["protocolVersion", "scope", "entities", "cursor", "nextPageToken"],
    "invalid_page_sequence",
  )
  if (!Array.isArray(record.entities)) return fail("invalid_page_sequence")
  for (const entity of record.entities) {
    assertExactKeys(
      entity,
      ["entityType", "entityId", "postImageJson"],
      "invalid_page_sequence",
    )
  }
  try {
    return decodeBootstrapResponse(record)
  } catch {
    return fail("invalid_page_sequence")
  }
}

const decodeLogPageStrict = (raw: unknown): LogPage => {
  const record = assertExactKeys(
    raw,
    ["protocolVersion", "scope", "entries", "nextCursor", "upToDate"],
    "invalid_page_sequence",
  )
  if (!Array.isArray(record.entries)) return fail("invalid_page_sequence")
  for (const entry of record.entries) {
    assertExactKeys(
      entry,
      [
        "scope",
        "version",
        "entityType",
        "entityId",
        "op",
        "postImageJson",
        "mutationRef",
        "committedAt",
      ],
      "invalid_page_sequence",
    )
  }
  try {
    return decodeLogPage(record)
  } catch {
    return fail("invalid_page_sequence")
  }
}

export const reduceSarahFleetBootstrapPages = (
  rawPages: ReadonlyArray<BootstrapResponse>,
): SarahFleetProjectionState => {
  if (
    rawPages.length === 0 ||
    rawPages.length > MAX_SARAH_FLEET_BOOTSTRAP_PAGES
  ) {
    return fail("invalid_page_sequence")
  }
  if (
    serializedByteLength(rawPages, "invalid_page_sequence") >
    MAX_SARAH_FLEET_PROJECTION_STATE_BYTES
  ) {
    return fail("invalid_page_sequence")
  }
  const rawEntityCount = rawPages.reduce((sum, rawPage) => {
    if (!isRecord(rawPage) || !Array.isArray(rawPage.entities)) {
      return fail("invalid_page_sequence")
    }
    return sum + rawPage.entities.length
  }, 0)
  if (rawEntityCount > MAX_SARAH_FLEET_PROJECTION_ENTITIES) {
    return fail("invalid_page_sequence")
  }
  const pages = rawPages.map(decodeBootstrapPageStrict)
  const { scope } = exactFleetScope(pages[0]!.scope)
  const seenTokens = new Set<string>()
  const rows = new Map<string, SarahFleetProjectionEntity>()

  for (const [index, page] of pages.entries()) {
    if (page.scope !== scope) return fail("foreign_scope")
    const isLast = index === pages.length - 1
    if (
      isLast
        ? page.cursor === undefined || page.nextPageToken !== undefined
        : page.cursor !== undefined || page.nextPageToken === undefined
    ) {
      return fail("invalid_page_sequence")
    }
    if (page.nextPageToken !== undefined) {
      if (seenTokens.has(page.nextPageToken)) {
        return fail("invalid_page_sequence")
      }
      seenTokens.add(page.nextPageToken)
    }
  }

  const cursor = pages.at(-1)?.cursor
  if (cursor === undefined) return fail("invalid_page_sequence")
  const entityCount = pages.reduce((sum, page) => sum + page.entities.length, 0)
  if (entityCount > MAX_SARAH_FLEET_PROJECTION_ENTITIES) {
    return fail("invalid_page_sequence")
  }
  if (cursor === 0 && entityCount > 0) return fail("invalid_page_sequence")

  for (const page of pages) {
    for (const entity of page.entities) {
      const postImageJson = decodeAndCanonicalizePostImage(
        entity.entityType,
        entity.entityId,
        entity.postImageJson,
      )
      const next = {
        entityType: Schema.decodeUnknownSync(SarahFleetProjectionEntityType)(
          entity.entityType,
        ),
        entityId: Schema.decodeUnknownSync(FleetPublicRef)(entity.entityId),
        version: SyncVersion.make(cursor),
        postImageJson,
      } satisfies SarahFleetProjectionEntity
      const key = keyOf(next.entityType, next.entityId)
      const existing = rows.get(key)
      if (
        existing !== undefined &&
        existing.postImageJson !== next.postImageJson
      ) {
        return fail("duplicate_conflict")
      }
      rows.set(key, next)
    }
  }

  return canonicalState({
    schema: SARAH_FLEET_PROJECTION_STATE_SCHEMA,
    scope,
    cursor,
    entities: [...rows.values()].sort(compareEntities),
  })
}

export const reduceSarahFleetLogPages = (
  rawState: SarahFleetProjectionState,
  rawPages: ReadonlyArray<LogPage>,
): SarahFleetProjectionState => {
  const state = canonicalState(rawState)
  if (rawPages.length === 0 || rawPages.length > MAX_SARAH_FLEET_LOG_PAGES) {
    return fail("invalid_page_sequence")
  }
  if (
    serializedByteLength(rawPages, "invalid_page_sequence") >
    MAX_SARAH_FLEET_PROJECTION_STATE_BYTES
  ) {
    return fail("invalid_page_sequence")
  }
  const pages = rawPages.map(decodeLogPageStrict)
  const rows = new Map(
    state.entities.map((entity) => [
      keyOf(entity.entityType, entity.entityId),
      entity,
    ]),
  )
  let cursor = state.cursor
  let previousPageCursor: number | undefined

  for (const [pageIndex, page] of pages.entries()) {
    if (page.scope !== state.scope) return fail("foreign_scope")
    const isLast = pageIndex === pages.length - 1
    if (page.upToDate !== isLast) return fail("invalid_page_sequence")
    if (page.nextCursor < state.cursor) return fail("version_regression")
    if (
      previousPageCursor !== undefined &&
      page.nextCursor < previousPageCursor
    ) {
      return fail("version_regression")
    }
    if (page.entries.length === 0) {
      const expectedCursor = previousPageCursor ?? state.cursor
      if (page.nextCursor !== expectedCursor) return fail("cursor_mismatch")
    } else if (
      Number(page.entries.at(-1)?.version) !== Number(page.nextCursor)
    ) {
      return fail("cursor_mismatch")
    }

    const pageStartCursor = previousPageCursor ?? state.cursor
    const exactReplayPage = page.nextCursor === pageStartCursor
    if (!page.upToDate && exactReplayPage) {
      return fail("invalid_page_sequence")
    }
    let previousEntryVersion = exactReplayPage ? 0 : pageStartCursor
    for (const entry of page.entries) {
      if (entry.scope !== state.scope) return fail("foreign_scope")
      if (
        entry.version < previousEntryVersion ||
        (!exactReplayPage && entry.version <= pageStartCursor)
      ) {
        return fail("version_regression")
      }
      previousEntryVersion = entry.version
      let entityType: SarahFleetProjectionEntityType
      let entityId: typeof FleetPublicRef.Type
      try {
        entityType = Schema.decodeUnknownSync(SarahFleetProjectionEntityType)(
          entry.entityType,
        )
        entityId = Schema.decodeUnknownSync(FleetPublicRef)(entry.entityId)
      } catch {
        if (!Schema.is(SarahFleetProjectionEntityType)(entry.entityType)) {
          return fail("unknown_entity_type")
        }
        return fail("invalid_post_image")
      }
      const postImageJson =
        entry.op === "delete"
          ? entry.postImageJson === undefined
            ? null
            : fail("invalid_post_image")
          : entry.postImageJson === undefined
            ? fail("invalid_post_image")
            : decodeAndCanonicalizePostImage(
                entityType,
                entityId,
                entry.postImageJson,
              )
      const key = keyOf(entityType, entityId)
      const existing = rows.get(key)
      if (entry.version <= state.cursor) {
        if (existing === undefined || entry.version !== existing.version) {
          return fail("version_regression")
        }
        if (existing.postImageJson !== postImageJson) {
          return fail("duplicate_conflict")
        }
        continue
      }
      if (existing !== undefined && entry.version === existing.version) {
        if (existing.postImageJson !== postImageJson) {
          return fail("duplicate_conflict")
        }
        continue
      }
      if (existing !== undefined && entry.version < existing.version) {
        return fail("version_regression")
      }
      rows.set(key, {
        entityType,
        entityId,
        version: entry.version,
        postImageJson,
      })
      if (rows.size > MAX_SARAH_FLEET_PROJECTION_ENTITIES) {
        return fail("invalid_state")
      }
    }
    previousPageCursor = page.nextCursor
    cursor = SyncVersionWatermark.make(Math.max(cursor, page.nextCursor))
  }

  return canonicalState({
    schema: SARAH_FLEET_PROJECTION_STATE_SCHEMA,
    scope: state.scope,
    cursor,
    entities: [...rows.values()].sort(compareEntities),
  })
}

const projectionInput = (
  state: SarahFleetProjectionState,
): Readonly<{
  run: FleetRunEntity
  workers: ReadonlyArray<FleetWorkerEntity>
  assignments: ReadonlyArray<FleetAssignmentEntity>
  approvals: ReadonlyArray<FleetApprovalEntity>
  inboxFlags: ReadonlyArray<FleetInboxFlagEntity>
}> => {
  const runs: FleetRunEntity[] = []
  const workers: FleetWorkerEntity[] = []
  const assignments: FleetAssignmentEntity[] = []
  const approvals: FleetApprovalEntity[] = []
  const inboxFlags: FleetInboxFlagEntity[] = []
  for (const entity of state.entities) {
    if (entity.postImageJson === null) continue
    const raw = JSON.parse(entity.postImageJson) as unknown
    switch (entity.entityType) {
      case FLEET_RUN_ENTITY_TYPE:
        runs.push(decodeFleetRunEntity(raw))
        break
      case FLEET_WORKER_ENTITY_TYPE:
        workers.push(decodeFleetWorkerEntity(raw))
        break
      case FLEET_ASSIGNMENT_ENTITY_TYPE:
        assignments.push(decodeFleetAssignmentEntity(raw))
        break
      case FLEET_APPROVAL_ENTITY_TYPE:
        approvals.push(decodeFleetApprovalEntity(raw))
        break
      case FLEET_INBOX_FLAG_ENTITY_TYPE:
        inboxFlags.push(decodeFleetInboxFlagEntity(raw))
        break
    }
  }
  if (runs.length === 0) return fail("missing_run")
  if (runs.length !== 1) return fail("multiple_runs")
  return { run: runs[0]!, workers, assignments, approvals, inboxFlags }
}

export const projectSarahFleetProjectionState = (
  rawState: SarahFleetProjectionState,
  projectedAtMs: number,
): SarahFleetOwnerProjection => {
  const state = canonicalState(rawState)
  if (
    !Number.isSafeInteger(projectedAtMs) ||
    projectedAtMs < 0 ||
    !Number.isFinite(projectedAtMs)
  ) {
    return fail("projection_failed")
  }
  try {
    return projectSarahFleetOwnerRun(projectionInput(state), projectedAtMs)
  } catch (error) {
    if (error instanceof SarahFleetProjectionReducerError) throw error
    return fail("projection_failed")
  }
}

export type SarahFleetProjectionPersistence = Readonly<{
  load: (scope: typeof SyncScope.Type) => Promise<unknown | null>
  save: (state: SarahFleetProjectionState) => Promise<void>
}>

export type SarahFleetProjectionOpenResult = Readonly<{
  source: "bootstrap" | "resume"
  state: SarahFleetProjectionState
  projection: SarahFleetOwnerProjection
}>

/**
 * Reopen boundary behind the cursor client. It never selects “latest”: a
 * persisted exact scope/cursor is the sole resume input, and a missing state
 * bootstraps only the explicitly requested run scope.
 */
export const makeSarahFleetProjectionStore = (input: Readonly<{
  client: Pick<SarahFleetSyncClient, "bootstrap" | "resume">
  persistence: SarahFleetProjectionPersistence
  now: () => number
}>): Readonly<{
  open: (
    scope: string,
    options?: SarahFleetSyncRequestOptions,
  ) => Promise<SarahFleetProjectionOpenResult>
}> => ({
  open: async (rawScope, options) => {
    const { scope } = exactFleetScope(rawScope)
    let loaded: unknown | null
    try {
      loaded = await input.persistence.load(scope)
    } catch (error) {
      if (error instanceof SarahFleetProjectionReducerError) throw error
      return fail("persistence_failed")
    }

    let source: SarahFleetProjectionOpenResult["source"]
    let state: SarahFleetProjectionState
    if (loaded === null) {
      source = "bootstrap"
      const result = await input.client.bootstrap(scope, options)
      state = reduceSarahFleetBootstrapPages(result.pages)
      if (
        result.state.schema !== SARAH_FLEET_CURSOR_STATE_SCHEMA ||
        result.state.scope !== state.scope ||
        result.state.cursor !== state.cursor
      ) {
        return fail("cursor_mismatch")
      }
    } else {
      source = "resume"
      const persisted = canonicalState(loaded)
      if (persisted.scope !== scope) return fail("foreign_scope")
      const result = await input.client.resume(
        {
          schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
          scope: persisted.scope,
          cursor: persisted.cursor,
        },
        options,
      )
      state = reduceSarahFleetLogPages(persisted, result.pages)
      if (
        result.state.schema !== SARAH_FLEET_CURSOR_STATE_SCHEMA ||
        result.state.scope !== state.scope ||
        result.state.cursor !== state.cursor
      ) {
        return fail("cursor_mismatch")
      }
    }

    let projectedAtMs: number
    try {
      projectedAtMs = input.now()
    } catch {
      return fail("projection_failed")
    }
    const projection = projectSarahFleetProjectionState(state, projectedAtMs)
    try {
      await input.persistence.save(state)
    } catch {
      return fail("persistence_failed")
    }
    return { source, state, projection }
  },
})
