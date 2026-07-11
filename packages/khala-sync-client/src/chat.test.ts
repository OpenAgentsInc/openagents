import {
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncVersion,
  canonicalJson,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  personalScope,
  threadScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createChatClientMutators,
} from "./chat.js"
import { createKhalaSyncConversation } from "./conversation.js"
import { createOverlay } from "./overlay.js"
import type { KhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const NOW = "2026-07-10T20:00:00.000Z"
const OWNER = "owner.chat-client"
const THREAD = "thread.chat-client"

describe("shared canonical chat client", () => {
  test("uses canonical create/append mutators on personal and thread overlays", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const mutators = createChatClientMutators({ ownerUserId: OWNER, now: () => NOW })
      const overlay = Effect.runSync(createOverlay(store, Object.values(mutators)))
      Effect.runSync(overlay.mutate(mutators.createThread, {
        threadId: THREAD,
        title: "  Shared thread  ",
      }))
      const personal = Effect.runSync(overlay.read(personalScope(OWNER)))
      expect(personal.list("chat_thread")).toHaveLength(1)
      expect(JSON.parse(personal.list("chat_thread")[0]!.postImageJson)).toMatchObject({
        threadId: THREAD,
        ownerUserId: OWNER,
        title: "Shared thread",
      })

      Effect.runSync(overlay.mutate(mutators.appendMessage, {
        threadId: THREAD,
        messageId: "message.chat-client.1",
        body: "Hello across devices",
      }))
      const thread = Effect.runSync(overlay.read(threadScope(THREAD)))
      expect(JSON.parse(thread.list("chat_message")[0]!.postImageJson)).toMatchObject({
        messageId: "message.chat-client.1",
        threadId: THREAD,
        body: "Hello across devices",
      })
      expect(Effect.runSync(store.readEntities(personalScope(OWNER)))).toEqual([])
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("projects confirmed refs, versions and cursors without owner identity", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const mutators = createChatClientMutators({ ownerUserId: OWNER, now: () => NOW })
      const overlay = Effect.runSync(createOverlay(store, Object.values(mutators)))
      const thread = decodeChatThreadEntity({
        threadId: THREAD,
        ownerUserId: OWNER,
        title: "Confirmed thread",
        status: "active",
        messageCount: 1,
        lastMessageAt: NOW,
        createdAt: NOW,
        updatedAt: NOW,
      })
      const message = decodeChatMessageEntity({
        messageId: "message.confirmed.1",
        threadId: THREAD,
        authorUserId: OWNER,
        body: "Confirmed body",
        createdAt: NOW,
        updatedAt: NOW,
        deletedAt: null,
      })
      const entry = (
        scope: SyncScope,
        version: number,
        entityType: string,
        entityId: string,
        postImageJson: string,
      ) => new ChangelogEntry({
        scope,
        version: SyncVersion.make(version),
        entityType: EntityType.make(entityType),
        entityId: EntityId.make(entityId),
        op: "upsert",
        postImageJson,
        mutationRef: `mutation.test.${version}`,
        committedAt: NOW,
      })
      Effect.runSync(store.applyConfirmed(personalScope(OWNER), [
        entry(
          personalScope(OWNER),
          1,
          "chat_thread",
          THREAD,
          canonicalJson(encodeChatThreadEntity(thread)),
        ),
      ], SyncVersion.make(1)))
      Effect.runSync(store.applyConfirmed(threadScope(THREAD), [
        entry(
          threadScope(THREAD),
          1,
          "chat_thread",
          THREAD,
          canonicalJson(encodeChatThreadEntity(thread)),
        ),
        entry(
          threadScope(THREAD),
          2,
          "chat_message",
          message.messageId,
          canonicalJson(encodeChatMessageEntity(message)),
        ),
      ], SyncVersion.make(2)))

      const opened: Array<string> = []
      const session = {
        state: (scope: SyncScope) => ({
          phase: "live" as const,
          cursor: SyncVersion.make(scope === personalScope(OWNER) ? 1 : 2),
        }),
        pending: () => [],
        subscribe: (scope: SyncScope) => Effect.sync(() => { opened.push(String(scope)) }),
        mutate: <Args>(mutator: Parameters<typeof overlay.mutate<Args>>[0], args: Args) =>
          overlay.mutate(mutator, args),
      } as unknown as KhalaSyncSession
      const conversation = createKhalaSyncConversation({
        ownerUserId: OWNER,
        store,
        session,
        mutators,
      })
      const threads = Effect.runSync(conversation.listConfirmedThreads())
      const messages = Effect.runSync(conversation.listConfirmedMessages(THREAD))
      expect(threads).toEqual([{
        threadRef: THREAD,
        title: "Confirmed thread",
        messageCount: 1,
        lastMessageAt: NOW,
        updatedAt: NOW,
        version: 1,
      }])
      expect(messages).toEqual([{
        messageRef: "message.confirmed.1",
        threadRef: THREAD,
        body: "Confirmed body",
        createdAt: NOW,
        updatedAt: NOW,
        version: 2,
      }])
      expect(JSON.stringify({ threads, messages })).not.toContain(OWNER)
      expect(conversation.personalStatus()).toEqual({
        phase: "live",
        cursor: 1,
        pendingMutationCount: 0,
      })
      Effect.runSync(conversation.openThread(THREAD))
      expect(opened).toEqual(["scope.thread.thread.chat-client"])
    } finally {
      Effect.runSync(store.close())
    }
  })
})
