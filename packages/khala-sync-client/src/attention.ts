import {
  decodeRuntimeAttentionEntity,
  RUNTIME_ATTENTION_ENTITY_TYPE,
  type RuntimeAttentionEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type { KhalaSyncClientStoreError, KhalaSyncLocalStore } from "./store.js"

export const MAX_CONFIRMED_RUNTIME_ATTENTION = 512

export type RuntimeAttentionProjectionIssue = Readonly<{
  code: "malformed" | "entity_ref_mismatch" | "owner_scope_mismatch"
  affectedRef: string
}>

export type ConfirmedRuntimeAttentionSnapshot = Readonly<{
  status: Readonly<{
    phase: ScopeSyncState["phase"]
    cursor: number | null
  }>
  pending: ReadonlyArray<RuntimeAttentionEntity>
  terminal: ReadonlyArray<RuntimeAttentionEntity>
  issues: ReadonlyArray<RuntimeAttentionProjectionIssue>
}>

export type KhalaSyncAttentionInbox = Readonly<{
  snapshot: () => Effect.Effect<ConfirmedRuntimeAttentionSnapshot, KhalaSyncClientStoreError>
}>

const empty = (state: ScopeSyncState): ConfirmedRuntimeAttentionSnapshot => ({
  status: { phase: state.phase, cursor: null },
  pending: [],
  terminal: [],
  issues: [],
})

const newestFirst = (
  left: RuntimeAttentionEntity,
  right: RuntimeAttentionEntity,
): number => right.updatedAt.localeCompare(left.updatedAt) ||
  left.attentionRef.localeCompare(right.attentionRef)

/** Confirmed-only body-free inbox over one authenticated personal scope. */
export const createKhalaSyncAttentionInbox = (input: Readonly<{
  ownerRef: string
  ownerScope: SyncScope
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
}>): KhalaSyncAttentionInbox => ({
  snapshot: () => {
    const state = input.session.state(input.ownerScope)
    if (state.phase !== "live") return Effect.succeed(empty(state))
    return Effect.map(
      input.store.readEntities(input.ownerScope, RUNTIME_ATTENTION_ENTITY_TYPE),
      rows => {
        const byRef = new Map<string, { value: RuntimeAttentionEntity; version: number }>()
        const issues: RuntimeAttentionProjectionIssue[] = []
        for (const row of rows) {
          try {
            const value = decodeRuntimeAttentionEntity(JSON.parse(row.postImageJson))
            if (value.attentionRef !== row.entityId) {
              issues.push({ code: "entity_ref_mismatch", affectedRef: row.entityId })
            } else if (value.ownerUserId !== input.ownerRef) {
              issues.push({ code: "owner_scope_mismatch", affectedRef: row.entityId })
            } else {
              const previous = byRef.get(value.attentionRef)
              if (previous === undefined || previous.version < Number(row.version)) {
                byRef.set(value.attentionRef, { value, version: Number(row.version) })
              }
            }
          } catch {
            issues.push({ code: "malformed", affectedRef: row.entityId })
          }
        }
        const items = [...byRef.values()]
          .sort((left, right) => right.version - left.version)
          .slice(0, MAX_CONFIRMED_RUNTIME_ATTENTION)
          .map(item => item.value)
          .sort(newestFirst)
        return {
          status: { phase: state.phase, cursor: Number(state.cursor) },
          pending: items.filter(item => item.status === "pending"),
          terminal: items.filter(item => item.status !== "pending"),
          issues,
        }
      },
    )
  },
})
