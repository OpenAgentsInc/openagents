import type { NotificationPriority } from "./notification-dispatch.js"

export type NotificationFeedItem = {
  sessionRef: string
  priority: NotificationPriority
  at: string
}

type IndexedNotification<T extends NotificationFeedItem> = {
  item: T
  index: number
}

const PRIORITY_RANK: Record<NotificationPriority, number> = {
  low: 0,
  normal: 1,
  high: 2,
}

function timestampRank(at: string): number {
  const timestamp = Date.parse(at)

  return Number.isFinite(timestamp) ? timestamp : 0
}

function compareNotifications<T extends NotificationFeedItem>(
  a: IndexedNotification<T>,
  b: IndexedNotification<T>,
): number {
  const priorityDiff = PRIORITY_RANK[b.item.priority] - PRIORITY_RANK[a.item.priority]
  if (priorityDiff !== 0) return priorityDiff

  const timeDiff = timestampRank(b.item.at) - timestampRank(a.item.at)
  if (timeDiff !== 0) return timeDiff

  return a.index - b.index
}

export function sortNotificationFeed<T extends NotificationFeedItem>(items: T[]): T[] {
  return items
    .map((item, index) => ({ item, index }))
    .sort(compareNotifications)
    .map(({ item }) => item)
}
