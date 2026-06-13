import { describe, expect, test } from "bun:test"
import { routeNotification } from "../src/node/notification-router"

describe("node notification router", () => {
  test("routes high-priority notifications to all enabled channels", () => {
    expect(routeNotification(
      { kind: "decision_required", priority: "high" },
      { push: true, desktop: true },
    )).toEqual({
      deliverTo: ["push", "desktop"],
      suppressed: false,
      reason: "high priority routed",
    })
  })

  test("routes high-priority notifications to push when only push is enabled", () => {
    expect(routeNotification(
      { kind: "session_failed", priority: "high" },
      { push: true, desktop: false },
    )).toEqual({
      deliverTo: ["push"],
      suppressed: false,
      reason: "high priority routed",
    })
  })

  test("routes high-priority notifications to desktop when only desktop is enabled", () => {
    expect(routeNotification(
      { kind: "session_failed", priority: "high" },
      { push: false, desktop: true },
    )).toEqual({
      deliverTo: ["desktop"],
      suppressed: false,
      reason: "high priority routed",
    })
  })

  test("routes normal-priority notifications to push only", () => {
    expect(routeNotification(
      { kind: "session_completed", priority: "normal" },
      { push: true, desktop: true },
    )).toEqual({
      deliverTo: ["push"],
      suppressed: false,
      reason: "normal priority routed",
    })
  })

  test("suppresses normal-priority notifications when push is disabled", () => {
    expect(routeNotification(
      { kind: "session_completed", priority: "normal" },
      { push: false, desktop: true },
    )).toEqual({
      deliverTo: [],
      suppressed: true,
      reason: "no eligible notification channel enabled",
    })
  })

  test("suppresses notifications when no eligible channel is enabled", () => {
    expect(routeNotification(
      { kind: "attention", priority: "high" },
      { push: false, desktop: false },
    )).toEqual({
      deliverTo: [],
      suppressed: true,
      reason: "no eligible notification channel enabled",
    })
  })
})
