import "../global.css"
import "../src/native/animated-view-css-interop"

import { Stack } from "expo-router"
import { StatusBar } from "expo-status-bar"
import { ActivityIndicator, LogBox, View } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"

import { KhalaAuthProvider, useKhalaAuth } from "../src/auth/khala-auth-context"
import { BlurredPopupProvider } from "../src/components/blurred-popup"
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
    // Mounted once around the whole signed-in app (Arcade's equivalent mount
    // point wraps its entire `<AppStack/>`), so a long-press screenshot
    // (`BlurredPopupProvider`, issue #8395) captures the real rendered
    // screen behind it regardless of which route is active.
    <BlurredPopupProvider>
      <Stack
        screenOptions={{
          contentStyle: { backgroundColor: khalaMobileTheme.background },
          headerShown: false
        }}
      />
    </BlurredPopupProvider>
  )
}

export default function RootLayout() {
  return (
    // Required by `react-native-gesture-handler`'s `GestureDetector`/Tap
    // gesture API (used by `TouchableFeedback` and the new `ArwesButton`,
    // issue #8392) — without this ancestor, gestures silently fail to
    // recognize at runtime ("GestureDetector must be used as a descendant of
    // GestureHandlerRootView"). Found missing while on-device verifying this
    // issue; fixed here since it's a whole-app prerequisite, not scoped to
    // one component.
    <GestureHandlerRootView style={{ flex: 1 }}>
      <StatusBar style="light" />
      <KhalaAuthProvider>
        <AuthGate />
      </KhalaAuthProvider>
    </GestureHandlerRootView>
  )
}
