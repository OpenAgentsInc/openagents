import { describe, expect, test } from "bun:test"

import {
  khalaCodeDeepLinkFromLocation,
  parseKhalaCodeDeepLink,
  viewForKhalaCodeDeepLinkTarget,
} from "../src/shared/deep-links"

describe("Khala Code deep links", () => {
  test("parses view, thread, project, and server targets", () => {
    expect(parseKhalaCodeDeepLink("khala-code://view/settings")).toEqual({
      ok: true,
      target: { kind: "view", view: "settings" },
      url: "khala-code://view/settings",
    })
    expect(parseKhalaCodeDeepLink("khala-code://thread/thread-123")).toEqual({
      ok: true,
      target: { kind: "thread", threadId: "thread-123" },
      url: "khala-code://thread/thread-123",
    })
    expect(parseKhalaCodeDeepLink("khala-code://open?project=project.alpha")).toEqual({
      ok: true,
      target: { kind: "project", projectId: "project.alpha" },
      url: "khala-code://open?project=project.alpha",
    })
    expect(viewForKhalaCodeDeepLinkTarget({ kind: "server", serverId: "local" })).toBe("settings")
  })

  test("rejects invalid schemes, unknown views, and private path-like ids", () => {
    expect(parseKhalaCodeDeepLink("https://openagents.com")).toEqual({
      error: "Unsupported deep-link scheme.",
      ok: false,
      url: "https://openagents.com",
    })
    expect(parseKhalaCodeDeepLink("khala-code://view/not-real")).toEqual({
      error: "Unsupported view target.",
      ok: false,
      url: "khala-code://view/not-real",
    })
    expect(parseKhalaCodeDeepLink("khala-code://project/%2FUsers%2Fme%2Fsecret")).toEqual({
      error: "Invalid project id.",
      ok: false,
      url: "khala-code://project/%2FUsers%2Fme%2Fsecret",
    })
  })

  test("extracts encoded app links from location search or hash", () => {
    expect(khalaCodeDeepLinkFromLocation({
      hash: "",
      href: "http://localhost/?khala-code-url=khala-code%3A%2F%2Fview%2Freview",
      protocol: "http:",
      search: "?khala-code-url=khala-code%3A%2F%2Fview%2Freview",
    })?.ok).toBe(true)
    expect(khalaCodeDeepLinkFromLocation({
      hash: "#khala-code-url=khala-code%3A%2F%2Fthread%2Fthread-1",
      href: "http://localhost/#khala-code-url=khala-code%3A%2F%2Fthread%2Fthread-1",
      protocol: "http:",
      search: "",
    })).toEqual({
      ok: true,
      target: { kind: "thread", threadId: "thread-1" },
      url: "khala-code://thread/thread-1",
    })
  })
})
