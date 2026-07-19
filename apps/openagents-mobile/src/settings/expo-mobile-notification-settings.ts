import {
  defaultMobileNotificationPreferences,
  mapNotificationPermission,
  type MobileNotificationPreferences,
  type MobileNotificationSettingsPort,
  type MobileNotificationSnapshot,
} from "./mobile-settings"
import { readPushDeviceRegistrationRecord } from "../push/expo-push-device-registration"

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
      const permission = mapNotificationPermission(permissions)
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
      // `registered` here means the server's Expo push relay actually holds
      // this installation's Expo push token
      // (`../push/expo-push-device-registration.ts` registers it on sign-in
      // and permission grant, SARAH-PUSH-1 #9062) — NOT merely that the OS
      // handed the app a native APNs/FCM token via
      // `getDevicePushTokenAsync()`, which the server's Expo relay cannot
      // send through. That native probe is now only a secondary, clearly
      // labeled signal for the "not yet server-registered" case below.
      const registered = await readPushDeviceRegistrationRecord(SecureStore)
      if (registered !== null) {
        return {
          permission,
          registration: "registered",
          preferences,
          detail: "This installation is registered for OpenAgents push notifications.",
        }
      }
      try {
        const token = await Notifications.getDevicePushTokenAsync()
        const nativeRegistered = typeof token.data === "string" ? token.data.length > 0 : token.data != null
        return {
          permission,
          registration: "unregistered",
          preferences,
          detail: nativeRegistered
            ? "Permission is granted and this installation has a native push channel, but it has not registered an Expo push token with OpenAgents yet. Sign in (or reopen the app) to finish registration."
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
