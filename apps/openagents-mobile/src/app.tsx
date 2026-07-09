import { khalaTheme } from "@effect-native/tokens"
import { StatusBar } from "expo-status-bar"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { HomeScreen } from "./screens/home-screen"

/**
 * OpenAgents mobile (#8597) — greenfield app shell. The application/component/
 * intent model is Effect Native; this React tree is host machinery only: a
 * safe-area provider, a status bar, and the Home screen's Effect Native mount.
 *
 * Styling policy: typed style objects on the shared `@effect-native/tokens`
 * vocabulary (the Protoss-blue `khalaTheme`). No NativeWind, no Tailwind class
 * strings — see docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md.
 */
export const App = () => (
  <SafeAreaProvider style={{ backgroundColor: khalaTheme.color.background }}>
    <StatusBar style="light" />
    <HomeScreen />
  </SafeAreaProvider>
)
