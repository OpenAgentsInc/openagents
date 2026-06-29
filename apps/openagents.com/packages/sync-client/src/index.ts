import type { SyncPatch } from '@openagentsinc/sync-schema'

export type CollectionRecords = Readonly<Record<string, unknown>>

export type SyncCollections = Readonly<Record<string, CollectionRecords>>

export type CollectionsByScope = Readonly<Record<string, SyncCollections>>

export type PendingMutation = Readonly<{
  mutationId: string
  scope: string
  command: string
  payload: unknown
}>

export type SyncClientState = Readonly<{
  cursorsByScope: Readonly<Record<string, number>>
  collectionsByScope: CollectionsByScope
  pendingMutations: Readonly<Record<string, PendingMutation>>
}>

export const emptySyncClientState: SyncClientState = {
  cursorsByScope: {},
  collectionsByScope: {},
  pendingMutations: {},
}

const withoutRecordKey = <Value>(
  record: Readonly<Record<string, Value>>,
  keyToRemove: string,
): Readonly<Record<string, Value>> =>
  Object.fromEntries(
    Object.entries(record).filter(([key]) => key !== keyToRemove),
  )

const isRecord = (value: unknown): value is CollectionRecords =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const applyCollectionPatch = (
  collection: CollectionRecords,
  patch: SyncPatch,
): CollectionRecords => {
  if (patch.op === 'delete' || patch.op === 'invalidate') {
    return withoutRecordKey(collection, patch.id)
  }

  if (patch.op === 'patch') {
    const previous = collection[patch.id]
    const previousRecord = isRecord(previous) ? previous : {}
    const patchRecord = isRecord(patch.patch) ? patch.patch : {}

    return {
      ...collection,
      [patch.id]: {
        ...previousRecord,
        ...patchRecord,
      },
    }
  }

  return {
    ...collection,
    [patch.id]: patch.value,
  }
}

export const applySyncPatch = (
  state: SyncClientState,
  patch: SyncPatch,
): SyncClientState => {
  const scopeCollections = state.collectionsByScope[patch.scope] ?? {}
  const collection = scopeCollections[patch.collection] ?? {}
  const pendingMutations =
    patch.mutationId === undefined
      ? state.pendingMutations
      : withoutRecordKey(state.pendingMutations, patch.mutationId)

  return {
    cursorsByScope: {
      ...state.cursorsByScope,
      [patch.scope]: patch.seq,
    },
    collectionsByScope: {
      ...state.collectionsByScope,
      [patch.scope]: {
        ...scopeCollections,
        [patch.collection]: applyCollectionPatch(collection, patch),
      },
    },
    pendingMutations,
  }
}

export const collectionsForScope = (
  state: SyncClientState,
  scope: string,
): SyncCollections => state.collectionsByScope[scope] ?? {}

export const cursorForScope = (state: SyncClientState, scope: string): number =>
  state.cursorsByScope[scope] ?? 0
