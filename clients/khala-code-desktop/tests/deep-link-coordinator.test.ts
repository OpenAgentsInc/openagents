import { describe, expect, test } from "bun:test"

import { createKhalaCodeDeepLinkCoordinator } from "../src/bun/deep-link-coordinator"
import type { KhalaCodeDeepLinkTarget } from "../src/shared/deep-links"

describe("Khala Code deep link coordinator", () => {
  test("buffers a link until the renderer is marked ready, then flushes it", () => {
    const routed: Array<{ target: KhalaCodeDeepLinkTarget; raw: string }> = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      onRoute: (target, raw) => routed.push({ raw, target }),
    })

    coordinator.handleUrl("khala-code://thread/abc-123")

    expect(coordinator.isRendererReady()).toBe(false)
    expect(coordinator.pendingCount()).toBe(1)
    expect(routed).toHaveLength(0)

    coordinator.markRendererReady()

    expect(coordinator.isRendererReady()).toBe(true)
    expect(coordinator.pendingCount()).toBe(0)
    expect(routed).toEqual([{
      raw: "khala-code://thread/abc-123",
      target: { kind: "thread", threadId: "abc-123" },
    }])
  })

  test("routes immediately once the renderer is already ready (warm case)", () => {
    const routed: KhalaCodeDeepLinkTarget[] = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      onRoute: target => routed.push(target),
    })

    coordinator.markRendererReady()
    coordinator.handleUrl("khala-code://editor")

    expect(routed).toEqual([{ kind: "view", view: "editor" }])
    expect(coordinator.pendingCount()).toBe(0)
  })

  test("flushes several buffered links in arrival order", () => {
    const routed: KhalaCodeDeepLinkTarget[] = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      onRoute: target => routed.push(target),
    })

    coordinator.handleUrl("khala-code://chat")
    coordinator.handleUrl("khala-code://fleet")
    coordinator.handleUrl("khala-code://thread/xyz")
    expect(coordinator.pendingCount()).toBe(3)

    coordinator.markRendererReady()

    expect(routed).toEqual([
      { kind: "view", view: "chat" },
      { kind: "view", view: "fleet" },
      { kind: "thread", threadId: "xyz" },
    ])
  })

  test("an invalid link is a harmless no-op -- never buffered, never routed", () => {
    const routed: KhalaCodeDeepLinkTarget[] = []
    const events: string[] = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      onEvent: event => events.push(event.type),
      onRoute: target => routed.push(target),
    })

    coordinator.handleUrl("not-a-khala-code-link")
    coordinator.handleUrl("http://evil.example/thread/abc")

    expect(coordinator.pendingCount()).toBe(0)
    expect(routed).toHaveLength(0)
    expect(events).toEqual(["invalid", "invalid"])

    coordinator.markRendererReady()
    expect(routed).toHaveLength(0)
  })

  test("marking renderer ready twice does not re-flush or double-route", () => {
    const routed: KhalaCodeDeepLinkTarget[] = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      onRoute: target => routed.push(target),
    })

    coordinator.handleUrl("khala-code://chat")
    coordinator.markRendererReady()
    coordinator.markRendererReady()

    expect(routed).toHaveLength(1)
  })

  test("bounds buffered links so a launch storm cannot grow memory unbounded", () => {
    const routed: KhalaCodeDeepLinkTarget[] = []
    const coordinator = createKhalaCodeDeepLinkCoordinator({
      maxBuffered: 2,
      onRoute: target => routed.push(target),
    })

    coordinator.handleUrl("khala-code://thread/1")
    coordinator.handleUrl("khala-code://thread/2")
    coordinator.handleUrl("khala-code://thread/3")

    expect(coordinator.pendingCount()).toBe(2)
    coordinator.markRendererReady()

    // The oldest ("thread/1") was dropped once the buffer filled.
    expect(routed).toEqual([
      { kind: "thread", threadId: "2" },
      { kind: "thread", threadId: "3" },
    ])
  })
})
