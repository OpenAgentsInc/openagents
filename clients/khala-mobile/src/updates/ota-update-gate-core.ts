/** Pure decision logic for the OTA update gate (ported/adapted from the
 * retired `clients/mobile/AutopilotRemoteControl` app's `update-policy.ts` —
 * same "auto-download, auto-reload, no user prompt" behavior, adapted to the
 * modern `expo-updates` `useUpdates()` hook's richer reactive state instead
 * of manual polling). See docs/khala-code/2026-07-06-mobile-ota-updates-runbook.md.
 *
 * The whole point: never leave the user on a stale bundle wondering "is this
 * thing even checking for updates" — check aggressively, fetch immediately
 * once found, reload immediately once fetched. No button, no prompt. */

export type OtaGateSnapshot = Readonly<{
  isChecking: boolean
  isDownloading: boolean
  isUpdateAvailable: boolean
  isUpdatePending: boolean
  isRestarting: boolean
}>

export type OtaGateAction = "fetch" | "reload" | "none"

/** Given the current `useUpdates()` snapshot, what should the gate do next?
 * - An update has been fetched and is ready (`isUpdatePending`) -> reload now.
 * - An update was found but isn't downloading/downloaded yet -> fetch it.
 * - Otherwise -> nothing to do (already in progress, or nothing available). */
export const decideOtaGateAction = (snapshot: OtaGateSnapshot): OtaGateAction => {
  if (snapshot.isRestarting) return "none"
  if (snapshot.isUpdatePending) return "reload"
  if (snapshot.isUpdateAvailable && !snapshot.isDownloading) return "fetch"
  return "none"
}

export type OtaGateVisibleState = "hidden" | "checking" | "downloading" | "reloading"

/** What the small status indicator should show. Hidden when idle/up-to-date
 * so it's never permanent chrome — only appears while something is actually
 * happening, which is itself the answer to "is this checking for updates". */
export const otaGateVisibleState = (snapshot: OtaGateSnapshot): OtaGateVisibleState => {
  if (snapshot.isRestarting) return "reloading"
  if (snapshot.isDownloading) return "downloading"
  if (snapshot.isChecking) return "checking"
  return "hidden"
}
