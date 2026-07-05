import { useEffect, useState } from "react"

import { buildOnDeviceReadinessRows, type OnDeviceReadinessRow } from "./on-device-readiness-core"
import { readNativeReadiness } from "./modules"

export type OnDeviceReadinessState = Readonly<{
  status: "loading" | "ready" | "error"
  rows: ReadonlyArray<OnDeviceReadinessRow>
}>

/**
 * Runs the one-shot speech + Apple Foundation Models availability probes
 * (`readNativeReadiness()`) on mount and formats them for display. Ported
 * from `../legacy-screens/settings.tsx`'s identical `useEffect` +
 * `readNativeReadiness()` call, which rendered the two raw statuses as plain
 * `StatLine` rows; this keeps the same probe but hands back the toned rows
 * `buildOnDeviceReadinessRows` produces for the routed settings screen's
 * Frame-card convention.
 */
export const useOnDeviceReadiness = (): OnDeviceReadinessState => {
  const [state, setState] = useState<OnDeviceReadinessState>({ rows: [], status: "loading" })

  useEffect(() => {
    let cancelled = false
    readNativeReadiness()
      .then(result => {
        if (cancelled) return
        setState({ rows: buildOnDeviceReadinessRows(result), status: "ready" })
      })
      .catch(() => {
        if (cancelled) return
        setState({ rows: [], status: "error" })
      })
    return () => {
      cancelled = true
    }
  }, [])

  return state
}
