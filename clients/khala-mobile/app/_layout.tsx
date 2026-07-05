import "../global.css"
import "../src/native/animated-view-css-interop"

import { Stack } from "expo-router"
import { StatusBar } from "expo-status-bar"
import { ActivityIndicator, LogBox, View } from "react-native"

import { KhalaAuthProvider, useKhalaAuth } from "../src/auth/khala-auth-context"
import { SignInScreen } from "../src/components/sign-in-screen"
import { khalaMobileTheme } from "../src/theme/tokens"

// React Native's own dev-only LogBox notification pill renders with broken
// (unreadable/invisible) text styling on this setup. It's dev chrome, not
// shipped app UI, and warnings still print to the Metro terminal — so
// disable the in-app overlay rather than ship an unreadable notification.
LogBox.ignoreAllLogs(true)

const AuthGate = () => {
  const { status } = useKhalaAuth()

  if (status === "loading") {
    return (
      <View className="flex-1 items-center justify-center bg-bg">
        <ActivityIndicator color="#4fd0ff" />
      </View>
    )
  }

  if (status === "signed_out") return <SignInScreen />

  return (
    <Stack
      screenOptions={{
        contentStyle: { backgroundColor: khalaMobileTheme.background },
        headerShown: false
      }}
    />
  )
}

export default function RootLayout() {
  return (
    <>
      <StatusBar style="light" />
      <KhalaAuthProvider>
        <AuthGate />
      </KhalaAuthProvider>
    </>
  )
}
