import { describe, expect, test } from "bun:test"

import {
  buildBootstrapRequestBody,
  buildBootstrapUrl,
  buildConnectUrl,
  chatFeedScope,
  makeFeedEvent
} from "../src/sync/khala-chat-feed-core"

describe("Khala mobile chat feed wire helpers", () => {
  test("chatFeedScope builds the thread scope", () => {
    expect(String(chatFeedScope("thread_demo"))).toBe("scope.thread.thread_demo")
  })

  test("buildBootstrapRequestBody matches the BootstrapRequest wire shape", () => {
    const scope = chatFeedScope("thread_demo")
    expect(buildBootstrapRequestBody(scope, "group-1")).toEqual({
      clientGroupId: "group-1",
      protocolVersion: 1,
      schemaVersion: 1,
      scope: "scope.thread.thread_demo"
    })
  })

  test("buildBootstrapUrl joins the base url", () => {
    expect(buildBootstrapUrl("https://openagents.com/")).toBe(
      "https://openagents.com/api/sync/bootstrap"
    )
  })

  test("buildConnectUrl produces a wss url carrying scope + cursor", () => {
    const scope = chatFeedScope("thread_demo")
    expect(buildConnectUrl("https://openagents.com", scope, 3)).toBe(
      "wss://openagents.com/api/sync/connect?scope=scope.thread.thread_demo&cursor=3"
    )
  })

  test("buildConnectUrl falls back to ws for a non-https base", () => {
    const scope = chatFeedScope("thread_demo")
    expect(buildConnectUrl("http://127.0.0.1:8787", scope, 0)).toBe(
      "ws://127.0.0.1:8787/api/sync/connect?scope=scope.thread.thread_demo&cursor=0"
    )
  })

  test("makeFeedEvent stringifies the payload as raw JSON", () => {
    const event = makeFeedEvent("frame", { hello: "world" }, "2026-07-05T00:00:00.000Z", 1)
    expect(event.id).toBe("frame-1")
    expect(event.raw).toBe('{\n  "hello": "world"\n}')
  })
})
