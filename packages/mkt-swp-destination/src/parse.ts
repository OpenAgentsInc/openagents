/**
 * The one shared destination parser (issue #9317 §2): the address field,
 * the invoice field, and the QR scanner all route through
 * `parseDestination`, and its typed discriminated result survives to the
 * presentation layer.
 *
 * Parse order mirrors the teardown §3.5 shape: empty check, URI
 * extraction, EVM/token rejection, then syntactic family dispatch with the
 * expected rail's family probed first. All families here have disjoint
 * prefixes, so the probe order is deterministic rather than heuristic.
 */
import {
  looksLikeBase58Address,
  looksLikeBech32Address,
  parseOnchainAddress,
} from "./address.js";
import { looksLikeBolt11, parseBolt11 } from "./bolt11.js";
import { bech32 } from "@scure/base";
import type {
  Bip21Details,
  BitcoinNetwork,
  Bolt11InvoiceDestination,
  DeferredDestination,
  DestinationParseFailure,
  DestinationParseResult,
  DestinationRail,
  OnchainAddressDestination,
  ParsedDestination,
} from "./model.js";
import { destinationRail } from "./model.js";

export interface DestinationParseContext {
  /** The rail the active field expects (from the current direction). */
  readonly rail: DestinationRail;
  readonly network: BitcoinNetwork;
  /** Injected clock, seconds since epoch; keeps the parser pure. */
  readonly nowSeconds: number;
}

const ok = (
  destination: ParsedDestination,
  context: DestinationParseContext,
): DestinationParseResult => {
  const rail = destinationRail(destination);
  return {
    ok: true,
    destination,
    requiredRail: rail === "both" || rail === context.rail ? null : rail,
  };
};

const fail = (failure: DestinationParseFailure): DestinationParseResult => ({
  ok: false,
  failure,
});

const EVM_ADDRESS = /^0x[0-9a-fA-F]{40}$/;

const SATS_PER_BTC = 100_000_000n;

/** BIP-21 amounts are read case-insensitively; malformed values are absent. */
const parseBip21Amount = (raw: string | null): bigint | null => {
  if (raw === null) return null;
  const match = /^([0-9]+)(?:\.([0-9]{1,8}))?$/.exec(raw);
  if (match === null) return null;
  const whole = BigInt(match[1] ?? "0");
  const fraction = match[2] ?? "";
  const fractionSats = fraction === "" ? 0n : BigInt(fraction.padEnd(8, "0"));
  const sats = whole * SATS_PER_BTC + fractionSats;
  return sats > 0n ? sats : null;
};

interface ExtractedUri {
  readonly scheme: "bitcoin" | "lightning" | null;
  readonly body: string;
  readonly params: ReadonlyMap<string, string>;
  readonly requiredUnsupported: string | null;
}

const KNOWN_BIP21_PARAMS = new Set(["amount", "label", "message", "lightning"]);

const extractUri = (text: string): ExtractedUri | null => {
  const match = /^([A-Za-z][A-Za-z0-9+.-]*):(.*)$/s.exec(text);
  if (match === null) return null;
  const scheme = (match[1] ?? "").toLowerCase();
  const rest = match[2] ?? "";
  if (scheme !== "bitcoin" && scheme !== "lightning") {
    return {
      scheme: null,
      body: text,
      params: new Map(),
      requiredUnsupported: null,
    };
  }
  const withoutSlashes = rest.startsWith("//") ? rest.slice(2) : rest;
  const queryIndex = withoutSlashes.indexOf("?");
  const body =
    queryIndex === -1 ? withoutSlashes : withoutSlashes.slice(0, queryIndex);
  const params = new Map<string, string>();
  let requiredUnsupported: string | null = null;
  if (queryIndex !== -1) {
    for (const pair of withoutSlashes.slice(queryIndex + 1).split("&")) {
      if (pair === "") continue;
      const eq = pair.indexOf("=");
      const rawKey = eq === -1 ? pair : pair.slice(0, eq);
      const key = decodeURIComponent(rawKey).toLowerCase();
      const value = eq === -1 ? "" : decodeURIComponent(pair.slice(eq + 1));
      if (key.startsWith("req-") && !KNOWN_BIP21_PARAMS.has(key.slice(4))) {
        requiredUnsupported ??= key;
      }
      const normalisedKey = key.startsWith("req-") ? key.slice(4) : key;
      if (!params.has(normalisedKey)) params.set(normalisedKey, value);
    }
  }
  return { scheme, body, params, requiredUnsupported };
};

const LIGHTNING_ADDRESS =
  /^([a-z0-9](?:[a-z0-9._+-]{0,63}))@((?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,})$/i;

const BECH32_BODY = /^[qpzry9x8gf2tvdw0s3jn54khce6mua7l]+$/;

const parseLnurl = (
  lower: string,
): DeferredDestination | DestinationParseFailure => {
  try {
    const decoded = bech32.decode(lower as `${string}1${string}`, false);
    if (decoded.prefix !== "lnurl") {
      return { mode: "lnurl_malformed", swpError: null };
    }
    const url = new TextDecoder().decode(bech32.fromWords([...decoded.words]));
    if (!/^https?:\/\//i.test(url)) {
      return { mode: "lnurl_malformed", swpError: null };
    }
    return { kind: "deferred_lnurl", rail: "lightning", url };
  } catch {
    return { mode: "lnurl_malformed", swpError: null };
  }
};

const parseBolt12Offer = (
  lower: string,
): DeferredDestination | DestinationParseFailure => {
  // BOLT12 offers use the bech32 charset without a checksum and permit `+`
  // continuations (stripped with surrounding whitespace before this point).
  const body = lower.slice("lno1".length);
  if (body.length < 16 || !BECH32_BODY.test(body)) {
    return { mode: "offer_malformed", swpError: null };
  }
  return { kind: "deferred_bolt12_offer", rail: "lightning", offer: lower };
};

type LightningDestination = Bolt11InvoiceDestination | DeferredDestination;

const parseLightningBody = (
  body: string,
  context: DestinationParseContext,
): {
  readonly destination?: LightningDestination;
  readonly failure?: DestinationParseFailure;
} => {
  // Lightning addresses are legitimately mixed-case in their local part;
  // check them before bech32 case normalisation applies.
  if (LIGHTNING_ADDRESS.test(body)) {
    const match = LIGHTNING_ADDRESS.exec(body);
    const user = match?.[1] ?? "";
    const domain = (match?.[2] ?? "").toLowerCase();
    return {
      destination: {
        kind: "deferred_lightning_address",
        rail: "lightning",
        address: `${user}@${domain}`,
        resolveUrl: `https://${domain}/.well-known/lnurlp/${user}`,
      },
    };
  }
  const lower = normaliseCase(body);
  if (lower === null) {
    return { failure: { mode: "invoice_checksum_invalid", swpError: null } };
  }
  if (lower.startsWith("lnurl1")) {
    const parsed = parseLnurl(lower);
    return "kind" in parsed ? { destination: parsed } : { failure: parsed };
  }
  if (lower.startsWith("lno1")) {
    const parsed = parseBolt12Offer(lower);
    return "kind" in parsed ? { destination: parsed } : { failure: parsed };
  }
  if (looksLikeBolt11(lower)) {
    const parsed = parseBolt11(lower, context);
    return parsed.ok ? { destination: parsed.invoice } : { failure: parsed.failure };
  }
  return { failure: { mode: "unrecognized", swpError: null } };
};

/**
 * Bech32 strings must be all-lowercase or all-uppercase (QR alternate
 * form). Mixed case is invalid; base58 and lightning addresses are handled
 * before this normalisation applies.
 */
const normaliseCase = (text: string): string | null => {
  const lower = text.toLowerCase();
  const upper = text.toUpperCase();
  if (text === lower) return lower;
  if (text === upper) return lower;
  return null;
};

const parseBitcoinUri = (
  uri: ExtractedUri,
  context: DestinationParseContext,
): DestinationParseResult => {
  if (uri.requiredUnsupported !== null) {
    return fail({
      mode: "uri_required_param_unsupported",
      swpError: null,
      param: uri.requiredUnsupported,
    });
  }
  const bip21: Bip21Details = {
    amountSats: parseBip21Amount(uri.params.get("amount") ?? null),
    label: uri.params.get("label") ?? null,
    message: uri.params.get("message") ?? null,
  };

  const lightningValue = uri.params.get("lightning") ?? null;
  const addressText = uri.body.trim();

  if (addressText === "" && lightningValue === null) {
    return fail({ mode: "unrecognized", swpError: null });
  }

  let onchain: OnchainAddressDestination | null = null;
  let onchainFailure: DestinationParseFailure | null = null;
  if (addressText !== "") {
    const candidate = normaliseCandidate(addressText);
    if (candidate === null) {
      onchainFailure = { mode: "address_checksum_invalid", swpError: null };
    } else {
      const parsed = parseOnchainAddress(candidate, context.network, bip21);
      if (parsed.ok) onchain = parsed.address;
      else onchainFailure = parsed.failure;
    }
  }

  let lightning: LightningDestination | null = null;
  let lightningFailure: DestinationParseFailure | null = null;
  if (lightningValue !== null && lightningValue !== "") {
    const parsed = parseLightningBody(lightningValue.trim(), context);
    if (parsed.destination !== undefined) {
      lightning = parsed.destination;
    } else {
      lightningFailure = parsed.failure ?? {
        mode: "unrecognized",
        swpError: null,
      };
    }
  }

  if (onchain === null && lightning !== null) {
    // No usable on-chain leg: behave as a plain Lightning destination.
    return ok(lightning, context);
  }
  if (onchain === null) {
    return fail(
      onchainFailure ??
        lightningFailure ?? { mode: "unrecognized", swpError: null },
    );
  }
  if (lightning === null && lightningFailure === null) {
    return ok(onchain, context);
  }

  // Unified: one effective amount only (issue #9317 §6). An amount-bearing
  // invoice suppresses a present BIP-21 amount.
  const bip21AmountSuppressed =
    lightning !== null &&
    lightning.kind === "bolt11_invoice" &&
    bip21.amountSats !== null;
  return ok(
    {
      kind: "unified",
      onchain,
      lightning,
      lightningFailure,
      bip21AmountSuppressed,
    },
    context,
  );
};

/** Bech32-family candidates get case-normalised; others pass through. */
const normaliseCandidate = (text: string): string | null => {
  const lower = text.toLowerCase();
  if (
    looksLikeBech32Address(lower) ||
    looksLikeBolt11(lower) ||
    lower.startsWith("lnurl1") ||
    lower.startsWith("lno1")
  ) {
    return normaliseCase(text);
  }
  return text;
};

/**
 * Parse a destination from typing, paste, or a QR scan. Never throws; the
 * discriminated failure survives to the presentation layer.
 */
export const parseDestination = (
  input: string,
  context: DestinationParseContext,
): DestinationParseResult => {
  // BOLT12 offers may arrive as `+`-joined continuation chunks.
  const text = input.trim().replace(/\+\s+/g, "");
  if (text === "") return fail({ mode: "empty", swpError: null });

  if (EVM_ADDRESS.test(text)) {
    return fail({ mode: "evm_destination_unsupported", swpError: null });
  }

  const uri = extractUri(text);
  if (uri !== null && uri.scheme === null) {
    const scheme = (
      /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(text)?.[1] ?? ""
    ).toLowerCase();
    if (scheme === "ethereum") {
      return fail({ mode: "evm_destination_unsupported", swpError: null });
    }
    // Only reject schemes that clearly are not destinations; anything else
    // colon-bearing falls through to `unrecognized`.
    if (/^(http|https|litecoin|monero|bitcoincash)$/.test(scheme)) {
      return fail({ mode: "uri_scheme_unsupported", swpError: null, scheme });
    }
    return fail({ mode: "unrecognized", swpError: null });
  }

  if (uri !== null && uri.scheme === "bitcoin") {
    return parseBitcoinUri(uri, context);
  }

  if (uri !== null && uri.scheme === "lightning") {
    const parsed = parseLightningBody(uri.body.trim(), context);
    if (parsed.destination !== undefined) return ok(parsed.destination, context);
    return fail(parsed.failure ?? { mode: "unrecognized", swpError: null });
  }

  // Bare string: probe the expected rail's family first (both families
  // have disjoint prefixes, so this is deterministic either way).
  const lightningFamily = (): DestinationParseResult | null => {
    const lower = text.toLowerCase();
    if (
      !looksLikeBolt11(lower) &&
      !lower.startsWith("lnurl1") &&
      !lower.startsWith("lno1") &&
      !LIGHTNING_ADDRESS.test(text)
    ) {
      return null;
    }
    const parsed = parseLightningBody(text, context);
    if (parsed.destination !== undefined) return ok(parsed.destination, context);
    return fail(parsed.failure ?? { mode: "unrecognized", swpError: null });
  };
  const chainFamily = (): DestinationParseResult | null => {
    const lower = text.toLowerCase();
    if (!looksLikeBech32Address(lower) && !looksLikeBase58Address(text)) {
      return null;
    }
    const candidate = normaliseCandidate(text);
    if (candidate === null) {
      return fail({ mode: "address_checksum_invalid", swpError: null });
    }
    const parsed = parseOnchainAddress(candidate, context.network, null);
    if (parsed.ok) return ok(parsed.address, context);
    return fail(parsed.failure);
  };

  const ordered =
    context.rail === "lightning"
      ? [lightningFamily, chainFamily]
      : [chainFamily, lightningFamily];
  for (const family of ordered) {
    const result = family();
    if (result !== null) return result;
  }
  return fail({ mode: "unrecognized", swpError: null });
};
