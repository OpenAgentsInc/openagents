import { sortNotificationFeed } from "./notification-feed-sort.js"
import type { NotificationPriority } from "./notification-dispatch.js"

export type NotificationCenterInputItem = {
  sessionRef: string
  title: string
  body: string
  priority: NotificationPriority
  at: string
}

export type NotificationCenterView = {
  items: NotificationCenterInputItem[]
  unread: number
  hasHigh: boolean
}

export function buildNotificationCenter(
  items: NotificationCenterInputItem[],
): NotificationCenterView {
  return {
    items: sortNotificationFeed(items),
    unread: items.length,
    hasHigh: items.some((item) => item.priority === "high"),
  }
}
