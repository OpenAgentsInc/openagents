import type { NotificationPriority } from "./notification-dispatch.js"

export type NotificationBatchItem = {
  sessionRef: string
  title: string
  priority: NotificationPriority
}

export type CoalescedNotification = {
  sessionRef: string
  count: number
  title: string
  priority: string
}

export type NotificationBatch = {
  grouped: CoalescedNotification[]
  total: number
}

const PRIORITY_RANK: Record<NotificationPriority, number> = {
  low: 0,
  normal: 1,
  high: 2,
}

export function coalesceNotifications(
  items: NotificationBatchItem[],
): NotificationBatch {
  const grouped = new Map<string, CoalescedNotification>()

  for (const item of items) {
    const existing = grouped.get(item.sessionRef)

    if (!existing) {
      grouped.set(item.sessionRef, {
        sessionRef: item.sessionRef,
        count: 1,
        title: item.title,
        priority: item.priority,
      })
      continue
    }

    existing.count += 1

    if (
      PRIORITY_RANK[item.priority] >
      PRIORITY_RANK[existing.priority as NotificationPriority]
    ) {
      existing.title = item.title
      existing.priority = item.priority
    }
  }

  return {
    grouped: Array.from(grouped.values()),
    total: items.length,
  }
}
