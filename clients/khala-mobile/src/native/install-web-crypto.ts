import { getRandomValues, randomUUID } from "expo-crypto"

import { applyWebCryptoShim } from "./install-web-crypto-core"

/**
 * Install a Web Crypto shim (`globalThis.crypto.getRandomValues` +
 * `globalThis.crypto.randomUUID`) backed by `expo-crypto` before any app code
 * runs. Imported for its side effect at the very top of `index.tsx`.
 *
 * React Native's Hermes engine ships no `globalThis.crypto`, so any dependency
 * reaching for Web Crypto throws. Real production crash (2026-07-06, build 15):
 * starting a task from the onboarding flow hit `@tanstack/db`'s
 * `safeRandomUUID()` (optimistic transaction IDs) → "No secure random number
 * generator available: neither crypto.randomUUID nor crypto.getRandomValues is
 * defined in this environment."
 *
 * The canonical fix (`react-native-get-random-values`) is a NEW native module
 * that would require a fresh native build — it cannot ship over-the-air.
 * `expo-crypto` is already a native dependency of this build and exposes
 * synchronous, native-backed `getRandomValues` + `randomUUID`, so this pure-JS
 * shim closes the gap and ships via OTA. See `install-web-crypto-core.ts`.
 */
applyWebCryptoShim(globalThis as unknown as { crypto?: Record<string, unknown> }, {
  getRandomValues: getRandomValues as unknown as <T>(typedArray: T) => T,
  randomUUID,
})
