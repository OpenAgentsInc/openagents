import {
  type NotificationCenterInputItem,
  type NotificationPermissionInput,
  type NotificationPermissionProjection,
  type NotificationSession,
  projectNotificationPermission,
} from '@openagentsinc/autopilot-control-protocol'
import { Schema as S } from 'effect'

import type { AgentRunStatus, ChatRun, Model } from '../model'

/**
 * Web-side notifications state for Pylon sessions.
 *
 * The shared cores in `@openagentsinc/autopilot-control-protocol` own all of the
 * derivation logic:
 *
 * - `notificationsFromSessions` turns a session list + a seen-set into the new
 *   notifications that should fire (see `transitions.ts`).
 * - `projectNotificationPermission` models the browser permission lifecycle.
 * - `buildNotificationCenter` builds the in-app feed (see `view.ts`).
 *
 * This module only stores the persisted bits (seen-set, accumulated feed items,
 * raw permission input) and projects the permission state for the view.
 */

export const NotificationPriority = S.Literals(['low', 'normal', 'high'])
export type NotificationPriority = typeof NotificationPriority.Type

export const NotificationCenterItem = S.Struct({
  sessionRef: S.String,
  title: S.String,
  body: S.String,
  priority: NotificationPriority,
  at: S.String,
})
export type NotificationCenterItem = typeof NotificationCenterItem.Type

export const NotificationsModel = S.Struct({
  /** `${sessionRef}:${state}` keys already observed; passed to the core. */
  seenRefs: S.Array(S.String),
  /** Accumulated in-app feed items, oldest first. */
  items: S.Array(NotificationCenterItem),
  /** Raw permission signal mirrored from the browser Notification API. */
  permissionGranted: S.Boolean,
  permissionCanAskAgain: S.Boolean,
})
export type NotificationsModel = typeof NotificationsModel.Type

export const initNotifications = (): NotificationsModel => ({
  seenRefs: [],
  items: [],
  permissionGranted: false,
  permissionCanAskAgain: true,
})

export const notificationPermissionInput = (
  notifications: NotificationsModel,
): NotificationPermissionInput => ({
  granted: notifications.permissionGranted,
  canAskAgain: notifications.permissionCanAskAgain,
  // The web client does not hold a push token; in-app + browser-notification
  // delivery only needs the granted/canAskAgain signal. Surface a sentinel
  // token when granted so the projection reaches the `enabled` state.
  pushToken: notifications.permissionGranted ? 'web-local' : null,
})

export const notificationPermissionProjection = (
  notifications: NotificationsModel,
): NotificationPermissionProjection =>
  projectNotificationPermission(notificationPermissionInput(notifications))

/**
 * Map the web app's `AgentRunStatus` onto the notify-worthy state vocabulary
 * understood by `notificationsFromSessions`
 * (`failed` / `completed` / `needs_decision`).
 *
 * `waiting_for_input` is the run state where Autopilot is blocked on an operator
 * decision, so it maps to `needs_decision`.
 */
export const notificationStateForRunStatus = (
  status: AgentRunStatus,
): string | undefined => {
  switch (status) {
    case 'completed':
      return 'completed'
    case 'failed':
      return 'failed'
    case 'waiting_for_input':
      return 'needs_decision'
    default:
      return undefined
  }
}

const sessionFromChatRun = (
  chatRun: ChatRun,
): NotificationSession | undefined => {
  if (chatRun._tag !== 'Active') {
    return undefined
  }

  const state = notificationStateForRunStatus(chatRun.metadata.status)

  if (state === undefined) {
    return undefined
  }

  return {
    sessionRef: chatRun.metadata.runId,
    state,
  }
}

/**
 * Derive the current notify-worthy Pylon sessions from the model. The active
 * chat run is the session the web client polls; only notify-worthy states are
 * surfaced to the core.
 */
export const notificationSessionsForModel = (
  model: Model,
): ReadonlyArray<NotificationSession> => {
  const session = sessionFromChatRun(model.chatRun)

  return session === undefined ? [] : [session]
}

export const notificationCenterInputItems = (
  notifications: NotificationsModel,
): NotificationCenterInputItem[] =>
  notifications.items.map(item => ({
    sessionRef: item.sessionRef,
    title: item.title,
    body: item.body,
    priority: item.priority,
    at: item.at,
  }))
