/** TEMP-DIAG-8467: report the mobile sync runtime + scope drive state to the
 * server debug sink so we can see, from the device, exactly where a
 * signed-in thread-list load gets stuck (the "Loading threads" loop). Fully
 * best-effort: never throws, never blocks a render. Remove with the sign-in
 * diagnostics once the empty-scope live-ready path is fixed. */
import { KHALA_OPENAGENTS_API_BASE_URL } from "../config/khala-sync-demo"

export const beaconSyncDebug = (snapshot: Record<string, unknown>): void => {
  try {
    void fetch(`${KHALA_OPENAGENTS_API_BASE_URL}/api/mobile/signin-debug`, {
      body: JSON.stringify({ kind: "sync", marker: "syncdbg1", ...snapshot }),
      headers: { "content-type": "application/json" },
      method: "POST",
    }).catch(() => undefined)
  } catch {
    // best-effort diagnostics only
  }
}
