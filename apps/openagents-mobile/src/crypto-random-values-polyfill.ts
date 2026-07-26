/**
 * Wire the Web Crypto fallback to the platform CSPRNG, once, at startup.
 *
 * The rationale, the two device failures this closes, and the reason a
 * `Math.random` shim would be worse than the crash all live in
 * `./crypto-random-values.ts`. This file exists only so the native import stays
 * out of the testable half.
 *
 * Import it before anything that can sign.
 */
import { getRandomValues } from "expo-crypto";

import { installWebCryptoFallback, type GetRandomValues } from "./crypto-random-values.ts";

installWebCryptoFallback(
  globalThis as Parameters<typeof installWebCryptoFallback>[0],
  getRandomValues as GetRandomValues,
);
