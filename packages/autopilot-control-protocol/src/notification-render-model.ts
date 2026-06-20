import { coalesceNotifications } from "./notification-batch.js"
import { buildNotification } from "./notification-dispatch.js"
import type { NotificationPriority } from "./notification-dispatch.js"
import {
  filterByQuietHours,
  inQuietHours,
} from "./notification-quiet-hours.js"

export type NotificationRenderEvent = {
  phase: string
  sessionRef: string
  messageText: string
}

export type NotificationRenderInput = {
  events: NotificationRenderEvent[]
  hour: number
  quietStart: number
  quietEnd: number
}

export type NotificationFeedItem = {
  sessionRef: string
  title: string
  body: string
  priority: string
}

export type NotificationFeed = {
  visible: NotificationFeedItem[]
  suppressed: number
}

type RenderCandidate = NotificationFeedItem & {
  priority: NotificationPriority
}

const PRIORITY_RANK: Record<NotificationPriority, number> = {
  low: 0,
  normal: 1,
  high: 2,
}

export function buildNotificationFeed(
  input: NotificationRenderInput,
): NotificationFeed {
  const notifyable = input.events.flatMap((event): RenderCandidate[] => {
    const notification = buildNotification(event)

    if (!notification.shouldNotify) {
      return []
    }

    return [{
      sessionRef: event.sessionRef.trim().replace(/\s+/g, " "),
      title: notification.title,
      body: notification.body,
      priority: notification.priority,
    }]
  })

  const quiet = inQuietHours({
    hour: input.hour,
    startHour: input.quietStart,
    endHour: input.quietEnd,
  })
  const visibleCandidates = filterByQuietHours(notifyable, quiet)
  const grouped = coalesceNotifications(visibleCandidates).grouped

  return {
    visible: grouped.map((item) => {
      const selected = selectCandidateForGroup(visibleCandidates, item)

      return {
        sessionRef: item.sessionRef,
        title: item.title,
        body: selected?.body ?? "",
        priority: item.priority,
      }
    }),
    suppressed: input.events.length - visibleCandidates.length,
  }
}

function selectCandidateForGroup(
  candidates: RenderCandidate[],
  group: { sessionRef: string; title: string; priority: string },
): RenderCandidate | undefined {
  const groupPriority = group.priority as NotificationPriority

  return candidates.find((candidate) =>
    candidate.sessionRef === group.sessionRef &&
    candidate.title === group.title &&
    candidate.priority === groupPriority
  ) ?? candidates.find((candidate) =>
    candidate.sessionRef === group.sessionRef &&
    candidate.priority === groupPriority
  ) ?? candidates
    .filter((candidate) => candidate.sessionRef === group.sessionRef)
    .sort((a, b) => PRIORITY_RANK[b.priority] - PRIORITY_RANK[a.priority])[0]
}
