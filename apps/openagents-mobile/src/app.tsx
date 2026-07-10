import { khalaTheme } from "@effect-native/tokens"
import { StatusBar } from "expo-status-bar"
import * as Updates from "expo-updates"
import { useEffect, useState } from "react"
import { SafeAreaProvider } from "react-native-safe-area-context"

import { HomeScreen } from "./screens/home-screen"
import type { MobileSyncPhase } from "./screens/home-core"
import { openMobileSyncHost, type MobileSyncHost } from "./sync/mobile-sync-host"
import { startOtaPolling } from "./updates/ota-polling"

/**
 * OpenAgents mobile (#8597) — greenfield app shell. The application/component/
 * intent model is Effect Native; this React tree is host machinery only: a
 * safe-area provider, a status bar, and the Home screen's Effect Native mount.
 *
 * Styling policy: typed style objects on the shared `@effect-native/tokens`
 * vocabulary (the Protoss-blue `khalaTheme`). No NativeWind, no Tailwind class
 * strings — see docs/effect-native/2026-07-08-styling-tailwind-stylex-effect-native.md.
 */
export const App = () => {
  const [syncPhase, setSyncPhase] = useState<MobileSyncPhase>("unconfigured")

  // OTA: poll the owned OpenAgents Updates server (updates.openagents.com,
  // channel openagents-production) on the TEMPORARY aggressive 3s cadence —
  // see src/updates/ota-polling.ts. `Updates.isEnabled` is false in Expo Go /
  // dev, so the loop is a no-op there.
  useEffect(() => {
    let syncHost: MobileSyncHost | undefined
    try {
      syncHost = openMobileSyncHost()
      setSyncPhase("local_ready")
    } catch {
      setSyncPhase("unavailable")
    }
    const handle = startOtaPolling({
      isEnabled: Updates.isEnabled,
      checkForUpdateAsync: () => Updates.checkForUpdateAsync(),
      fetchUpdateAsync: () => Updates.fetchUpdateAsync(),
      reloadAsync: () => Updates.reloadAsync(),
    }, {
      beforeReload: () => syncHost?.close(),
    })
    return () => {
      handle.stop()
      syncHost?.close()
    }
  }, [])

  return (
    <SafeAreaProvider style={{ backgroundColor: khalaTheme.color.background }}>
      <StatusBar style="light" />
      <HomeScreen syncPhase={syncPhase} />
    </SafeAreaProvider>
  )
}
