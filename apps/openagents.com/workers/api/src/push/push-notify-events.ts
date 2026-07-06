// Notify-event -> push-notification payload mapping (MM-G2, #8486).
//
// PIN (Lane 0/#8473-#8477, #8479 not yet merged as of this writing): the
// cloud-execution spine and metering that will call
// `POST /api/internal/push/notify-events` (this module's route counterpart)
// do not exist on `main` yet. This module is built against the DOCUMENTED
// intended shape from the issue bodies — turn completed (with branch/PR link
// when #8477 produced one), turn needs input/approval, turn failed, credit
// low/exhausted (from #8479) — with a typed `RuntimeNotifyEvent` any future
// caller can construct once those land. See the route file for the same pin
// noted against the ingest auth boundary.
//
// PAYLOAD SAFETY (the issue's hard requirement): titles/bodies are drawn from
// a SMALL FIXED set of templated strings keyed only by `kind` and a couple of
// booleans (hasBranch/hasPr/exhausted) — NEVER interpolated from threadId,
// turnId, ownerUserId, or any other caller-supplied string. Those identifiers
// only ever flow into the `data` field (for client-side deep-linking), never
// into visible title/body text. This is what makes "never contains repo
// code, prompts, or secrets" a structural guarantee instead of a hope — see
// `push-notify-events.test.ts`'s payload-safety oracle, which fuzzes
// threadId/turnId with adversarial strings and asserts they never appear in
// title/body.

export type RuntimeNotifyEventKind =
  | 'turn_completed'
  | 'turn_needs_input'
  | 'turn_failed'
  | 'credit_low'

export type RuntimeNotifyEvent = Readonly<{
  kind: RuntimeNotifyEventKind
  ownerUserId: string
  threadId: string
  turnId?: string
  /** Set when #8477 (branch/PR writeback) produced one. Only ever used to
   * flip WHICH fixed template string is picked — the URL itself is public
   * GitHub content the user already owns, not "repo code" or a prompt. */
  branchUrl?: string
  prUrl?: string
  /** credit_low only: true once the balance is fully exhausted, not just low. */
  exhausted?: boolean
}>

export type PushNotificationPayload = Readonly<{
  title: string
  body: string
  data: Readonly<{
    kind: RuntimeNotifyEventKind
    threadId: string
    turnId?: string
    deepLink: string
    url?: string
  }>
}>

/** `khala://thread/<threadId>` — the app's own URL scheme (see
 * `clients/khala-mobile/app.json`'s `expo.scheme`), a NEW convention this
 * issue introduces for notification-tap deep-linking. Client-side tap
 * handling (subscribing to the notification-response listener and
 * navigating on it) is NOT wired by this server-only issue — see the
 * route/issue closing comment for that honest gap. */
export const runtimeNotifyDeepLink = (threadId: string): string =>
  `khala://thread/${encodeURIComponent(threadId)}`

const TURN_COMPLETED_BODIES = {
  branch: 'Your task finished — a branch is ready to review.',
  plain: 'Your task finished.',
  pr: 'Your task finished — a pull request is ready to review.',
} as const

export const buildNotificationPayload = (
  event: RuntimeNotifyEvent,
): PushNotificationPayload => {
  const data = {
    deepLink: runtimeNotifyDeepLink(event.threadId),
    kind: event.kind,
    threadId: event.threadId,
    ...(event.turnId === undefined ? {} : { turnId: event.turnId }),
  }

  switch (event.kind) {
    case 'turn_completed': {
      const url = event.prUrl ?? event.branchUrl
      const body =
        event.prUrl !== undefined
          ? TURN_COMPLETED_BODIES.pr
          : event.branchUrl !== undefined
            ? TURN_COMPLETED_BODIES.branch
            : TURN_COMPLETED_BODIES.plain
      return {
        body,
        data: url === undefined ? data : { ...data, url },
        title: 'Task finished',
      }
    }
    case 'turn_needs_input':
      return {
        body: 'Your task needs your input to continue.',
        data,
        title: 'Needs your input',
      }
    case 'turn_failed':
      return {
        body: 'Your task ran into a problem and stopped.',
        data,
        title: 'Task failed',
      }
    case 'credit_low':
      return event.exhausted === true
        ? {
            body: "You're out of credits — buy more to keep running tasks.",
            data,
            title: 'Credits exhausted',
          }
        : {
            body: 'Your credit balance is running low.',
            data,
            title: 'Credits running low',
          }
  }
}

/** Public-safe strings this module is allowed to ever emit as title/body —
 * the payload-safety oracle asserts every produced payload's title/body is a
 * MEMBER of this set, never a novel interpolated string. */
export const KNOWN_SAFE_NOTIFICATION_TITLES: ReadonlySet<string> = new Set([
  'Task finished',
  'Needs your input',
  'Task failed',
  'Credits running low',
  'Credits exhausted',
])

export const KNOWN_SAFE_NOTIFICATION_BODIES: ReadonlySet<string> = new Set([
  TURN_COMPLETED_BODIES.plain,
  TURN_COMPLETED_BODIES.branch,
  TURN_COMPLETED_BODIES.pr,
  'Your task needs your input to continue.',
  'Your task ran into a problem and stopped.',
  'Your credit balance is running low.',
  "You're out of credits — buy more to keep running tasks.",
])
