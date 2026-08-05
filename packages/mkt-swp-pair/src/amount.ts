/**
 * Exact integer-satoshi arithmetic for amount entry (openagents#9316 §3).
 *
 * Every amount is a `bigint` count of satoshis or a canonical decimal
 * string per MKT-SWP §3.2 (`^(0|[1-9][0-9]*)$`, u64 range, no JSON number,
 * no sign, no decimal point, no exponent, no whitespace, no unit suffix,
 * no leading zero). No floating-point value ever represents money in this
 * package — a source-scan test asserts no float operation exists in the
 * amount path, and boundary tests pin exact round-trips.
 *
 * The display denomination (BTC or sats) and the decimal separator are
 * persisted user preferences. Changing denomination converts the parsed
 * value exactly; it never reinterprets typed digits (the no-auto-unit-
 * switch law — Boltz's silent auto-denomination switching is the
 * anti-pattern, teardown §3.2).
 */

export const SATS_PER_BTC = 100_000_000n;

/** Largest v1 amount: 21,000,000 BTC in satoshis (MKT-SWP §3.2 bound). */
export const MAX_AMOUNT_SATS = 2_100_000_000_000_000n;

export type Denomination = "btc" | "sats";

export type DecimalSeparator = "." | ",";

const WIRE_AMOUNT_PATTERN = /^(0|[1-9][0-9]*)$/;
const U64_MAX = 18_446_744_073_709_551_615n;

/**
 * Parse a canonical MKT-SWP wire amount string. Returns `null` unless the
 * exact §3.2 grammar and the u64 bound hold.
 */
export const satsFromWire = (value: string): bigint | null => {
  if (!WIRE_AMOUNT_PATTERN.test(value)) return null;
  const sats = BigInt(value);
  return sats <= U64_MAX ? sats : null;
};

/** Canonical wire string for a satoshi count. */
export const satsToWire = (sats: bigint): string => {
  if (sats < 0n) throw new RangeError("negative satoshi amount");
  return sats.toString(10);
};

export const AMOUNT_PARSE_FAILURE_MODES = [
  "empty",
  "not_a_number",
  "multiple_separators",
  "sats_fractional",
  "too_many_decimal_places",
  "exceeds_supply",
] as const;

export type AmountParseFailureMode = (typeof AMOUNT_PARSE_FAILURE_MODES)[number];

export type AmountParseResult =
  | { readonly ok: true; readonly sats: bigint }
  | { readonly ok: false; readonly mode: AmountParseFailureMode };

const DIGITS_ONLY = /^[0-9]+$/;

/**
 * Parse user-typed amount text in the current denomination. The text is
 * read strictly in the denomination the user selected: digits plus at most
 * one configured decimal separator in BTC mode; integer digits only in
 * sats mode (a separator in sats mode is `sats_fractional`, never silently
 * reinterpreted as BTC). All arithmetic is bigint: BTC text becomes
 * `whole * SATS_PER_BTC + fraction` with the fraction zero-padded to 8
 * places.
 */
export const parseAmountText = (
  text: string,
  denomination: Denomination,
  separator: DecimalSeparator,
): AmountParseResult => {
  const trimmed = text.trim();
  if (trimmed === "") return { ok: false, mode: "empty" };

  if (denomination === "sats") {
    if (trimmed.includes(".") || trimmed.includes(",")) {
      return { ok: false, mode: "sats_fractional" };
    }
    if (!DIGITS_ONLY.test(trimmed)) return { ok: false, mode: "not_a_number" };
    const sats = BigInt(trimmed);
    if (sats > MAX_AMOUNT_SATS) return { ok: false, mode: "exceeds_supply" };
    return { ok: true, sats };
  }

  const parts = trimmed.split(separator);
  if (parts.length > 2) return { ok: false, mode: "multiple_separators" };
  const wholeText = parts[0] ?? "";
  const fractionText = parts[1] ?? "";
  if (wholeText === "" && fractionText === "") {
    return { ok: false, mode: "not_a_number" };
  }
  if (wholeText !== "" && !DIGITS_ONLY.test(wholeText)) {
    return { ok: false, mode: "not_a_number" };
  }
  if (fractionText !== "" && !DIGITS_ONLY.test(fractionText)) {
    return { ok: false, mode: "not_a_number" };
  }
  if (fractionText.length > 8) {
    return { ok: false, mode: "too_many_decimal_places" };
  }
  const whole = wholeText === "" ? 0n : BigInt(wholeText);
  const fraction =
    fractionText === "" ? 0n : BigInt(fractionText.padEnd(8, "0"));
  const sats = whole * SATS_PER_BTC + fraction;
  if (sats > MAX_AMOUNT_SATS) return { ok: false, mode: "exceeds_supply" };
  return { ok: true, sats };
};

/**
 * Format a satoshi count for display in the given denomination. BTC text
 * trims trailing fraction zeros but always keeps the whole part ("0.0015",
 * "1", "0.00000001"). Exact string arithmetic; round-trips through
 * `parseAmountText` are identity for every representable amount.
 */
export const formatAmountText = (
  sats: bigint,
  denomination: Denomination,
  separator: DecimalSeparator,
): string => {
  if (sats < 0n) throw new RangeError("negative satoshi amount");
  if (denomination === "sats") return sats.toString(10);
  const whole = sats / SATS_PER_BTC;
  const fraction = sats % SATS_PER_BTC;
  if (fraction === 0n) return whole.toString(10);
  const fractionText = fraction
    .toString(10)
    .padStart(8, "0")
    .replace(/0+$/, "");
  return `${whole.toString(10)}${separator}${fractionText}`;
};
