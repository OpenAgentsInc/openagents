import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"

// CL-30 (desktop): render the in-app notification center built by the shared
// `buildNotificationCenter` core. Read-only: it shows what already happened.

function escapeHtml(value: string): string {
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

export function renderNotifications(container: HTMLElement, view: NotificationCenterView): void {
  container.innerHTML = notificationsHtml(view)
}
