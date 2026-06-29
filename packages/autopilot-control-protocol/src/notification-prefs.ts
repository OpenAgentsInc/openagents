export type NotificationPreferencePriority = "low" | "normal" | "high"

export type NotificationPrefs = {
  enabled: boolean
  minPriority: NotificationPreferencePriority
  quietStart: number
  quietEnd: number
}

export type NotificationPreferenceItem = {
  priority: string
}

const DEFAULT_NOTIFICATION_PREFS: NotificationPrefs = {
  enabled: true,
  minPriority: "normal",
  quietStart: 22,
  quietEnd: 7,
}

const PRIORITY_RANK: Record<NotificationPreferencePriority, number> = {
  low: 0,
  normal: 1,
  high: 2,
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

function isPriority(value: string): value is NotificationPreferencePriority {
  return Object.prototype.hasOwnProperty.call(PRIORITY_RANK, value)
}

function normalizePriority(
  value: unknown,
  fallback: NotificationPreferencePriority,
): NotificationPreferencePriority {
  if (typeof value === "string" && isPriority(value)) {
    return value
  }

  return fallback
}

function normalizeHour(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback
  }

  return Math.min(23, Math.max(0, Math.trunc(value)))
}

function priorityRank(priority: string): number | undefined {
  return isPriority(priority) ? PRIORITY_RANK[priority] : undefined
}

export function normalizeNotificationPrefs(raw: unknown): NotificationPrefs {
  if (!isRecord(raw)) {
    return { ...DEFAULT_NOTIFICATION_PREFS }
  }

  return {
    enabled: typeof raw.enabled === "boolean"
      ? raw.enabled
      : DEFAULT_NOTIFICATION_PREFS.enabled,
    minPriority: normalizePriority(
      raw.minPriority,
      DEFAULT_NOTIFICATION_PREFS.minPriority,
    ),
    quietStart: normalizeHour(raw.quietStart, DEFAULT_NOTIFICATION_PREFS.quietStart),
    quietEnd: normalizeHour(raw.quietEnd, DEFAULT_NOTIFICATION_PREFS.quietEnd),
  }
}

export function shouldDeliver(
  prefs: NotificationPrefs,
  item: NotificationPreferenceItem,
): boolean {
  const normalizedPrefs = normalizeNotificationPrefs(prefs)

  if (!normalizedPrefs.enabled) {
    return false
  }

  const itemRank = priorityRank(item.priority)
  if (itemRank === undefined) {
    return false
  }

  return itemRank >= PRIORITY_RANK[normalizedPrefs.minPriority]
}
