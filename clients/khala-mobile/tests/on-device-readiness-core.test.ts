import { describe, expect, test } from "bun:test"

import { buildOnDeviceReadinessRows } from "../src/native/on-device-readiness-core"

describe("buildOnDeviceReadinessRows", () => {
  test("tones a fully-available speech + Apple FM pair as success", () => {
    const rows = buildOnDeviceReadinessRows({
      appleFM: { blockerRefs: [], status: "available", summary: "ready" },
      speech: { status: "available" }
    })
    expect(rows).toHaveLength(2)
    expect(rows[0]).toMatchObject({ key: "speech", status: "available", tone: "success" })
    expect(rows[1]).toMatchObject({ key: "appleFM", status: "available", tone: "success" })
  })

  test("tones denied speech as danger and surfaces the reason as detail", () => {
    const rows = buildOnDeviceReadinessRows({
      appleFM: { blockerRefs: [], status: "unavailable", summary: "Apple Foundation Models are not available on Android." },
      speech: { reason: "speech_or_microphone_permission_denied", status: "denied" }
    })
    const speechRow = rows.find(row => row.key === "speech")
    expect(speechRow?.tone).toBe("danger")
    expect(speechRow?.detail).toBe("speech_or_microphone_permission_denied")
  })

  test("tones a blocked Apple FM bridge as warning with its summary as detail", () => {
    const rows = buildOnDeviceReadinessRows({
      appleFM: {
        blockerRefs: ["blocker.khala_mobile.apple_fm_bridge_health_unproven"],
        status: "blocked",
        summary: "Apple FM bridge requires local helper health proof before mobile inference."
      },
      speech: { status: "unavailable" }
    })
    const appleFMRow = rows.find(row => row.key === "appleFM")
    expect(appleFMRow?.tone).toBe("warning")
    expect(appleFMRow?.detail).toBe("Apple FM bridge requires local helper health proof before mobile inference.")
  })

  test("tones an unavailable (non-Apple, e.g. Android) Apple FM probe as faint, not danger", () => {
    const rows = buildOnDeviceReadinessRows({
      appleFM: {
        blockerRefs: ["blocker.khala_mobile.apple_fm_android_unavailable"],
        status: "unavailable",
        summary: "Apple Foundation Models are not available on Android."
      },
      speech: { status: "unavailable" }
    })
    const appleFMRow = rows.find(row => row.key === "appleFM")
    expect(appleFMRow?.tone).toBe("faint")
  })
})
