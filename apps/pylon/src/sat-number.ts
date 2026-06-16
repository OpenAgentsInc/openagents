// Shared satoshi-amount coercion (#5166).
//
// The Breez Spark SDK's wasm-bindgen layer hands back u64 money amounts as a
// `bigint` (its whole money surface is bigint; only GetInfoResponse.balanceSats
// is optimistically typed `number`), and some daemons serialize large amounts as
// decimal strings. A strict `typeof === "number"` check then coerces a real
// amount to `null` — which made received funds invisible and kept send-readiness
// blocked. Normalize every money read through this instead.
//
// Lives in its own module (no imports) so both `wallet.ts` and
// `spark-backup-helper.ts` can use it without a circular import.

/**
 * Coerce a satoshi amount that may arrive as a JS number, a bigint, or a decimal
 * string into a plain non-negative number. Returns null when the value is absent
 * or not a non-negative integer amount.
 */
export function toSatNumber(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null
  if (typeof value === "bigint") {
    return value <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(value) : Number.MAX_SAFE_INTEGER
  }
  if (typeof value === "string") {
    const trimmed = value.trim()
    if (/^\d+$/.test(trimmed)) {
      const n = Number(trimmed)
      return Number.isFinite(n) ? n : null
    }
  }
  return null
}
