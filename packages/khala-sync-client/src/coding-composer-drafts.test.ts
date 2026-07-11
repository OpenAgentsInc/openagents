import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  decodeCodingComposerDraftSnapshot,
  emptyComposerState,
  type CodingComposerDraftSnapshot,
} from "@openagentsinc/composer-state"
import {
  deviceLocalScope,
  LocalIdentityRef,
  LocalRevision,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import {
  CODING_COMPOSER_DRAFT_ENTITY_TYPE,
  createKhalaSyncCodingComposerDrafts,
  MAX_DEVICE_CODING_COMPOSER_DRAFT_BYTES,
} from "./coding-composer-drafts.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const ownerRef = "local_composer_fixture"
const deviceScope = deviceLocalScope(LocalIdentityRef.make(ownerRef))

const draft = (input: Readonly<{
  revision?: number
  text?: string
  owner?: string
  updatedAt?: string
}> = {}): CodingComposerDraftSnapshot => {
  const state = emptyComposerState()
  return decodeCodingComposerDraftSnapshot({
    schema: "openagents.coding_composer_draft.v1",
    draftRef: "draft.coding.fixture",
    ownerRef: input.owner ?? ownerRef,
    sessionRef: "session.coding.fixture",
    threadRef: "thread.coding.fixture",
    revision: input.revision ?? 1,
    doc: {
      ...state.doc,
      blocks: [{
        id: "block-1",
        kind: "paragraph",
        text: input.text ?? "private restart-safe prompt",
        marks: [],
      }],
    },
    selection: state.selection,
    view: {},
    context: [{
      kind: "repository",
      repositoryRef: "repository.openagents",
      revisionRef: "revision.git.fixture",
    }],
    target: {
      laneRef: "lane.codex_app_server",
      providerRef: "provider.openai",
      modelRef: "model.gpt-5.6-sol",
      readiness: "ready",
    },
    submission: { status: "editing" },
    updatedAt: input.updatedAt ?? "2026-07-11T23:00:00.000Z",
  })
}

describe("device-local coding composer drafts", () => {
  test("survives restart and fences duplicate, stale, conflict, and foreign writes", () => {
    const root = mkdtempSync(path.join(tmpdir(), "khala-sync-composer-drafts-"))
    const database = path.join(root, "drafts.sqlite")
    try {
      const initial = openKhalaSyncStore(database)
      const drafts = createKhalaSyncCodingComposerDrafts({
        store: initial,
        deviceScope,
        ownerRef,
      })
      expect(Effect.runSync(drafts.save(draft()))).toBe("saved")
      expect(Effect.runSync(drafts.save(draft()))).toBe("duplicate")
      expect(Effect.runSync(drafts.save(draft({ text: "conflicting body" })))).toBe(
        "conflict",
      )
      expect(Effect.runSync(drafts.save(draft({ owner: "local_foreign" })))).toBe(
        "owner_mismatch",
      )
      const next = draft({
        revision: 2,
        text: "new revision",
        updatedAt: "2026-07-11T23:01:00.000Z",
      })
      expect(Effect.runSync(drafts.save(next))).toBe("saved")
      expect(Effect.runSync(drafts.save(draft()))).toBe("stale")
      Effect.runSync(initial.close())

      const restarted = openKhalaSyncStore(database)
      try {
        const restored = createKhalaSyncCodingComposerDrafts({
          store: restarted,
          deviceScope,
          ownerRef,
        })
        expect(Effect.runSync(restored.load(next.draftRef))).toEqual(next)
        expect(Effect.runSync(restored.list())).toEqual([next])
      } finally {
        Effect.runSync(restarted.close())
      }
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("withholds malformed and foreign rows and bounds private payload size", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      Effect.runSync(store.writeLocalEntities(deviceScope, [
        {
          entityType: CODING_COMPOSER_DRAFT_ENTITY_TYPE,
          entityId: "draft.malformed",
          postImageJson: "not-json",
          revision: LocalRevision.make(1),
        },
        {
          entityType: CODING_COMPOSER_DRAFT_ENTITY_TYPE,
          entityId: "draft.foreign",
          postImageJson: JSON.stringify({
            ...draft({ owner: "local_foreign" }),
            draftRef: "draft.foreign",
          }),
          revision: LocalRevision.make(2),
        },
      ]))
      const drafts = createKhalaSyncCodingComposerDrafts({
        store,
        deviceScope,
        ownerRef,
      })
      expect(Effect.runSync(drafts.list())).toEqual([])
      expect(Effect.runSync(drafts.load("draft.foreign"))).toBeNull()
      expect(Effect.runSync(drafts.save(draft({
        text: "x".repeat(MAX_DEVICE_CODING_COMPOSER_DRAFT_BYTES),
      })))).toBe("too_large")
    } finally {
      Effect.runSync(store.close())
    }
  })
})
