import type { KhalaSyncClientStoreErrorReason } from "../store.js"

/**
 * Typed postMessage RPC protocol between the main-thread web store proxy
 * (web/sqlite-wasm-store.ts) and the storage worker that owns the single
 * SQLite-WASM `opfs-sahpool` connection (web/sqlite-wasm-worker.ts).
 *
 * Wire shapes are structured-clone-safe plain objects (no class
 * instances, no branded constructors). The proxy encodes domain values to
 * wire before posting; the worker decodes wire back into khala-sync
 * domain values before touching the store core, and the proxy
 * reconstructs branded/domain values from response wire on the way out.
 * Requests correlate to responses by `id` (per-port monotonic counter).
 */

// ---------------------------------------------------------------------------
// Wire value shapes
// ---------------------------------------------------------------------------

export interface ChangelogEntryWire {
  readonly scope: string
  readonly version: number
  readonly entityType: string
  readonly entityId: string
  readonly op: "upsert" | "delete"
  readonly postImageJson?: string
  readonly mutationRef?: string
  readonly committedAt: string
}

export interface ConfirmedEntityWire {
  readonly entityType: string
  readonly entityId: string
  readonly postImageJson: string
  readonly version: number
}

export interface MutationEnvelopeWire {
  readonly mutationId: number
  readonly name: string
  readonly argsJson: string
}

export interface ClientIdentityWire {
  readonly clientId: string
  readonly clientGroupId: string
  readonly schemaVersion: number
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

export type StoreRequest =
  | { readonly id: number; readonly op: "cursor"; readonly scope: string }
  | {
      readonly id: number
      readonly op: "applyConfirmed"
      readonly scope: string
      readonly entries: ReadonlyArray<ChangelogEntryWire>
      readonly cursor: number
    }
  | {
      readonly id: number
      readonly op: "resetScope"
      readonly scope: string
      readonly entities: ReadonlyArray<ConfirmedEntityWire>
      /** 0 is the "scope start" watermark (clears the durable cursor). */
      readonly cursor: number
    }
  | {
      readonly id: number
      readonly op: "readEntities"
      readonly scope: string
      readonly entityType?: string
    }
  | {
      readonly id: number
      readonly op: "enqueueMutation"
      readonly mutation: MutationEnvelopeWire
    }
  | { readonly id: number; readonly op: "pendingMutations" }
  | { readonly id: number; readonly op: "lastMutationId" }
  | {
      readonly id: number
      readonly op: "ackMutations"
      readonly through: number
    }
  | { readonly id: number; readonly op: "identity" }
  | {
      readonly id: number
      readonly op: "setIdentity"
      readonly identity: ClientIdentityWire
    }

export type StoreRequestOp = StoreRequest["op"]

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown
  ? Omit<T, K>
  : never

/** A request minus its correlation id (the proxy assigns ids). */
export type StoreRequestBody = DistributiveOmit<StoreRequest, "id">

const STORE_REQUEST_OPS: ReadonlySet<string> = new Set([
  "cursor",
  "applyConfirmed",
  "resetScope",
  "readEntities",
  "enqueueMutation",
  "pendingMutations",
  "lastMutationId",
  "ackMutations",
  "identity",
  "setIdentity",
] satisfies ReadonlyArray<StoreRequestOp>)

/** Shape gate for inbound worker messages (payload fields are op-typed). */
export const isStoreRequest = (value: unknown): value is StoreRequest =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as { id?: unknown }).id === "number" &&
  typeof (value as { op?: unknown }).op === "string" &&
  STORE_REQUEST_OPS.has((value as { op: string }).op)

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

export type StoreResponseValue =
  | undefined
  | null
  | number
  | ReadonlyArray<ConfirmedEntityWire>
  | ReadonlyArray<MutationEnvelopeWire>
  | ClientIdentityWire

export type StoreResponse =
  | { readonly id: number; readonly ok: true; readonly value: StoreResponseValue }
  | {
      readonly id: number
      readonly ok: false
      readonly reason: KhalaSyncClientStoreErrorReason
      readonly message: string
    }

export const isStoreResponse = (value: unknown): value is StoreResponse =>
  typeof value === "object" &&
  value !== null &&
  typeof (value as { id?: unknown }).id === "number" &&
  typeof (value as { ok?: unknown }).ok === "boolean"
