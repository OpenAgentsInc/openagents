import type { RaiseOsNotification } from "./notifier.js"

// CL-30 (desktop): raise a native macOS notification via `osascript`. This is
// the reliable Bun-side fallback for the Electrobun host process (the webview
// Notification API is not guaranteed to be granted/visible in the host shell).
// On non-macOS hosts this is a no-op rather than a hard failure, so polling and
// the in-app notification center keep working everywhere.

function escapeAppleScriptString(value: string): string {
  // AppleScript string literals are double-quoted; backslashes and quotes must
  // be escaped. Collapse newlines so the `display notification` stays one line.
  return value
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/[\r\n]+/g, " ")
}

export function raiseMacNotification(notification: {
  readonly title: string
  readonly body: string
  readonly priority: string
}): void {
  if (process.platform !== "darwin") return

  const title = escapeAppleScriptString(notification.title)
  const body = escapeAppleScriptString(notification.body)
  const script = `display notification "${body}" with title "${title}"`

  // Fire-and-forget: never block the poll loop on the notification subprocess.
  Bun.spawn(["osascript", "-e", script], {
    stdout: "ignore",
    stderr: "ignore",
  })
}

export const raiseOsNotification: RaiseOsNotification = raiseMacNotification
