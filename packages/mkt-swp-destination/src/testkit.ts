/**
 * Deterministic fixtures for destination-entry tests. Builds syntactically
 * valid BOLT11 invoices (correct bech32 checksum, dummy signature — the
 * parser does not verify signatures; the engine does), bech32 addresses
 * for any network, and LNURL strings, so tests never depend on copied
 * invoice strings whose checksums cannot be regenerated.
 */
import { sha256 } from "@noble/hashes/sha256";
import { base58, bech32, bech32m } from "@scure/base";

import type { BitcoinNetwork } from "./model.js";

const NETWORK_HRP: Readonly<Record<BitcoinNetwork, string>> = {
  mainnet: "lnbc",
  testnet: "lntb",
  signet: "lntbs",
  regtest: "lnbcrt",
};

const ADDRESS_HRP: Readonly<Record<BitcoinNetwork, string>> = {
  mainnet: "bc",
  testnet: "tb",
  signet: "tb",
  regtest: "bcrt",
};

const bytesToWords = (bytes: Uint8Array): number[] => bech32.toWords(bytes);

/** Pack bytes into 5-bit words, zero-padding the final partial group. */
const numberToWords = (value: number, minWords: number): number[] => {
  const words: number[] = [];
  let rest = value;
  while (rest > 0) {
    words.unshift(rest % 32);
    rest = Math.floor(rest / 32);
  }
  while (words.length < minWords) words.unshift(0);
  return words;
};

const taggedField = (type: number, data: readonly number[]): number[] => [
  type,
  Math.floor(data.length / 32),
  data.length % 32,
  ...data,
];

export interface TestInvoiceOptions {
  readonly network?: BitcoinNetwork;
  /** e.g. "2500u", "1", "25m", "10n"; `null` builds an amountless invoice. */
  readonly amount?: string | null;
  readonly timestampSeconds?: number;
  readonly expirySeconds?: number | null;
  readonly minFinalCltv?: number | null;
  readonly description?: string;
  readonly descriptionHashByte?: number | null;
  readonly paymentHashByte?: number;
  readonly routeHints?: number;
  readonly includePaymentSecret?: boolean;
  /** Drop the payment hash entirely (malformed). */
  readonly omitPaymentHash?: boolean;
  /** Drop both description commitments (malformed). */
  readonly omitDescription?: boolean;
  /**
   * Raw tagged fields appended after the standard ones, for crafted
   * invoices (duplicate `x`/`c` fields, out-of-range values).
   */
  readonly extraTaggedFields?: readonly {
    readonly type: number;
    readonly data: readonly number[];
  }[];
}

export const DEFAULT_TEST_TIMESTAMP = 1_754_265_600; // 2026-08-04T00:00:00Z

/** Build a checksum-valid BOLT11 invoice with a dummy signature. */
export const encodeTestInvoice = (options: TestInvoiceOptions = {}): string => {
  const network = options.network ?? "regtest";
  const amount = options.amount === undefined ? "2500u" : options.amount;
  const prefix = `${NETWORK_HRP[network]}${amount ?? ""}`;
  const timestamp = options.timestampSeconds ?? DEFAULT_TEST_TIMESTAMP;

  const words: number[] = [...numberToWords(timestamp, 7)];

  if (options.omitPaymentHash !== true) {
    const hash = new Uint8Array(32).fill(options.paymentHashByte ?? 0x01);
    words.push(...taggedField(1, bytesToWords(hash)));
  }
  if (options.omitDescription !== true) {
    if (options.descriptionHashByte != null) {
      const hash = new Uint8Array(32).fill(options.descriptionHashByte);
      words.push(...taggedField(23, bytesToWords(hash)));
    } else {
      const text = new TextEncoder().encode(options.description ?? "1 cup coffee");
      words.push(...taggedField(13, bytesToWords(text)));
    }
  }
  if (options.expirySeconds != null) {
    words.push(...taggedField(6, numberToWords(options.expirySeconds, 1)));
  }
  if (options.minFinalCltv != null) {
    words.push(...taggedField(24, numberToWords(options.minFinalCltv, 1)));
  }
  for (let i = 0; i < (options.routeHints ?? 0); i += 1) {
    const hint = new Uint8Array(51).fill(0x02);
    words.push(...taggedField(3, bytesToWords(hint)));
  }
  if (options.includePaymentSecret === true) {
    const secret = new Uint8Array(32).fill(0x03);
    words.push(...taggedField(16, bytesToWords(secret)));
  }
  for (const field of options.extraTaggedFields ?? []) {
    words.push(...taggedField(field.type, [...field.data]));
  }

  // 65-byte dummy signature (r || s || recovery) → 104 words exactly.
  const signature = new Uint8Array(65).fill(0x04);
  words.push(...bytesToWords(signature));

  return bech32.encode(prefix, words, false);
};

/** Build a valid segwit address for any network/version/program length. */
export const encodeTestSegwitAddress = (
  network: BitcoinNetwork,
  witnessVersion: number,
  programLength: number,
  fillByte = 0x05,
  variant: "auto" | "bech32" | "bech32m" = "auto",
): string => {
  const program = new Uint8Array(programLength).fill(fillByte);
  const words = [witnessVersion, ...bech32.toWords(program)];
  const encoder =
    variant === "auto"
      ? witnessVersion === 0
        ? bech32
        : bech32m
      : variant === "bech32"
        ? bech32
        : bech32m;
  return encoder.encode(ADDRESS_HRP[network], words, 90);
};

/** Encode a URL as an LNURL bech32 string. */
export const encodeTestLnurl = (url: string): string =>
  bech32.encode("lnurl", bech32.toWords(new TextEncoder().encode(url)), false);

/** Build a checksum-valid base58check address with any version byte. */
export const encodeTestBase58Address = (
  versionByte: number,
  fillByte = 0x06,
): string => {
  const body = new Uint8Array(21);
  body[0] = versionByte;
  body.fill(fillByte, 1);
  const checksum = sha256(sha256(body)).slice(0, 4);
  const payload = new Uint8Array(25);
  payload.set(body, 0);
  payload.set(checksum, 21);
  return base58.encode(payload);
};
