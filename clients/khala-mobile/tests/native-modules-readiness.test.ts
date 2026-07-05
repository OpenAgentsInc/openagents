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
 *
 * `../src/native/modules` (the wrapper under test) is imported below with a
 * cache-busting query suffix rather than the bare specifier. Bun's
 * `mock.module` registry is process-global and keyed on the exact specifier
 * string, and a mocked module's cached exports persist for the rest of the
 * `bun test` process once evaluated — so if any OTHER test file in this
 * suite (today or in the future — see `tests/chat-composer.test.tsx`, which
 * legitimately mocks this same wrapper wholesale for its own unrelated
 * purpose) mocks or triggers evaluation of the bare `"../src/native/modules"`
 * specifier before this file runs, `await import("../src/native/modules")`
 * here would silently return that OTHER file's (differently-shaped, or
 * incomplete) stand-in instead of the real production wrapper — this bit
 * a prior version of this test with a `readNativeReadiness is not a
 * function` crash. The `?fresh=` suffix makes this a distinct module
 * specifier that no `mock.module` call anywhere else in the suite targets,
 * forcing a genuinely fresh, real evaluation of `modules.ts` — using only
 * the two leaf-package mocks this file registers immediately above —
 * regardless of run order or what any other file has done to the bare
 * specifier (verified empirically: a cache-busted specifier bypasses an
 * active `mock.module` registration on the bare path entirely).
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

// TypeScript has no declaration for the cache-busted specifier (it isn't a
// real file on disk — see the comment above). Building the specifier from a
// non-literal string keeps `tsc` from attempting (and failing) static module
// resolution on it, and the type shape is pulled from the real module path
// via a type-only import, asserted onto the runtime dynamic-import result.
type NativeModulesModule = typeof import("../src/native/modules")
const freshNativeModulesSpecifier: string = "../src/native/modules?fresh=native-modules-readiness-test"
const { readNativeReadiness } = (await import(freshNativeModulesSpecifier)) as NativeModulesModule

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
