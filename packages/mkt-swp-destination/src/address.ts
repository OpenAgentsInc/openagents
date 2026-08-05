/**
 * Network-aware on-chain address classification for destination entry
 * (issue #9317 §1, §4 pre-check half).
 *
 * This validates what address *bytes* alone can prove: encoding, checksum,
 * witness version/program shape, and network class. It never authorises
 * anything. The MKT-SWP engine independently parses scripts and Taproot
 * trees from bytes and re-derives the output key/address (§7.1 steps 3–5);
 * a counterparty-supplied address is only trusted after that re-derivation,
 * surfaced through the `verify.ts` verdict port.
 */
import { sha256 } from "@noble/hashes/sha256";
import { base58, bech32, bech32m } from "@scure/base";

import type {
  AddressNetworkClass,
  BitcoinNetwork,
  DestinationParseFailure,
  OnchainAddressDestination,
} from "./model.js";

export type ParseAddressResult =
  | { readonly ok: true; readonly address: OnchainAddressDestination }
  | { readonly ok: false; readonly failure: DestinationParseFailure };

const failure = (f: DestinationParseFailure): ParseAddressResult => ({
  ok: false,
  failure: f,
});

const BECH32_HRP_CLASSES: Readonly<Record<string, AddressNetworkClass>> = {
  bc: "mainnet",
  tb: "test",
  bcrt: "regtest",
};

interface Base58Version {
  readonly networkClass: AddressNetworkClass;
  readonly addressType: "p2pkh" | "p2sh";
}

const BASE58_VERSIONS: Readonly<Record<number, Base58Version>> = {
  0x00: { networkClass: "mainnet", addressType: "p2pkh" },
  0x05: { networkClass: "mainnet", addressType: "p2sh" },
  0x6f: { networkClass: "test", addressType: "p2pkh" },
  0xc4: { networkClass: "test", addressType: "p2sh" },
};

const BASE58_ALPHABET = /^[1-9A-HJ-NP-Za-km-z]+$/;

/**
 * Whether a network class recovered from address bytes satisfies the
 * expected network. `tb`/testnet version bytes cover testnet and signet;
 * base58 has no regtest-specific bytes, so regtest accepts the test class
 * for base58 addresses.
 */
export const networkClassMatches = (
  expected: BitcoinNetwork,
  actual: AddressNetworkClass,
  encoding: "bech32" | "base58",
): boolean => {
  switch (expected) {
    case "mainnet":
      return actual === "mainnet";
    case "testnet":
    case "signet":
      return actual === "test";
    case "regtest":
      return encoding === "bech32" ? actual === "regtest" : actual === "test";
  }
};

/** True when the (lowercased) text looks like a segwit address attempt. */
export const looksLikeBech32Address = (lower: string): boolean => {
  const separator = lower.lastIndexOf("1");
  if (separator <= 0) return false;
  return Object.hasOwn(BECH32_HRP_CLASSES, lower.slice(0, separator));
};

/**
 * True when the text could be a base58check address attempt. Deliberately
 * alphabet-and-length only: an unknown version byte changes the leading
 * character, and it must still reach `address_base58_version_unknown`
 * rather than collapsing into `unrecognized`.
 */
export const looksLikeBase58Address = (text: string): boolean =>
  text.length >= 26 && text.length <= 35 && BASE58_ALPHABET.test(text);

interface Bip21Amounts {
  readonly amountSats: bigint | null;
  readonly label: string | null;
  readonly message: string | null;
}

const parseBech32Address = (
  lower: string,
  expected: BitcoinNetwork,
  bip21: Bip21Amounts | null,
): ParseAddressResult => {
  const decodeWith = (
    variant: typeof bech32 | typeof bech32m,
  ): { prefix: string; words: number[] } | null => {
    try {
      const decoded = variant.decode(lower as `${string}1${string}`, 90);
      return { prefix: decoded.prefix, words: [...decoded.words] };
    } catch {
      return null;
    }
  };

  const asBech32 = decodeWith(bech32);
  const asBech32m = decodeWith(bech32m);
  const decoded = asBech32 ?? asBech32m;
  if (decoded === null) {
    return failure({ mode: "address_checksum_invalid", swpError: null });
  }

  const networkClass = BECH32_HRP_CLASSES[decoded.prefix];
  if (networkClass === undefined) {
    return failure({ mode: "unrecognized", swpError: null });
  }

  const [witnessVersion, ...programWords] = decoded.words;
  if (witnessVersion === undefined || witnessVersion > 16) {
    return failure({
      mode: "address_witness_program_invalid",
      swpError: null,
      witnessVersion: witnessVersion ?? -1,
      programLength: 0,
    });
  }

  // BIP350: v0 must use bech32, v1+ must use bech32m.
  const requiredVariant = witnessVersion === 0 ? asBech32 : asBech32m;
  if (requiredVariant === null) {
    return failure({
      mode: "address_encoding_mismatch",
      swpError: null,
      witnessVersion,
    });
  }

  let program: Uint8Array;
  try {
    program = bech32.fromWords(programWords);
  } catch {
    return failure({ mode: "address_checksum_invalid", swpError: null });
  }

  const programLength = program.length;
  const addressType =
    witnessVersion === 0 && programLength === 20
      ? "p2wpkh"
      : witnessVersion === 0 && programLength === 32
        ? "p2wsh"
        : witnessVersion === 1 && programLength === 32
          ? "p2tr"
          : null;
  if (addressType === null) {
    return failure({
      mode: "address_witness_program_invalid",
      swpError: null,
      witnessVersion,
      programLength,
    });
  }

  if (!networkClassMatches(expected, networkClass, "bech32")) {
    return failure({
      mode: "address_network_mismatch",
      swpError: null,
      expected,
      actual: networkClass,
    });
  }

  return {
    ok: true,
    address: {
      kind: "onchain_address",
      rail: "chain",
      address: lower,
      addressType,
      networkClass,
      bip21,
    },
  };
};

const parseBase58Address = (
  text: string,
  expected: BitcoinNetwork,
  bip21: Bip21Amounts | null,
): ParseAddressResult => {
  let payload: Uint8Array;
  try {
    payload = base58.decode(text);
  } catch {
    return failure({ mode: "address_checksum_invalid", swpError: null });
  }
  if (payload.length !== 25) {
    return failure({ mode: "address_checksum_invalid", swpError: null });
  }
  const body = payload.slice(0, 21);
  const checksum = payload.slice(21);
  const digest = sha256(sha256(body)).slice(0, 4);
  for (let i = 0; i < 4; i += 1) {
    if (checksum[i] !== digest[i]) {
      return failure({ mode: "address_checksum_invalid", swpError: null });
    }
  }

  const versionByte = body[0] ?? -1;
  const version = BASE58_VERSIONS[versionByte];
  if (version === undefined) {
    return failure({
      mode: "address_base58_version_unknown",
      swpError: null,
      versionByte,
    });
  }

  if (!networkClassMatches(expected, version.networkClass, "base58")) {
    return failure({
      mode: "address_network_mismatch",
      swpError: null,
      expected,
      actual: version.networkClass,
    });
  }

  return {
    ok: true,
    address: {
      kind: "onchain_address",
      rail: "chain",
      address: text,
      addressType: version.addressType,
      networkClass: version.networkClass,
      bip21,
    },
  };
};

/**
 * Parse an on-chain address candidate. `text` is trimmed; bech32 candidates
 * must already be lowercased by the shared parser (QR-style all-uppercase
 * input is normalised there). Base58 is case-significant and passed
 * verbatim.
 */
export const parseOnchainAddress = (
  text: string,
  expected: BitcoinNetwork,
  bip21: Bip21Amounts | null = null,
): ParseAddressResult => {
  if (looksLikeBech32Address(text.toLowerCase())) {
    return parseBech32Address(text.toLowerCase(), expected, bip21);
  }
  if (looksLikeBase58Address(text)) {
    return parseBase58Address(text, expected, bip21);
  }
  return failure({ mode: "unrecognized", swpError: null });
};
