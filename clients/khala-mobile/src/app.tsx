import "../global.css"
import "./native/animated-view-css-interop"

import { useFonts } from "expo-font"
import { StatusBar } from "expo-status-bar"
import { LogBox, View } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { initialWindowMetrics, SafeAreaProvider } from "react-native-safe-area-context"

import { KhalaAuthProvider, useKhalaAuth } from "./auth/khala-auth-context"
import { ActivityIndicator } from "./components/activity-indicator"
import { BlurredPopupProvider } from "./components/blurred-popup"
import { KhalaErrorBoundary } from "./components/khala-error-boundary"
import { SignInScreen } from "./components/sign-in-screen"
import { AppNavigator } from "./navigators/AppNavigator"
import { KhalaMobileSyncRuntimeProvider } from "./sync/khala-mobile-sync-runtime-context"
import { khalaMobileTheme } from "./theme/tokens"
import { KhalaThemeProvider } from "./theme/khala-theme-provider"
import { khalaMobileFontsToLoad } from "./theme/typography"
import { OtaUpdateGate } from "./updates/ota-update-gate"

// React Native's own dev-only LogBox notification pill renders with broken
// (unreadable/invisible) text styling on this setup. It's dev chrome, not
// shipped app UI, and warnings still print to the Metro terminal.
LogBox.ignoreAllLogs(true)

const AuthGate = () => {
  const { baseUrl, ownerUserId, status, token } = useKhalaAuth()

  if (status === "loading") {
    return (
      <View className="flex-1 items-center justify-center bg-bg">
        <ActivityIndicator color={khalaMobileTheme.accent} />
      </View>
    )
  }

  if (status === "signed_out" || status === "signing_in") return <SignInScreen />

  return (
    // Mounted once around the whole signed-in app so a long-press screenshot
    // (`BlurredPopupProvider`, issue #8395) captures the real rendered screen
    // behind it regardless of which route is active. The Khala Sync runtime
    // (Expo SQLite store + durable-cursor session, MC-8/#8433) is opened here
    // too — once per signed-in session — so every thread screen shares the
    // SAME durable local cache instead of each mount bootstrapping its own
    // from-scratch fetch.
    <KhalaErrorBoundary>
      <BlurredPopupProvider>
        <KhalaMobileSyncRuntimeProvider ownerUserId={ownerUserId} syncBaseUrl={baseUrl} token={token}>
          <AppNavigator />
        </KhalaMobileSyncRuntimeProvider>
      </BlurredPopupProvider>
    </KhalaErrorBoundary>
  )
}

export const App = () => {
  // Space Grotesk (arcade's primary font), Protomolecule (arcade's display
  // font, `heading` variant only), and JetBrains Mono (code/mono content)
  // must be loaded before ANYTHING renders — every `KhalaText` variant
  // references one of these family names directly, and an unloaded native
  // font name silently falls back to the OS default with no error, which
  // is exactly how this app's typography drifted from arcade's in the
  // first place. Blocks on a plain themed spinner, matching Ignite's own
  // font-gate pattern.
  const [fontsLoaded] = useFonts(khalaMobileFontsToLoad)

  if (!fontsLoaded) {
    return (
      <View className="flex-1 items-center justify-center bg-bg">
        <ActivityIndicator color={khalaMobileTheme.accent} />
      </View>
    )
  }

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <SafeAreaProvider initialMetrics={initialWindowMetrics}>
        <KhalaThemeProvider>
          <StatusBar style="light" />
          {/* Mounted above auth so OTA checking works even on the sign-in
           * screen — the exact screen a stale/stuck build gets caught on. */}
          <OtaUpdateGate />
          <KhalaAuthProvider>
            <AuthGate />
          </KhalaAuthProvider>
        </KhalaThemeProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  )
}
