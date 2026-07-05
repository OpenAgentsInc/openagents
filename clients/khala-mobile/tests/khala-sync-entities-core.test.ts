import { describe, expect, test } from "bun:test"

import {
  applyDeltaFrameOfType,
  buildBootstrapRequestBody,
  buildBootstrapUrl,
  buildConnectUrl,
  entitiesOfType,
  sortByKeyAsc,
  sortByKeyDesc
} from "../src/sync/khala-sync-entities-core"

type Item = Readonly<{ id: string; body: string; at: string }>
const decode = (value: unknown): Item => value as Item
const idOf = (item: Item): string => item.id

describe("Khala Sync entity wire helpers", () => {
  test("buildBootstrapUrl joins the base url", () => {
    expect(buildBootstrapUrl("https://openagents.com/")).toBe(
      "https://openagents.com/api/sync/bootstrap"
    )
  })

  test("buildBootstrapRequestBody matches the wire shape", () => {
    expect(buildBootstrapRequestBody("scope.user.u1", "group-1")).toEqual({
      clientGroupId: "group-1",
      protocolVersion: 1,
      schemaVersion: 1,
      scope: "scope.user.u1"
    })
  })

  test("buildConnectUrl carries scope + cursor and maps https to wss", () => {
    expect(buildConnectUrl("https://openagents.com", "scope.thread.t1", 3)).toBe(
      "wss://openagents.com/api/sync/connect?scope=scope.thread.t1&cursor=3"
    )
  })

  test("buildConnectUrl maps http to ws", () => {
    expect(buildConnectUrl("http://127.0.0.1:8787", "scope.thread.t1", 0)).toBe(
      "ws://127.0.0.1:8787/api/sync/connect?scope=scope.thread.t1&cursor=0"
    )
  })

  test("entitiesOfType filters and decodes only the requested entity type", () => {
    const rows = [
      { entityId: "t1", entityType: "chat_thread", postImageJson: JSON.stringify({ id: "t1", body: "thread", at: "2026-01-01T00:00:00Z" }) },
      { entityId: "m1", entityType: "chat_message", postImageJson: JSON.stringify({ id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" }) }
    ]
    expect(entitiesOfType(rows, "chat_message", decode)).toEqual([
      { id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" }
    ])
  })

  test("applyDeltaFrameOfType upserts a new item", () => {
    const current: ReadonlyArray<Item> = []
    const frame = {
      _tag: "DeltaFrame",
      entries: [
        {
          entityId: "m1",
          entityType: "chat_message",
          op: "upsert",
          postImageJson: JSON.stringify({ id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" })
        }
      ]
    }
    expect(applyDeltaFrameOfType(current, frame, "chat_message", idOf, decode)).toEqual([
      { id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" }
    ])
  })

  test("applyDeltaFrameOfType replaces an existing item by id", () => {
    const current: ReadonlyArray<Item> = [{ id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" }]
    const frame = {
      _tag: "DeltaFrame",
      entries: [
        {
          entityId: "m1",
          entityType: "chat_message",
          op: "upsert",
          postImageJson: JSON.stringify({ id: "m1", body: "edited", at: "2026-01-01T00:00:02Z" })
        }
      ]
    }
    expect(applyDeltaFrameOfType(current, frame, "chat_message", idOf, decode)).toEqual([
      { id: "m1", body: "edited", at: "2026-01-01T00:00:02Z" }
    ])
  })

  test("applyDeltaFrameOfType removes an item on delete", () => {
    const current: ReadonlyArray<Item> = [{ id: "m1", body: "hi", at: "2026-01-01T00:00:01Z" }]
    const frame = {
      _tag: "DeltaFrame",
      entries: [{ entityId: "m1", entityType: "chat_message", op: "delete" }]
    }
    expect(applyDeltaFrameOfType(current, frame, "chat_message", idOf, decode)).toEqual([])
  })

  test("applyDeltaFrameOfType ignores entries of a different entity type", () => {
    const current: ReadonlyArray<Item> = []
    const frame = {
      _tag: "DeltaFrame",
      entries: [
        {
          entityId: "t1",
          entityType: "chat_thread",
          op: "upsert",
          postImageJson: JSON.stringify({ id: "t1", body: "thread", at: "2026-01-01T00:00:00Z" })
        }
      ]
    }
    expect(applyDeltaFrameOfType(current, frame, "chat_message", idOf, decode)).toEqual([])
  })

  test("sortByKeyAsc / sortByKeyDesc order by ISO timestamp string", () => {
    const items: ReadonlyArray<Item> = [
      { id: "a", body: "", at: "2026-01-02T00:00:00Z" },
      { id: "b", body: "", at: "2026-01-01T00:00:00Z" }
    ]
    expect(sortByKeyAsc(items, i => i.at).map(i => i.id)).toEqual(["b", "a"])
    expect(sortByKeyDesc(items, i => i.at).map(i => i.id)).toEqual(["a", "b"])
  })
})
