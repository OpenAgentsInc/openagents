// The Breez Spark SDK can surface money amounts as number, bigint, or decimal
// string depending on the wasm-bindgen path. Normalize before exposing public
// treasury rail totals.
export const toSatNumber = value => {
  if (typeof value === 'number') {
    return Number.isFinite(value) && value >= 0 ? value : null
  }
  if (typeof value === 'bigint') {
    return value <= BigInt(Number.MAX_SAFE_INTEGER)
      ? Number(value)
      : Number.MAX_SAFE_INTEGER
  }
  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (/^\d+$/.test(trimmed)) {
      const n = Number(trimmed)
      return Number.isFinite(n) ? n : null
    }
  }
  return null
}
