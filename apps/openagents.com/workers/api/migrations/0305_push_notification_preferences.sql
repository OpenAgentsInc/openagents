-- MM-G2 (#8486): per-user push notification preference (global toggle).
--
-- Absent row == the default (enabled). This mirrors the "opt-out, not
-- opt-in" default other notification-adjacent surfaces in this repo use;
-- `readPushNotificationPreference` treats no row as `pushEnabled: true`.

CREATE TABLE IF NOT EXISTS push_notification_preferences (
  user_id TEXT PRIMARY KEY,
  push_enabled INTEGER NOT NULL DEFAULT 1 CHECK (push_enabled IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
