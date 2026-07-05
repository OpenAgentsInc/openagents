import { describe, expect, test } from "bun:test"

import {
  buildPushRequestBody,
  buildPushUrl,
  makeSafeRef,
  stableArgsJson
} from "../src/sync/khala-sync-push-core"

describe("buildPushUrl", () => {
  test("joins the base url without a double slash", () => {
    expect(buildPushUrl("https://openagents.com/")).toBe("https://openagents.com/api/sync/push")
    expect(buildPushUrl("https://openagents.com")).toBe("https://openagents.com/api/sync/push")
  })
})

describe("buildPushRequestBody", () => {
  test("wraps mutations with the protocol/schema version", () => {
    const body = buildPushRequestBody({
      clientGroupId: "g1",
      clientId: "c1",
      mutations: [{ argsJson: "{}", mutationId: 1, name: "chat.appendMessage" }]
    })
    expect(body.protocolVersion).toBe(1)
    expect(body.schemaVersion).toBe(1)
    expect(body.mutations).toHaveLength(1)
  })
})

describe("stableArgsJson", () => {
  test("sorts keys so the same object always serializes the same way", () => {
    expect(stableArgsJson({ b: 2, a: 1 })).toBe(stableArgsJson({ a: 1, b: 2 }))
  })

  test("drops undefined-valued keys instead of serializing them as null", () => {
    expect(stableArgsJson({ a: 1, b: undefined })).toBe('{"a":1}')
  })
})

describe("makeSafeRef", () => {
  test("produces a Khala Sync safe-ref-shaped id", () => {
    const ref = makeSafeRef("msg")
    expect(ref).toMatch(/^msg\.[a-z0-9]+$/)
  })

  test("two calls produce distinct refs", () => {
    expect(makeSafeRef("msg")).not.toBe(makeSafeRef("msg"))
  })
})
