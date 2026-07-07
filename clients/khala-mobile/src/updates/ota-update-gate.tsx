import { useEffect, useRef } from "react"
import { AppState, StyleSheet, View } from "react-native"
import * as Updates from "expo-updates"

import { Text } from "../ignite"
import { khalaMobileTheme } from "../theme/tokens"
import { ActivityIndicator } from "../components/activity-indicator"
import { decideOtaGateAction, otaGateVisibleState } from "./ota-update-gate-core"
import { closeActiveSyncRuntimeBeforeReload } from "../sync/khala-mobile-sync-runtime-registry"

/** Mounted once at the app root (before auth, so it works even on the
 * sign-in screen — the exact screen a stale/stuck build gets caught on).
 * Auto-checks for an OTA update on mount and every time the app returns to
 * the foreground, auto-downloads the moment one is found, and auto-reloads
 * the moment it's downloaded — no button, no prompt, matching the retired
 * AutopilotRemoteControl app's proven "download_and_reload" policy. Renders
 * nothing when idle or merely checking (routine checks are silent by
 * design — permanent "checking for updates" chrome is not something a
 * shipped app should show); a small themed pill appears only once an
 * update is actually downloading or being applied. A no-op in dev/Expo-Go
 * where `expo-updates` is disabled. */
export const OtaUpdateGate = () => {
  const { isChecking, isDownloading, isUpdateAvailable, isUpdatePending, isRestarting } =
    Updates.useUpdates()
  const fetchInFlight = useRef(false)

  const checkNow = () => {
    if (!Updates.isEnabled) return
    void Updates.checkForUpdateAsync().catch(() => {
      // Network hiccups are expected (offline, server briefly down); the
      // next foreground/interval check will retry. Never crash the app.
    })
  }

  // Check once on mount (in addition to the native ON_LOAD check, which ran
  // before any JS rendered) and every time the app comes back to the
  // foreground — a cold-launch-only check misses "I've had this open in the
  // background for a day" and "I just background/foregrounded to retry".
  useEffect(() => {
    if (!Updates.isEnabled) return
    checkNow()
    const subscription = AppState.addEventListener("change", nextState => {
      if (nextState === "active") checkNow()
    })
    // TEMP (owner dev convenience, 2026-07-07): poll for OTAs every 3s so a
    // freshly-published bundle lands near-instantly while actively testing.
    // DISABLE BEFORE PROD — a 3s update poll is far too aggressive for a
    // shipped app (battery/network); the on-mount + on-foreground checks above
    // are the real production cadence. Remove this interval (or gate it behind
    // a dev flag) when cutting over to a public production build.
    const devPoll = setInterval(checkNow, 3000)
    return () => {
      subscription.remove()
      clearInterval(devPoll)
    }
  }, [])

  useEffect(() => {
    if (!Updates.isEnabled) return
    const action = decideOtaGateAction({
      isChecking,
      isDownloading,
      isRestarting,
      isUpdateAvailable,
      isUpdatePending,
    })

    if (action === "fetch" && !fetchInFlight.current) {
      fetchInFlight.current = true
      void Updates.fetchUpdateAsync()
        .catch(() => {
          // Downloading failed (offline mid-fetch, etc.) — leave
          // isUpdateAvailable true so the next check/foreground retries.
        })
        .finally(() => {
          fetchInFlight.current = false
        })
    }

    if (action === "reload") {
      // A confirmed, reproducible production crash (build 11, 2026-07-06):
      // reloadAsync() tears down the JS context immediately, with no
      // guarantee any in-flight expo-sqlite work on the sync runtime has
      // drained first — a known expo-sqlite native race (a fresh async DB
      // request landing before a prior one resolves). Draining the sync
      // runtime closed first (bounded; a no-op when signed out) gives the
      // native queue a chance to finish before the JS context goes away.
      // See khala-mobile-sync-runtime-registry.ts and
      // docs/khala-mobile/2026-07-06-crash-triage-runbook.md.
      void closeActiveSyncRuntimeBeforeReload().finally(() => {
        void Updates.reloadAsync()
      })
    }
  }, [isChecking, isDownloading, isRestarting, isUpdateAvailable, isUpdatePending])

  const visible = Updates.isEnabled
    ? otaGateVisibleState({ isChecking, isDownloading, isRestarting, isUpdateAvailable, isUpdatePending })
    : "hidden"

  if (visible === "hidden") return null

  const label = visible === "downloading" ? "Downloading update…" : "Reloading…"

  return (
    <View pointerEvents="none" style={styles.container}>
      <View style={styles.pill}>
        <ActivityIndicator color={khalaMobileTheme.accent} size={14} />
        <Text size="xs" style={{ color: khalaMobileTheme.accentText }}>
          {label}
        </Text>
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    alignItems: "center",
    bottom: 12,
    left: 0,
    position: "absolute",
    right: 0,
    zIndex: 1000,
  },
  pill: {
    alignItems: "center",
    backgroundColor: "rgba(10, 17, 29, 0.92)",
    borderColor: khalaMobileTheme.accent,
    borderRadius: 999,
    borderWidth: 1,
    flexDirection: "row",
    gap: 8,
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
})
