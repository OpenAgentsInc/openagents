import "../global.css"

import { Stack } from "expo-router"
import { StatusBar } from "expo-status-bar"

import { khalaMobileTheme } from "../src/theme/tokens"

export default function RootLayout() {
  return (
    <>
      <StatusBar style="light" />
      <Stack
        screenOptions={{
          contentStyle: { backgroundColor: khalaMobileTheme.background },
          headerStyle: { backgroundColor: khalaMobileTheme.background },
          headerTintColor: khalaMobileTheme.text,
          headerTitleStyle: { fontWeight: "600" }
        }}
      />
    </>
  )
}
