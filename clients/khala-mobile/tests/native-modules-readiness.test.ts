import { describe, expect, mock, test } from "bun:test"

/**
 * `readNativeReadiness()` (`../src/native/modules`) runs two unrelated
 * native capability probes: `pushToTalkStt.getAvailabilityAsync()` and
 * `appleFoundationModels.getAvailabilityAsync()`. Per
 * docs/2026-07-05-promise-all-cron-landmine-audit.md, the native module
 * bodies never throw today, but the Expo JS/native bridge itself can reject
 * independently of the native implementation (a real, explicitly-supported
 * failure mode when a native binary is out of sync with an OTA-updated
 * bundle). A bare `Promise.all` over the two probes would let one bridge
 * rejection blank visibility into the OTHER, completely unrelated
 * capability's real readiness.
 *
 * The real `khala-push-to-talk-stt`/`khala-apple-foundation-models` default
 * exports call Expo's `requireNativeModule` at import time, which throws
 * immediately outside a native host (see `tests/chat-composer.test.tsx` for
 * the same constraint) — so both packages are mocked here at the module
 * level, matching that established pattern, with the push-to-talk probe
 * rejecting like a real bridge failure and the Apple FM probe resolving
 * normally.
 */
mock.module("khala-push-to-talk-stt", () => ({
  default: {
    getAvailabilityAsync: () => Promise.reject(new Error("bridge disconnected: native binary out of sync")),
    startRecognitionAsync: () => Promise.reject(new Error("not implemented in test")),
    stopRecognitionAsync: () => Promise.reject(new Error("not implemented in test"))
  }
}))

mock.module("khala-apple-foundation-models", () => ({
  default: {
    getAvailabilityAsync: () =>
      Promise.resolve({ blockerRefs: [], status: "available", summary: "Apple Foundation Models are ready." })
  }
}))

const { readNativeReadiness } = await import("../src/native/modules")

describe("readNativeReadiness", () => {
  test("isolates one probe's bridge rejection from the sibling probe's real readiness", async () => {
    const result = await readNativeReadiness()

    // The Apple FM probe succeeded and must NOT be blanked out by the
    // unrelated speech probe's bridge rejection.
    expect(result.appleFM.status).toBe("available")
    expect(result.appleFM.summary).toBe("Apple Foundation Models are ready.")

    // The speech probe's bridge rejection is represented as a typed
    // "unavailable" outcome (never an unhandled rejection that would have
    // taken down the whole `readNativeReadiness()` call).
    expect(result.speech.status).toBe("unavailable")
    expect(result.speech.reason).toContain("bridge disconnected")
  })
})
