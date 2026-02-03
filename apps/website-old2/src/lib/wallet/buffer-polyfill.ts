/**
 * Buffer polyfill for browser. bip39 (and other deps) expect Node's Buffer.
 * Import this first in the wallet entry so globalThis.Buffer is set before bip39 loads.
 */
import { Buffer } from "buffer";

if (typeof globalThis !== "undefined") {
  (globalThis as unknown as { Buffer: typeof Buffer }).Buffer = Buffer;
}
