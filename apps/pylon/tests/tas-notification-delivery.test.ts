import { describe, expect, test } from "bun:test"
import {
  toDesktopNotification,
  toMobilePush,
  toWebNotification,
} from "../src/notifications/notification-delivery"
import type { NotificationPayload } from "../src/notifications/notification-projection"

const payload: NotificationPayload = {
  kind: "decision_requested",
  title: "Decision requested",
  sessionRef: "session.fixture.0001",
  detailRef: "action.fixture.approve",
  decisionRef: "decision.fixture.req01",
}

const expectedRef = {
  sessionRef: "session.fixture.0001",
  detailRef: "action.fixture.approve",
  decisionRef: "decision.fixture.req01",
}

const expectedDeeplink =
  "openagents://pylon/sessions/session.fixture.0001/decisions/decision.fixture.req01"

describe("notification delivery mapping", () => {
  test("maps payloads to mobile push with title, ref, and deeplink", () => {
    const push = toMobilePush(payload)

    expect(push).toEqual({
      title: "Decision requested",
      data: {
        ref: expectedRef,
        deeplink: expectedDeeplink,
      },
    })
  })

  test("maps payloads to desktop notifications with title, ref, and deeplink", () => {
    const desktop = toDesktopNotification(payload)

    expect(desktop).toEqual({
      title: "Decision requested",
      ref: expectedRef,
      deeplink: expectedDeeplink,
    })
  })

  test("maps payloads to web notifications with title, ref, and deeplink", () => {
    const web = toWebNotification(payload)

    expect(web).toEqual({
      title: "Decision requested",
      options: {
        data: {
          ref: expectedRef,
          deeplink: expectedDeeplink,
        },
      },
    })
  })

  test("delivery mappings stay refs-only", () => {
    const rawContent = "private user prompt with local file paths"
    const unsafePayload = {
      ...payload,
      rawContent,
      body: rawContent,
      content: rawContent,
    } as NotificationPayload

    expect(JSON.stringify(toMobilePush(unsafePayload))).not.toContain(rawContent)
    expect(JSON.stringify(toDesktopNotification(unsafePayload))).not.toContain(rawContent)
    expect(JSON.stringify(toWebNotification(unsafePayload))).not.toContain(rawContent)
  })
})
