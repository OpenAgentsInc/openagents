import "../global.css"
import "./native/animated-view-css-interop"

import { StatusBar } from "expo-status-bar"
import { ActivityIndicator, LogBox, View } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { initialWindowMetrics, SafeAreaProvider } from "react-native-safe-area-context"

import { KhalaAuthProvider, useKhalaAuth } from "./auth/khala-auth-context"
import { BlurredPopupProvider } from "./components/blurred-popup"
import { KhalaErrorBoundary } from "./components/khala-error-boundary"
import { SignInScreen } from "./components/sign-in-screen"
import { AppNavigator } from "./navigators/AppNavigator"
import { KhalaThemeProvider } from "./theme/khala-theme-provider"

// React Native's own dev-only LogBox notification pill renders with broken
// (unreadable/invisible) text styling on this setup. It's dev chrome, not
// shipped app UI, and warnings still print to the Metro terminal.
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

  // "discovering" also renders SignInScreen — it owns the Tailnet
  // auto-discovery status UI so the phone never flashes a bare spinner while
  // quietly trying every candidate desktop host.
  if (status === "signed_out" || status === "discovering") return <SignInScreen />

  return (
    // Mounted once around the whole signed-in app so a long-press screenshot
    // (`BlurredPopupProvider`, issue #8395) captures the real rendered screen
    // behind it regardless of which route is active.
    <KhalaErrorBoundary>
      <BlurredPopupProvider>
        <AppNavigator />
      </BlurredPopupProvider>
    </KhalaErrorBoundary>
  )
}

export const App = () => (
  <GestureHandlerRootView style={{ flex: 1 }}>
    <SafeAreaProvider initialMetrics={initialWindowMetrics}>
      <KhalaThemeProvider>
        <StatusBar style="light" />
        <KhalaAuthProvider>
          <AuthGate />
        </KhalaAuthProvider>
      </KhalaThemeProvider>
    </SafeAreaProvider>
  </GestureHandlerRootView>
)
