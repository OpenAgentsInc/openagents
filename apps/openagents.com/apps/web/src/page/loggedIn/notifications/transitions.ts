import {
  type NotificationPriority,
  type SessionNotification,
  buildNotificationCenter,
  notificationsFromSessions,
} from '@openagentsinc/autopilot-control-protocol'
import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { Message } from '../message'
import { Model } from '../model'
import { type UpdateReturn, noUpdate } from '../transition'
import { RaiseBrowserNotifications, RequestNotificationPermission } from './commands'
import {
  type NotificationCenterItem,
  type NotificationsModel,
  notificationCenterInputItems,
  notificationPermissionProjection,
  notificationSessionsForModel,
} from './model'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const toNotificationPriority = (priority: string): NotificationPriority =>
  priority === 'high' ? 'high' : priority === 'low' ? 'low' : 'normal'

/** Timestamp to stamp on a derived feed item: the session's last update. */
const atForSessionRef = (model: Model, sessionRef: string): string =>
  model.chatRun._tag === 'Active' && model.chatRun.metadata.runId === sessionRef
    ? model.chatRun.metadata.updatedAt
    : ''

const centerItemFromNotification = (
  model: Model,
  notification: SessionNotification,
): NotificationCenterItem => ({
  sessionRef: notification.sessionRef,
  title: notification.title,
  body: notification.body,
  priority: toNotificationPriority(notification.priority),
  at: atForSessionRef(model, notification.sessionRef),
})

/**
 * Derive new notifications from the current sessions + seen-set using the shared
 * `notificationsFromSessions` core, append them to the in-app feed, advance the
 * seen-set, and — when permission is granted — emit a command that raises a
 * browser notification per new item.
 *
 * Pure: the only timestamp used comes from existing model data
 * (`chatRun.metadata.updatedAt`), so `update` stays side-effect free.
 */
export const ingestSessionNotifications = (model: Model): UpdateReturn => {
  const sessions = notificationSessionsForModel(model)
  const result = notificationsFromSessions(
    [...sessions],
    [...model.notifications.seenRefs],
  )

  if (
    result.new.length === 0 &&
    result.seenRefs.length === model.notifications.seenRefs.length
  ) {
    return noUpdate(model)
  }

  const newItems = result.new.map(notification =>
    centerItemFromNotification(model, notification),
  )

  const nextModel = evo(model, {
    notifications: notifications => ({
      ...notifications,
      seenRefs: result.seenRefs,
      items: [...notifications.items, ...newItems],
    }),
  })

  const permission = notificationPermissionProjection(nextModel.notifications)
  const shouldRaise = permission.state === 'enabled' && result.new.length > 0
  const browserNotifications = result.new.map(notification => ({
    title: notification.title,
    body: notification.body,
  }))

  return [
    nextModel,
    shouldRaise
      ? [RaiseBrowserNotifications({ notifications: browserNotifications })]
      : [],
    Option.none(),
  ]
}

/**
 * Initial-command builder: request browser permission when the projected
 * permission state still allows prompting.
 */
export const notificationInitialCommands = (model: Model) => {
  const permission = notificationPermissionProjection(model.notifications)

  return permission.canPrompt ? [RequestNotificationPermission()] : []
}

const setPermission = (
  notifications: NotificationsModel,
  granted: boolean,
  canAskAgain: boolean,
): NotificationsModel => ({
  ...notifications,
  permissionGranted: granted,
  permissionCanAskAgain: canAskAgain,
})

export const updateNotifications = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedNotificationPermission: () => {
        const permission = notificationPermissionProjection(model.notifications)

        return permission.canPrompt
          ? [model, [RequestNotificationPermission()], Option.none()]
          : noUpdate(model)
      },
      ResolvedNotificationPermission: ({ granted, canAskAgain }) => [
        evo(model, {
          notifications: notifications =>
            setPermission(notifications, granted, canAskAgain),
        }),
        [],
        Option.none(),
      ],
      RaisedBrowserNotifications: () => noUpdate(model),
      DismissedNotifications: () => [
        evo(model, {
          notifications: notifications => ({ ...notifications, items: [] }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )

export const notificationCenterView = (model: Model) =>
  buildNotificationCenter(notificationCenterInputItems(model.notifications))
