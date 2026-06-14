import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  RaisedBrowserNotifications,
  ResolvedNotificationPermission,
} from '../message'

const notificationApiAvailable = (): boolean =>
  typeof window !== 'undefined' && 'Notification' in window

/**
 * Ask the browser for Notification permission.
 *
 * SSR-safe: when there is no `window`/`Notification`, resolves to a
 * non-granted, can't-ask-again state so `projectNotificationPermission`
 * settles on `denied` rather than prompting forever.
 */
export const RequestNotificationPermission = Command.define(
  'RequestNotificationPermission',
  ResolvedNotificationPermission,
)(
  Effect.gen(function* () {
    if (!notificationApiAvailable()) {
      return ResolvedNotificationPermission({
        granted: false,
        canAskAgain: false,
      })
    }

    const current = window.Notification.permission

    if (current === 'granted') {
      return ResolvedNotificationPermission({
        granted: true,
        canAskAgain: false,
      })
    }

    if (current === 'denied') {
      return ResolvedNotificationPermission({
        granted: false,
        canAskAgain: false,
      })
    }

    const result = yield* Effect.tryPromise(() =>
      window.Notification.requestPermission(),
    )

    return ResolvedNotificationPermission({
      granted: result === 'granted',
      canAskAgain: result === 'default',
    })
  }).pipe(
    Effect.catch(() =>
      Effect.succeed(
        ResolvedNotificationPermission({
          granted: false,
          canAskAgain: false,
        }),
      ),
    ),
  ),
)

const BrowserNotification = S.Struct({
  title: S.String,
  body: S.String,
})

/**
 * Raise one browser notification per derived notification via the Web
 * Notifications API. SSR-safe and a no-op when permission is not granted.
 */
export const RaiseBrowserNotifications = Command.define(
  'RaiseBrowserNotifications',
  { notifications: S.Array(BrowserNotification) },
  RaisedBrowserNotifications,
)(({ notifications }) =>
  Effect.sync(() => {
    if (
      notificationApiAvailable() &&
      window.Notification.permission === 'granted'
    ) {
      for (const notification of notifications) {
        try {
          // eslint-disable-next-line no-new -- side-effecting Web Notification.
          new window.Notification(notification.title, {
            body: notification.body,
          })
        } catch {
          // A failed individual notification must not break the batch.
        }
      }
    }

    return RaisedBrowserNotifications()
  }),
)
