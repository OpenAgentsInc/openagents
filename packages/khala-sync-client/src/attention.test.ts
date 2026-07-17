import { describe, expect, test } from "vite-plus/test"
import {
  ChangelogEntry,
  EntityId,
  EntityType,
  RUNTIME_ATTENTION_ENTITY_TYPE,
  SyncVersion,
  SyncVersionWatermark,
  personalScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import { createKhalaSyncAttentionInbox } from "./attention.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const ownerRef = "owner.attention"
const scope = personalScope(ownerRef)
const at = "2026-07-17T12:00:00.000Z"
const item = (attentionRef: string, status: "pending" | "resolved") => ({
  schema: "openagents.runtime_attention.v1" as const,
  attentionRef,
  ownerUserId: ownerRef,
  interactionRef: attentionRef,
  threadRef: `thread.${attentionRef}`,
  turnRef: `turn.${attentionRef}`,
  kind: "provider_question" as const,
  status,
  requestedAt: at,
  expiresAt: "2026-07-17T13:00:00.000Z",
  updatedAt: status === "pending" ? at : "2026-07-17T12:01:00.000Z",
})
const entry = (version: number, entityId: string, value: unknown) => new ChangelogEntry({
  scope,
  version: SyncVersion.make(version),
  entityType: EntityType.make(RUNTIME_ATTENTION_ENTITY_TYPE),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImageJson: JSON.stringify(value),
  mutationRef: `mutation.attention.${version}`,
  committedAt: at,
})
const session = (state: ScopeSyncState): KhalaSyncSession => ({
  state: (_scope: SyncScope) => state,
}) as unknown as KhalaSyncSession

describe("confirmed runtime attention client", () => {
  test("splits pending and terminal confirmed rows and loss-accounts invalid authority", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "attention.pending", item("attention.pending", "pending")),
        entry(2, "attention.resolved", item("attention.resolved", "resolved")),
        entry(3, "attention.foreign", { ...item("attention.foreign", "pending"), ownerUserId: "other.owner" }),
        entry(4, "attention.bad-ref", item("attention.different", "pending")),
        entry(5, "attention.malformed", { prompt: "must not decode" }),
      ], SyncVersion.make(5)))
      const snapshot = Effect.runSync(createKhalaSyncAttentionInbox({
        ownerRef,
        ownerScope: scope,
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(5) }),
      }).snapshot())
      expect(snapshot.status).toEqual({ phase: "live", cursor: 5 })
      expect(snapshot.pending.map(value => value.attentionRef)).toEqual(["attention.pending"])
      expect(snapshot.terminal.map(value => value.attentionRef)).toEqual(["attention.resolved"])
      expect(snapshot.issues.map(value => value.code).sort()).toEqual([
        "entity_ref_mismatch", "malformed", "owner_scope_mismatch",
      ])
      expect(JSON.stringify(snapshot)).not.toContain("must not decode")
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("withholds cached attention whenever personal authority is not live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "attention.pending", item("attention.pending", "pending")),
      ], SyncVersion.make(1)))
      const snapshot = Effect.runSync(createKhalaSyncAttentionInbox({
        ownerRef,
        ownerScope: scope,
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
      }).snapshot())
      expect(snapshot).toEqual({
        status: { phase: "must_refetch", cursor: null },
        pending: [], terminal: [], issues: [],
      })
    } finally {
      Effect.runSync(store.close())
    }
  })
})
