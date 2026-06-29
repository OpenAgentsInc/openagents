// CL-30 / CL-53: pure string rendering of the in-app notification center.
//
// This is the read-only projection of the NotificationCenterView built by the
// shared `buildNotificationCenter` core on the Bun side. It is kept as a pure,
// DOM-free string helper (no electrobun, no foldkit) so the Bun-side notifier
// test can assert the unread count, the high-priority class, and HTML escaping
// without a runtime. The Foldkit notifications view (src/ui/view.ts) renders the
// same center view as real VNodes; this helper stays the canonical place for the
// escaping/labeling contract that notifier.test.ts pins.

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"

export function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")
}

export function notificationsHtml(view: NotificationCenterView): string {
  const heading = view.unread > 0 ? `Notifications · ${view.unread}` : "Notifications"
  const headerClass = view.hasHigh ? "notif-header notif-has-high" : "notif-header"

  const body =
    view.items.length === 0
      ? '<p class="empty-state">No notifications yet.</p>'
      : `<ul class="notif-list">${view.items
          .map((item) =>
            [
              `<li class="notif-row notif-${escapeHtml(item.priority)}">`,
              `<span class="notif-title">${escapeHtml(item.title)}</span>`,
              `<span class="notif-body">${escapeHtml(item.body)}</span>`,
              "</li>",
            ].join(""),
          )
          .join("")}</ul>`

  return [
    `<header class="${headerClass}">`,
    `<h2>${escapeHtml(heading)}</h2>`,
    "</header>",
    body,
  ].join("")
}
