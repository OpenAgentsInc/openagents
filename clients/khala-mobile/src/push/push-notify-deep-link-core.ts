/**
 * MM-H3 (#8489): pure parsing for the "tap a push notification, land on the
 * right thread" behavior. The server (MM-G2, #8486,
 * `apps/openagents.com/workers/api/src/push/push-notify-events.ts`) already
 * emits `data.deepLink` as `khala://thread/<threadId>` — the SAME scheme/path
 * `AppNavigator.tsx`'s `linking` config already parses for `ThreadMessages`
 * (`ThreadMessages: "thread/:threadId"`). What was missing was a client-side
 * listener that turns a notification TAP into a `Linking.openURL(...)` call;
 * this module is the pure, defensive extraction of that URL from whatever
 * shape a real notification payload's `data` object turns out to have.
 */

const KHALA_THREAD_DEEP_LINK_PATTERN = /^khala:\/\/thread\/[^/?#]+$/

/** Extracts a safe-to-open deep link from a push notification's `data`
 * payload, or `null` if the payload is missing/malformed/not a recognized
 * Khala deep link. Deliberately narrow (exact `khala://thread/<id>` shape
 * only) so a malformed or unexpected payload never gets handed to
 * `Linking.openURL` — that API can open arbitrary URL schemes on the OS
 * level, so this is a real safety boundary, not just defensive parsing. */
export const parsePushNotificationDeepLink = (data: unknown): string | null => {
  if (data === null || typeof data !== "object") return null
  const deepLink = (data as Record<string, unknown>).deepLink
  if (typeof deepLink !== "string") return null
  const trimmed = deepLink.trim()
  return KHALA_THREAD_DEEP_LINK_PATTERN.test(trimmed) ? trimmed : null
}
