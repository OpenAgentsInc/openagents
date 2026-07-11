import {
  decodeCodingComposerDraftSnapshot,
  type CodingComposerDraftSnapshot,
} from "@openagentsinc/composer-state"
import {
  isDeviceLocalScope,
  LocalRevision,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import {
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

export const CODING_COMPOSER_DRAFT_ENTITY_TYPE =
  "coding_composer_draft" as const
export const MAX_DEVICE_CODING_COMPOSER_DRAFTS = 128
export const MAX_DEVICE_CODING_COMPOSER_DRAFT_BYTES = 1_048_576

export type CodingComposerDraftSaveOutcome =
  | "saved"
  | "duplicate"
  | "stale"
  | "conflict"
  | "owner_mismatch"
  | "capacity_exceeded"
  | "too_large"

export type KhalaSyncCodingComposerDrafts = Readonly<{
  /** Opaque device-local owner ref required when creating a new private draft. */
  ownerRef: string
  list: () => Effect.Effect<
    ReadonlyArray<CodingComposerDraftSnapshot>,
    KhalaSyncClientStoreError
  >
  load: (
    draftRef: string,
  ) => Effect.Effect<CodingComposerDraftSnapshot | null, KhalaSyncClientStoreError>
  save: (
    draft: CodingComposerDraftSnapshot,
  ) => Effect.Effect<CodingComposerDraftSaveOutcome, KhalaSyncClientStoreError>
}>

const decodeDraft = (
  row: Readonly<{ entityId: string; postImageJson: string }>,
): CodingComposerDraftSnapshot | null => {
  try {
    const draft = decodeCodingComposerDraftSnapshot(JSON.parse(row.postImageJson))
    return draft.draftRef === row.entityId ? draft : null
  } catch {
    return null
  }
}

const decodeOwnedDraft = (
  row: Readonly<{ entityId: string; postImageJson: string }>,
  ownerRef: string,
): CodingComposerDraftSnapshot | null => {
  const draft = decodeDraft(row)
  return draft?.ownerRef === ownerRef ? draft : null
}

const compareDrafts = (
  left: CodingComposerDraftSnapshot,
  right: CodingComposerDraftSnapshot,
): number => right.updatedAt.localeCompare(left.updatedAt) ||
  left.draftRef.localeCompare(right.draftRef)

/**
 * Native-only, device-local persistence for the canonical coding draft.
 * This scope never enters hosted Sync, and malformed/foreign rows are
 * withheld instead of becoming composer authority after restart.
 */
export const createKhalaSyncCodingComposerDrafts = (input: Readonly<{
  store: KhalaSyncLocalStore
  deviceScope: SyncScope
  ownerRef: string
}>): KhalaSyncCodingComposerDrafts => {
  const readRows = () => input.store.readLocalEntities(
    input.deviceScope,
    CODING_COMPOSER_DRAFT_ENTITY_TYPE,
  )

  const requireDeviceScope = (): Effect.Effect<void, KhalaSyncClientStoreError> =>
    isDeviceLocalScope(input.deviceScope)
      ? Effect.void
      : Effect.fail(new KhalaSyncClientStoreError(
          "constraint_violation",
          "coding composer drafts require device-local authority",
        ))

  const list = (): Effect.Effect<
    ReadonlyArray<CodingComposerDraftSnapshot>,
    KhalaSyncClientStoreError
  > => Effect.gen(function*() {
    yield* requireDeviceScope()
    const rows = yield* readRows()
    return rows
      .map(row => decodeOwnedDraft(row, input.ownerRef))
      .filter((draft): draft is CodingComposerDraftSnapshot => draft !== null)
      .sort(compareDrafts)
      .slice(0, MAX_DEVICE_CODING_COMPOSER_DRAFTS)
  })

  return {
    ownerRef: input.ownerRef,
    list,
    load: draftRef => Effect.map(list(), drafts =>
      drafts.find(draft => draft.draftRef === draftRef) ?? null),
    save: draft => Effect.gen(function*() {
      yield* requireDeviceScope()
      if (draft.ownerRef !== input.ownerRef) return "owner_mismatch" as const

      const serialized = JSON.stringify(draft)
      if (new TextEncoder().encode(serialized).byteLength >
        MAX_DEVICE_CODING_COMPOSER_DRAFT_BYTES) {
        return "too_large" as const
      }

      const rows = yield* readRows()
      const existingRow = rows.find(row => row.entityId === draft.draftRef)
      if (existingRow !== undefined) {
        const existing = decodeDraft(existingRow)
        if (existing !== null) {
          if (existing.ownerRef !== input.ownerRef) return "owner_mismatch" as const
          if (existing.revision > draft.revision) return "stale" as const
          if (existing.revision === draft.revision) {
            return JSON.stringify(existing) === serialized
              ? "duplicate" as const
              : "conflict" as const
          }
        }
        // A valid newer local revision repairs a malformed/pre-contract row.
      } else {
        const ownedCount = rows.filter(row =>
          decodeOwnedDraft(row, input.ownerRef) !== null).length
        if (ownedCount >= MAX_DEVICE_CODING_COMPOSER_DRAFTS) {
          return "capacity_exceeded" as const
        }
      }

      const localRevision = LocalRevision.make(
        Math.max(0, ...rows.map(row => Number(row.revision))) + 1,
      )
      yield* input.store.writeLocalEntities(input.deviceScope, [{
        entityType: CODING_COMPOSER_DRAFT_ENTITY_TYPE,
        entityId: draft.draftRef,
        postImageJson: serialized,
        revision: localRevision,
      }])
      return "saved" as const
    }),
  }
}
