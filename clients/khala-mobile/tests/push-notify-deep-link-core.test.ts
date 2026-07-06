import { describe, expect, test } from "bun:test"

import { parsePushNotificationDeepLink } from "../src/push/push-notify-deep-link-core"

// Oracle for khala_mobile.push.notification_tap_opens_thread.v1
describe("contract khala_mobile.push.notification_tap_opens_thread.v1", () => {
  test("notification_tap_opens_thread_deep_link.unit — extracts a well-formed khala://thread/<id> deep link", () => {
    expect(parsePushNotificationDeepLink({ deepLink: "khala://thread/thread_123", threadId: "thread_123" })).toBe(
      "khala://thread/thread_123",
    )
  })

  test("trims surrounding whitespace", () => {
    expect(parsePushNotificationDeepLink({ deepLink: "  khala://thread/thread_123  " })).toBe(
      "khala://thread/thread_123",
    )
  })

  test("rejects a missing data payload", () => {
    expect(parsePushNotificationDeepLink(null)).toBeNull()
    expect(parsePushNotificationDeepLink(undefined)).toBeNull()
  })

  test("rejects a payload with no deepLink field", () => {
    expect(parsePushNotificationDeepLink({ threadId: "thread_123" })).toBeNull()
  })

  test("rejects a non-string deepLink", () => {
    expect(parsePushNotificationDeepLink({ deepLink: 12345 })).toBeNull()
  })

  test("rejects an unrecognized URL scheme (never hands an arbitrary scheme to Linking.openURL)", () => {
    expect(parsePushNotificationDeepLink({ deepLink: "https://evil.example.com/phish" })).toBeNull()
    expect(parsePushNotificationDeepLink({ deepLink: "javascript:alert(1)" })).toBeNull()
  })

  test("rejects a khala:// URL that isn't the thread path shape", () => {
    expect(parsePushNotificationDeepLink({ deepLink: "khala://auth" })).toBeNull()
    expect(parsePushNotificationDeepLink({ deepLink: "khala://thread/" })).toBeNull()
    expect(parsePushNotificationDeepLink({ deepLink: "khala://thread/abc/extra" })).toBeNull()
  })
})
