import { useEffect } from "react"
import * as Notifications from "expo-notifications"
import { Linking } from "react-native"

import { parsePushNotificationDeepLink } from "./push-notify-deep-link-core"

/**
 * MM-H3 (#8489): wires a tapped push notification to the thread it's about
 * (server-emitted `data.deepLink`, `khala://thread/<threadId>`) via the
 * SAME `Linking`/`linking` config `AppNavigator.tsx` already uses for the
 * `ThreadMessages` route — so `Linking.openURL` here is exactly equivalent
 * to the user having tapped a `khala://thread/...` link anywhere else.
 * Covers both the cold-start case (app launched BY tapping a notification)
 * and the warm/background case (app already running).
 */
export const usePushNotificationDeepLink = (): void => {
  useEffect(() => {
    const openIfDeepLink = (data: unknown) => {
      const deepLink = parsePushNotificationDeepLink(data)
      if (deepLink !== null) void Linking.openURL(deepLink)
    }

    // Cold start: the app was launched by tapping a notification.
    void Notifications.getLastNotificationResponseAsync().then(response => {
      if (response !== null) openIfDeepLink(response.notification.request.content.data)
    })

    // Warm/background: the app was already running (or backgrounded) when
    // the user tapped the notification.
    const subscription = Notifications.addNotificationResponseReceivedListener(response => {
      openIfDeepLink(response.notification.request.content.data)
    })

    return () => subscription.remove()
  }, [])
}
