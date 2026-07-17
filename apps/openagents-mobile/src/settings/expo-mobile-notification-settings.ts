import {
  defaultMobileNotificationPreferences,
  type MobileNotificationPreferences,
  type MobileNotificationSettingsPort,
  type MobileNotificationSnapshot,
} from "./mobile-settings"

const PREFERENCES_KEY = "openagents.mobile.notification.preferences.v1"

type SecureStoreLike = Readonly<{
  getItemAsync: (key: string) => Promise<string | null>
  setItemAsync: (key: string, value: string) => Promise<void>
}>

const loadPreferences = async (store: SecureStoreLike): Promise<MobileNotificationPreferences> => {
  try {
    const raw = await store.getItemAsync(PREFERENCES_KEY)
    if (raw === null) return defaultMobileNotificationPreferences
    const value = JSON.parse(raw) as Record<string, unknown>
    return typeof value.attention === "boolean" && typeof value.completion === "boolean" &&
      typeof value.approvals === "boolean"
      ? { attention: value.attention, completion: value.completion, approvals: value.approvals }
      : defaultMobileNotificationPreferences
  } catch {
    return defaultMobileNotificationPreferences
  }
}

export const openExpoMobileNotificationSettings = (): MobileNotificationSettingsPort => {
  const Notifications = require("expo-notifications") as typeof import("expo-notifications")
  const SecureStore = require("expo-secure-store") as SecureStoreLike

  const snapshot = async (): Promise<MobileNotificationSnapshot> => {
    const preferences = await loadPreferences(SecureStore)
    try {
      const permissions = await Notifications.getPermissionsAsync()
      const permission = permissions.granted
        ? "granted" as const
        : permissions.canAskAgain ? "undetermined" as const : "denied" as const
      if (permission !== "granted") {
        return {
          permission,
          registration: "unregistered",
          preferences,
          detail: permission === "denied"
            ? "Notifications are off in system settings. OpenAgents will not prompt again automatically."
            : "Notifications stay off until you explicitly enable them.",
        }
      }
      try {
        const token = await Notifications.getDevicePushTokenAsync()
        const registered = typeof token.data === "string" ? token.data.length > 0 : token.data != null
        return {
          permission,
          registration: registered ? "registered" : "unregistered",
          preferences,
          detail: registered
            ? "This installation has a native push registration. Token material remains in the native host."
            : "Permission is granted, but this installation has no native push registration yet.",
        }
      } catch {
        return {
          permission,
          registration: "unavailable",
          preferences,
          detail: "Permission is granted, but native push registration is unavailable on this build or device.",
        }
      }
    } catch {
      return {
        permission: "undetermined",
        registration: "unavailable",
        preferences,
        detail: "Notification health could not be read on this installation.",
      }
    }
  }

  return {
    snapshot,
    requestPermission: async () => {
      await Notifications.requestPermissionsAsync()
      return snapshot()
    },
    setPreferences: async preferences => {
      await SecureStore.setItemAsync(PREFERENCES_KEY, JSON.stringify(preferences))
      return snapshot()
    },
  }
}
