export type NotificationRoutePayload = {
  kind: string
  priority: "high" | "normal"
}

export type NotificationChannels = {
  push: boolean
  desktop: boolean
}

export type NotificationRoute = {
  deliverTo: string[]
  suppressed: boolean
  reason: string
}

export function routeNotification(
  payload: NotificationRoutePayload,
  channels: NotificationChannels,
): NotificationRoute {
  const deliverTo = payload.priority === "high"
    ? enabledChannels(channels)
    : channels.push ? ["push"] : []

  if (deliverTo.length === 0) {
    return {
      deliverTo,
      suppressed: true,
      reason: "no eligible notification channel enabled",
    }
  }

  return {
    deliverTo,
    suppressed: false,
    reason: `${payload.priority} priority routed`,
  }
}

function enabledChannels(channels: NotificationChannels): string[] {
  const deliverTo: string[] = []

  if (channels.push) deliverTo.push("push")
  if (channels.desktop) deliverTo.push("desktop")

  return deliverTo
}
