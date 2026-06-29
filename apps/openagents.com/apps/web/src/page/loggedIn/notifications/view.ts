import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { FeedItem, Tone } from '../../../ui'
import { DismissedNotifications, type Message } from '../message'
import type { Model } from '../model'
import {
  type NotificationPriority,
  notificationPermissionProjection,
} from './model'
import { notificationCenterView } from './transitions'

const toneForPriority = (priority: NotificationPriority): Tone =>
  priority === 'high' ? 'warning' : priority === 'low' ? 'neutral' : 'info'

const permissionMeta = (state: string): string =>
  state === 'enabled'
    ? 'Browser notifications enabled'
    : state === 'denied'
      ? 'Browser notifications blocked'
      : 'Browser notifications not enabled'

/**
 * In-app notifications panel built from the shared `buildNotificationCenter`
 * core. Surfaces the derived Pylon-session feed (needs_decision / failed /
 * completed) and reflects the projected browser-permission state. Composed
 * entirely through the Foldkit UI system.
 */
export const notificationsPanel = (model: Model): Html => {
  const h = html<Message>()
  const center = notificationCenterView(model)
  const permission = notificationPermissionProjection(model.notifications)

  const feedItems: ReadonlyArray<FeedItem> = center.items.map(item => ({
    title: item.title,
    body: item.body,
    tone: toneForPriority(item.priority),
  }))

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: `Notifications${
          center.unread > 0 ? ` (${center.unread})` : ''
        }`,
        title: 'Session activity',
        body: permissionMeta(permission.state),
        level: 3,
      }),
      center.items.length === 0
        ? Ui.emptyState<Message>({
            title: 'No notifications yet',
            body: 'Sessions that need a decision, fail, or complete appear here.',
          })
        : Ui.notificationStack<Message>(feedItems),
      center.items.length === 0
        ? null
        : Ui.button<Message>({
            label: 'Clear',
            variant: 'ghost',
            size: 'sm',
            attrs: [h.OnClick(DismissedNotifications())],
          }),
    ],
    [h.DataAttribute('component', 'logged-in-notifications-panel')],
  )
}
