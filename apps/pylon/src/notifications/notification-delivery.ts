import type { NotificationPayload } from "./notification-projection.js"

export type NotificationDeliveryRef = {
  sessionRef: string
  detailRef?: string
  decisionRef?: string
}

export type MobilePushNotification = {
  title: string
  data: {
    ref: NotificationDeliveryRef
    deeplink: string
  }
}

export type DesktopNotification = {
  title: string
  ref: NotificationDeliveryRef
  deeplink: string
}

export type WebNotification = {
  title: string
  options: {
    data: {
      ref: NotificationDeliveryRef
      deeplink: string
    }
  }
}

export function toMobilePush(payload: NotificationPayload): MobilePushNotification {
  return {
    title: payload.title,
    data: {
      ref: toDeliveryRef(payload),
      deeplink: toDeeplink(payload),
    },
  }
}

export function toDesktopNotification(
  payload: NotificationPayload,
): DesktopNotification {
  return {
    title: payload.title,
    ref: toDeliveryRef(payload),
    deeplink: toDeeplink(payload),
  }
}

export function toWebNotification(payload: NotificationPayload): WebNotification {
  return {
    title: payload.title,
    options: {
      data: {
        ref: toDeliveryRef(payload),
        deeplink: toDeeplink(payload),
      },
    },
  }
}

function toDeliveryRef(payload: NotificationPayload): NotificationDeliveryRef {
  return {
    sessionRef: payload.sessionRef,
    ...(payload.detailRef ? { detailRef: payload.detailRef } : {}),
    ...(payload.decisionRef ? { decisionRef: payload.decisionRef } : {}),
  }
}

function toDeeplink(payload: NotificationPayload): string {
  const sessionPath = `/sessions/${encodeURIComponent(payload.sessionRef)}`

  if (payload.decisionRef) {
    return `openagents://pylon${sessionPath}/decisions/${encodeURIComponent(payload.decisionRef)}`
  }

  if (payload.detailRef) {
    return `openagents://pylon${sessionPath}/details/${encodeURIComponent(payload.detailRef)}`
  }

  return `openagents://pylon${sessionPath}`
}
