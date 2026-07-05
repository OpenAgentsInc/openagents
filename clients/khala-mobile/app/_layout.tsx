import "../global.css"

import { Stack } from "expo-router"
import { StatusBar } from "expo-status-bar"
import { LogBox } from "react-native"

import { khalaMobileTheme } from "../src/theme/tokens"

// React Native's own dev-only LogBox notification pill renders with broken
// (unreadable/invisible) text styling on this setup. It's dev chrome, not
// shipped app UI, and warnings still print to the Metro terminal — so
// disable the in-app overlay rather than ship an unreadable notification.
LogBox.ignoreAllLogs(true)

export default function RootLayout() {
  return (
    <>
      <StatusBar style="light" />
      <Stack
        screenOptions={{
          contentStyle: { backgroundColor: khalaMobileTheme.background },
          headerShown: false
        }}
      />
    </>
  )
}
