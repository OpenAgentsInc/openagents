import {
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import {
  type KhalaSyncStoreCore,
  toKhalaSyncStoreError,
} from "../store-core.js"
import type { ClientIdentity, ConfirmedEntity } from "../store.js"
import {
  type ChangelogEntryWire,
  type ClientIdentityWire,
  type ConfirmedEntityWire,
  isStoreRequest,
  type MutationEnvelopeWire,
  type StoreRequest,
  type StoreResponse,
  type StoreResponseValue,
} from "./protocol.js"

/**
 * Storage-worker RPC server (KS-5.4): decodes {@link StoreRequest} wire
 * values into khala-sync domain values, dispatches to the driver-agnostic
 * {@link KhalaSyncStoreCore}, and encodes results / typed errors back to
 * wire. Pure request→response — no port, timer, or sqlite specifics —
 * so the full semantics are testable in bun with `bun:sqlite` behind the
 * core.
 */

export interface KhalaSyncStoreWorkerServer {
  readonly handle: (request: unknown) => StoreResponse
}

// -- wire → domain -----------------------------------------------------------

const decodeEntry = (wire: ChangelogEntryWire): ChangelogEntry =>
  new ChangelogEntry({
    scope: SyncScope.make(wire.scope),
    version: SyncVersion.make(wire.version),
    entityType: EntityType.make(wire.entityType),
    entityId: EntityId.make(wire.entityId),
    op: wire.op,
    ...(wire.postImageJson === undefined
      ? {}
      : { postImageJson: wire.postImageJson }),
    ...(wire.mutationRef === undefined
      ? {}
      : { mutationRef: wire.mutationRef }),
    committedAt: wire.committedAt,
  })

const decodeEntity = (wire: ConfirmedEntityWire): ConfirmedEntity => ({
  entityType: wire.entityType,
  entityId: wire.entityId,
  postImageJson: wire.postImageJson,
  version: SyncVersion.make(wire.version),
})

const decodeMutation = (wire: MutationEnvelopeWire): MutationEnvelope =>
  new MutationEnvelope({
    mutationId: MutationId.make(wire.mutationId),
    name: MutatorName.make(wire.name),
    argsJson: wire.argsJson,
  })

const decodeIdentity = (wire: ClientIdentityWire): ClientIdentity => ({
  clientId: ClientId.make(wire.clientId),
  clientGroupId: ClientGroupId.make(wire.clientGroupId),
  schemaVersion: SyncSchemaVersion.make(wire.schemaVersion),
})

// -- domain → wire -----------------------------------------------------------

const encodeEntity = (entity: ConfirmedEntity): ConfirmedEntityWire => ({
  entityType: entity.entityType,
  entityId: entity.entityId,
  postImageJson: entity.postImageJson,
  version: entity.version,
})

const encodeMutation = (mutation: MutationEnvelope): MutationEnvelopeWire => ({
  mutationId: mutation.mutationId,
  name: mutation.name,
  argsJson: mutation.argsJson,
})

// -- dispatch ------------------------------------------------------------------

const dispatch = (
  core: KhalaSyncStoreCore,
  request: StoreRequest,
): StoreResponseValue => {
  switch (request.op) {
    case "cursor":
      return core.cursor(SyncScope.make(request.scope))
    case "applyConfirmed": {
      core.applyConfirmed(
        SyncScope.make(request.scope),
        request.entries.map(decodeEntry),
        SyncVersion.make(request.cursor),
      )
      return undefined
    }
    case "resetScope": {
      core.resetScope(
        SyncScope.make(request.scope),
        request.entities.map(decodeEntity),
        request.cursor === 0
          ? SyncVersionWatermark.make(0)
          : SyncVersion.make(request.cursor),
      )
      return undefined
    }
    case "readEntities":
      return core
        .readEntities(SyncScope.make(request.scope), request.entityType)
        .map(encodeEntity)
    case "enqueueMutation": {
      core.enqueueMutation(decodeMutation(request.mutation))
      return undefined
    }
    case "pendingMutations":
      return core.pendingMutations().map(encodeMutation)
    case "lastMutationId":
      return core.lastMutationId()
    case "ackMutations": {
      core.ackMutations(MutationId.make(request.through))
      return undefined
    }
    case "identity": {
      const identity = core.identity()
      return identity === null
        ? null
        : {
            clientId: identity.clientId,
            clientGroupId: identity.clientGroupId,
            schemaVersion: identity.schemaVersion,
          }
    }
    case "setIdentity": {
      core.setIdentity(decodeIdentity(request.identity))
      return undefined
    }
  }
}

/** Correlation id for error replies to malformed frames (proxy ignores it). */
export const MALFORMED_REQUEST_ID = -1

export const createKhalaSyncStoreWorkerServer = (
  core: KhalaSyncStoreCore,
): KhalaSyncStoreWorkerServer => ({
  handle: (request) => {
    if (!isStoreRequest(request)) {
      return {
        id: MALFORMED_REQUEST_ID,
        ok: false,
        reason: "storage_failure",
        message: "malformed khala-sync store request frame",
      }
    }
    try {
      return { id: request.id, ok: true, value: dispatch(core, request) }
    } catch (error) {
      const storeError = toKhalaSyncStoreError(error)
      return {
        id: request.id,
        ok: false,
        reason: storeError.reason,
        message: storeError.message,
      }
    }
  },
})
