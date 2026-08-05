/**
 * Local BOLT11 parse for destination entry (issue #9317 §5; MKT-SWP §7.2).
 *
 * This is the UX pre-check view of an invoice: network, amount, payment
 * hash, expiry, minimum final CLTV delta, route-hint disclosure, and the
 * description commitment are surfaced at entry time so the widget can
 * refuse early and render specifics. It is not the safety layer: the
 * MKT-SWP engine re-parses and verifies the complete invoice before any
 * fund action (§7.2), and signature recovery/verification is engine work.
 *
 * Amountless invoices are invalid in MKT-SWP v1 and are refused here with
 * the typed identifier `swp_invoice_invalid`, never prose.
 */
import { bech32 } from "@scure/base";

import type {
  BitcoinNetwork,
  Bolt11InvoiceDestination,
  DestinationParseFailure,
  InvoiceDescriptionCommitment,
} from "./model.js";

const HRP_NETWORKS: readonly (readonly [string, BitcoinNetwork])[] = [
  // Order matters: `lnbcrt` must match before `lnbc`, `lntbs` before `lntb`.
  ["lnbcrt", "regtest"],
  ["lntbs", "signet"],
  ["lntb", "testnet"],
  ["lnbc", "mainnet"],
];

const MSAT_PER_BTC = 100_000_000_000n;

const MULTIPLIER_DIVISORS: Readonly<Record<string, bigint>> = {
  m: 1_000n,
  u: 1_000_000n,
  n: 1_000_000_000n,
  p: 1_000_000_000_000n,
};

const TAG_PAYMENT_HASH = 1;
const TAG_ROUTE_HINT = 3;
const TAG_EXPIRY = 6;
const TAG_DESCRIPTION = 13;
const TAG_PAYMENT_SECRET = 16;
const TAG_DESCRIPTION_HASH = 23;
const TAG_MIN_FINAL_CLTV = 24;

const SIGNATURE_WORDS = 104;
const TIMESTAMP_WORDS = 7;
const HASH_WORDS = 52;
const ROUTE_HINT_ENTRY_BYTES = 51;

const DEFAULT_EXPIRY_SECONDS = 3600;
const DEFAULT_MIN_FINAL_CLTV = 18;

const failure = (f: DestinationParseFailure): ParseBolt11Result => ({
  ok: false,
  failure: f,
});

const malformed = (detail: string): ParseBolt11Result =>
  failure({ mode: "invoice_malformed", swpError: null, detail });

export type ParseBolt11Result =
  | { readonly ok: true; readonly invoice: Bolt11InvoiceDestination }
  | { readonly ok: false; readonly failure: DestinationParseFailure };

/** True when the (lowercased) text starts with a BOLT11 HRP. */
export const looksLikeBolt11 = (lower: string): boolean =>
  HRP_NETWORKS.some(([hrp]) => lower.startsWith(hrp)) && lower.includes("1");

const wordsToNumber = (words: readonly number[]): number => {
  let value = 0;
  for (const word of words) value = value * 32 + word;
  return value;
};

const wordsToBytes = (words: readonly number[]): Uint8Array | null => {
  // Tagged-field data pads the final partial group with zero bits.
  let acc = 0;
  let bits = 0;
  const bytes: number[] = [];
  for (const word of words) {
    acc = (acc << 5) | word;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((acc >> bits) & 0xff);
    }
  }
  if (bits > 0 && (acc & ((1 << bits) - 1)) !== 0) return null;
  return Uint8Array.from(bytes);
};

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

const parseAmountMsat = (
  amountText: string,
): { readonly ok: true; readonly msat: bigint } | { readonly ok: false } => {
  const match = /^([0-9]+)([munp]?)$/.exec(amountText);
  if (match === null) return { ok: false };
  const digits = match[1] ?? "";
  if (digits.length > 1 && digits.startsWith("0")) return { ok: false };
  const value = BigInt(digits);
  if (value === 0n) return { ok: false };
  const multiplier = match[2] ?? "";
  if (multiplier === "") return { ok: true, msat: value * MSAT_PER_BTC };
  const divisor = MULTIPLIER_DIVISORS[multiplier];
  if (divisor === undefined) return { ok: false };
  const scaled = value * MSAT_PER_BTC;
  if (scaled % divisor !== 0n) return { ok: false };
  return { ok: true, msat: scaled / divisor };
};

export interface ParseBolt11Context {
  readonly network: BitcoinNetwork;
  readonly nowSeconds: number;
}

/**
 * Parse a normalised (lowercase, trimmed) BOLT11 string. Callers reach this
 * only through the shared `parseDestination`, which owns normalisation and
 * scheme extraction.
 */
export const parseBolt11 = (
  lower: string,
  context: ParseBolt11Context,
): ParseBolt11Result => {
  let prefix: string;
  let words: number[];
  try {
    const decoded = bech32.decode(lower as `${string}1${string}`, false);
    prefix = decoded.prefix;
    words = [...decoded.words];
  } catch {
    return failure({ mode: "invoice_checksum_invalid", swpError: null });
  }

  const hrpEntry = HRP_NETWORKS.find(([hrp]) => prefix.startsWith(hrp));
  if (hrpEntry === undefined) return malformed("unknown invoice prefix");
  const [hrp, network] = hrpEntry;

  if (words.length < TIMESTAMP_WORDS + SIGNATURE_WORDS) {
    return malformed("invoice data too short");
  }

  const amountText = prefix.slice(hrp.length);
  if (amountText === "") {
    return failure({
      mode: "invoice_amountless",
      swpError: "swp_invoice_invalid",
    });
  }
  const amount = parseAmountMsat(amountText);
  if (!amount.ok) {
    return failure({ mode: "invoice_amount_invalid", swpError: null, amountText });
  }

  const timestampSeconds = wordsToNumber(words.slice(0, TIMESTAMP_WORDS));
  const tagged = words.slice(TIMESTAMP_WORDS, words.length - SIGNATURE_WORDS);

  let paymentHashHex: string | null = null;
  let description: InvoiceDescriptionCommitment | null = null;
  let expirySeconds = DEFAULT_EXPIRY_SECONDS;
  let minFinalCltvExpiryDelta = DEFAULT_MIN_FINAL_CLTV;
  let routeHintCount = 0;
  let hasPaymentSecret = false;

  let index = 0;
  while (index < tagged.length) {
    if (index + 3 > tagged.length) return malformed("truncated tagged field");
    const type = tagged[index] ?? 0;
    const dataLength = (tagged[index + 1] ?? 0) * 32 + (tagged[index + 2] ?? 0);
    const dataStart = index + 3;
    const dataEnd = dataStart + dataLength;
    if (dataEnd > tagged.length) return malformed("tagged field overruns data");
    const data = tagged.slice(dataStart, dataEnd);
    index = dataEnd;

    switch (type) {
      case TAG_PAYMENT_HASH: {
        // BOLT11: readers skip p fields with unexpected length.
        if (data.length !== HASH_WORDS) break;
        if (paymentHashHex !== null) return malformed("duplicate payment hash");
        const bytes = wordsToBytes(data);
        if (bytes === null || bytes.length !== 32) {
          return malformed("payment hash padding invalid");
        }
        paymentHashHex = bytesToHex(bytes);
        break;
      }
      case TAG_DESCRIPTION: {
        if (description !== null) {
          return malformed("conflicting description commitment");
        }
        const bytes = wordsToBytes(data);
        if (bytes === null) return malformed("description padding invalid");
        description = {
          kind: "description",
          text: new TextDecoder().decode(bytes),
        };
        break;
      }
      case TAG_DESCRIPTION_HASH: {
        if (data.length !== HASH_WORDS) break;
        if (description !== null) {
          return malformed("conflicting description commitment");
        }
        const bytes = wordsToBytes(data);
        if (bytes === null || bytes.length !== 32) {
          return malformed("description hash padding invalid");
        }
        description = { kind: "description_hash", hashHex: bytesToHex(bytes) };
        break;
      }
      case TAG_EXPIRY: {
        expirySeconds = wordsToNumber(data);
        break;
      }
      case TAG_MIN_FINAL_CLTV: {
        minFinalCltvExpiryDelta = wordsToNumber(data);
        break;
      }
      case TAG_ROUTE_HINT: {
        const bytes = wordsToBytes(data);
        if (bytes === null || bytes.length % ROUTE_HINT_ENTRY_BYTES !== 0) {
          return malformed("route hint field invalid");
        }
        routeHintCount += bytes.length / ROUTE_HINT_ENTRY_BYTES;
        break;
      }
      case TAG_PAYMENT_SECRET: {
        if (data.length === HASH_WORDS) hasPaymentSecret = true;
        break;
      }
      default:
        // Unknown tagged fields are skipped, per BOLT11.
        break;
    }
  }

  if (paymentHashHex === null) return malformed("missing payment hash");
  if (description === null) return malformed("missing description commitment");

  if (network !== context.network) {
    return failure({
      mode: "invoice_network_mismatch",
      swpError: null,
      expected: context.network,
      actual: network,
    });
  }

  const expiresAtSeconds = timestampSeconds + expirySeconds;
  if (expiresAtSeconds <= context.nowSeconds) {
    return failure({
      mode: "invoice_expired",
      swpError: null,
      expiresAtSeconds,
    });
  }

  return {
    ok: true,
    invoice: {
      kind: "bolt11_invoice",
      rail: "lightning",
      invoice: lower,
      network,
      amountMsat: amount.msat,
      paymentHashHex,
      timestampSeconds,
      expirySeconds,
      expiresAtSeconds,
      minFinalCltvExpiryDelta,
      description,
      routeHintCount,
      hasPaymentSecret,
    },
  };
};
