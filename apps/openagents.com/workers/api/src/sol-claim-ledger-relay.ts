/**
 * Sol claim-ledger signing + relay bridge (forge Stage 2, #9185).
 *
 * `@openagentsinc/forge-protocol` owns the pure projection of the Sol
 * cross-session claim record to and from unsigned NIP-34 event templates
 * (`SolClaimLedgerEvent` = `{ kind, tags, content }`). By design it never
 * signs or transports. This module is the missing wire layer: it takes those
 * projections, signs them with the owned `nostr-effect` signer, serializes
 * them into NIP-01 relay frames, parses relay frames back, verifies the
 * signature, and recovers the original typed claim record.
 *
 * The round trip proved by the companion test is:
 *   build record -> project -> sign -> serialize (relay EVENT frame)
 *     -> parse frame -> verify signature -> project back -> recover record
 *
 * Boundaries:
 * - The signer secret key is a 32-byte hex value passed in per call. It never
 *   appears in the returned frame, the recovered record, error messages, or
 *   any log. Only the derived public key (`signer`) and event id are surfaced.
 * - This module owns transport shape (signing, serialization, filters), not
 *   claim-protocol semantics. Staleness, collision, and coordination rules
 *   stay in `@openagentsinc/forge-protocol`.
 * - Event durability (an `EventStore` on the owned relay) is a separate slice.
 *   This module produces and consumes the frames such a store would persist.
 */
import {
  type SolClaim,
  type SolClaimLedgerEvent,
  type SolClaimRelease,
  type SolClaimStatus,
  type SolClaimWorkItem,
  SOL_CLAIM_ISSUE_KIND,
  SOL_CLAIM_STATUS_APPLIED_KIND,
  SOL_CLAIM_STATUS_CLOSED_KIND,
  SOL_CLAIM_STATUS_DRAFT_KIND,
  SOL_CLAIM_STATUS_OPEN_KIND,
  parseSolClaimEvent,
  parseSolClaimReleaseEvent,
  parseSolClaimStatusEvent,
  parseSolWorkItemEvent,
  solClaimLedgerEntryType,
  solClaimReleaseToLedgerEvent,
  solClaimStatusToLedgerEvent,
  solClaimToLedgerEvent,
  solWorkItemToLedgerEvent,
} from "@openagentsinc/forge-protocol";
import {
  type Event as SignedNostrEvent,
  type EventTemplate,
  finalizeEvent,
  getPublicKey,
  verifyEvent,
} from "nostr-effect/pure";

export type { SignedNostrEvent };

/** The five NIP-34 kinds the Sol claim ledger uses, in ascending order. */
export const SOL_CLAIM_LEDGER_KINDS: ReadonlyArray<number> = [
  SOL_CLAIM_ISSUE_KIND,
  SOL_CLAIM_STATUS_OPEN_KIND,
  SOL_CLAIM_STATUS_APPLIED_KIND,
  SOL_CLAIM_STATUS_CLOSED_KIND,
  SOL_CLAIM_STATUS_DRAFT_KIND,
];

const SecretKeyHexPattern = /^[0-9a-f]{64}$/i;

/** Raised when the signer key is not a 32-byte hex value. Never echoes the key. */
export class SolClaimLedgerSignerKeyError extends Error {
  readonly _tag = "SolClaimLedgerSignerKeyError";

  constructor() {
    super("Sol claim ledger signer key must be a 64-character hex string");
    this.name = "SolClaimLedgerSignerKeyError";
  }
}

/** Raised when a relay frame is malformed or is not a NIP-01 EVENT frame. */
export class SolClaimLedgerRelayFrameError extends Error {
  readonly _tag = "SolClaimLedgerRelayFrameError";

  constructor(reason: string) {
    super(reason);
    this.name = "SolClaimLedgerRelayFrameError";
  }
}

/** Raised when a received event fails signature verification. */
export class SolClaimLedgerSignatureError extends Error {
  readonly _tag = "SolClaimLedgerSignatureError";

  constructor() {
    super("Sol claim ledger event failed signature verification");
    this.name = "SolClaimLedgerSignatureError";
  }
}

/** Raised when a verified event is not a Sol claim-ledger entry. */
export class SolClaimLedgerNotAnEntryError extends Error {
  readonly _tag = "SolClaimLedgerNotAnEntryError";

  constructor(kind: number) {
    super(`Event kind ${kind} is not a tagged Sol claim-ledger entry`);
    this.name = "SolClaimLedgerNotAnEntryError";
  }
}

const hexToBytes = (hex: string): Uint8Array => {
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
};

export type SolClaimLedgerSignOptions = Readonly<{
  /** 32-byte (64-hex) Nostr secret key for the ledger-writing identity. */
  secretKeyHex: string;
  /** Event `created_at` in whole epoch seconds. */
  createdAtEpochSeconds: number;
}>;

/**
 * Sign an unsigned Sol claim-ledger projection into a complete NIP-01 event.
 *
 * The projection's readonly tags are cloned into fresh mutable arrays before
 * signing so the caller's value is never mutated and `finalizeEvent`'s in-place
 * write has an owned target.
 */
export const signSolClaimLedgerEvent = (
  event: SolClaimLedgerEvent,
  options: SolClaimLedgerSignOptions,
): SignedNostrEvent => {
  if (!SecretKeyHexPattern.test(options.secretKeyHex)) {
    throw new SolClaimLedgerSignerKeyError();
  }
  if (!Number.isInteger(options.createdAtEpochSeconds) || options.createdAtEpochSeconds < 0) {
    throw new SolClaimLedgerRelayFrameError(
      "created_at must be a non-negative integer number of epoch seconds",
    );
  }
  const template: EventTemplate = {
    kind: event.kind,
    tags: event.tags.map((tag) => [...tag]),
    content: event.content,
    created_at: options.createdAtEpochSeconds,
  };
  const secretKey = hexToBytes(options.secretKeyHex);
  try {
    return finalizeEvent(template, secretKey);
  } finally {
    // Zero the key material as soon as signing completes.
    secretKey.fill(0);
  }
};

/** Public key (64-hex) for a signer secret key, without signing anything. */
export const solClaimLedgerSigner = (secretKeyHex: string): string => {
  if (!SecretKeyHexPattern.test(secretKeyHex)) {
    throw new SolClaimLedgerSignerKeyError();
  }
  const secretKey = hexToBytes(secretKeyHex);
  try {
    return getPublicKey(secretKey);
  } finally {
    secretKey.fill(0);
  }
};

/** Serialize a signed event into a NIP-01 client->relay publish frame. */
export const toRelayPublishMessage = (event: SignedNostrEvent): string =>
  JSON.stringify(["EVENT", event]);

/**
 * Serialize a signed event into a NIP-01 relay->client subscription delivery
 * frame for a given subscription id.
 */
export const toRelaySubscriptionMessage = (
  subscriptionId: string,
  event: SignedNostrEvent,
): string => JSON.stringify(["EVENT", subscriptionId, event]);

const isStringArray = (value: unknown): value is Array<string> =>
  Array.isArray(value) && value.every((entry) => typeof entry === "string");

const coerceSignedEvent = (candidate: unknown): SignedNostrEvent => {
  if (candidate === null || typeof candidate !== "object") {
    throw new SolClaimLedgerRelayFrameError("EVENT frame payload is not an object");
  }
  const record = candidate as Record<string, unknown>;
  if (typeof record.kind !== "number") {
    throw new SolClaimLedgerRelayFrameError("EVENT payload is missing a numeric kind");
  }
  if (typeof record.content !== "string") {
    throw new SolClaimLedgerRelayFrameError("EVENT payload is missing string content");
  }
  if (typeof record.created_at !== "number") {
    throw new SolClaimLedgerRelayFrameError("EVENT payload is missing numeric created_at");
  }
  if (
    typeof record.pubkey !== "string" ||
    typeof record.id !== "string" ||
    typeof record.sig !== "string"
  ) {
    throw new SolClaimLedgerRelayFrameError("EVENT payload is missing id/pubkey/sig");
  }
  if (!Array.isArray(record.tags) || !record.tags.every(isStringArray)) {
    throw new SolClaimLedgerRelayFrameError("EVENT payload tags are not a string[][]");
  }
  return {
    kind: record.kind,
    content: record.content,
    created_at: record.created_at,
    pubkey: record.pubkey,
    id: record.id,
    sig: record.sig,
    tags: record.tags as Array<Array<string>>,
  };
};

/**
 * Parse a NIP-01 EVENT relay frame back into a signed event. Accepts both the
 * client->relay publish shape `["EVENT", event]` and the relay->client
 * subscription shape `["EVENT", subscriptionId, event]`. Signature is NOT
 * verified here; call `verifiedSolClaimLedgerEntry`.
 */
export const parseRelayEventMessage = (raw: string): SignedNostrEvent => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new SolClaimLedgerRelayFrameError("relay frame is not valid JSON");
  }
  if (!Array.isArray(parsed) || parsed[0] !== "EVENT") {
    throw new SolClaimLedgerRelayFrameError("relay frame is not a NIP-01 EVENT message");
  }
  if (parsed.length === 2) {
    return coerceSignedEvent(parsed[1]);
  }
  if (parsed.length === 3 && typeof parsed[1] === "string") {
    return coerceSignedEvent(parsed[2]);
  }
  throw new SolClaimLedgerRelayFrameError("EVENT frame has an unexpected shape");
};

/** Project a signed event down to the transport-agnostic ledger projection. */
export const signedEventToLedgerEvent = (event: SignedNostrEvent): SolClaimLedgerEvent => ({
  kind: event.kind,
  tags: event.tags.map((tag) => [...tag]),
  content: event.content,
});

/** A verified Sol claim-ledger entry recovered from a signed event. */
export type VerifiedSolClaimLedgerEntry =
  | Readonly<{
      type: "work_item";
      workItem: SolClaimWorkItem;
      signer: string;
      eventId: string;
      createdAtEpochSeconds: number;
    }>
  | Readonly<{
      type: "claim";
      claim: SolClaim;
      signer: string;
      eventId: string;
      createdAtEpochSeconds: number;
    }>
  | Readonly<{
      type: "claim_status";
      claimStatus: SolClaimStatus;
      signer: string;
      eventId: string;
      createdAtEpochSeconds: number;
    }>
  | Readonly<{
      type: "claim_release";
      claimRelease: SolClaimRelease;
      signer: string;
      eventId: string;
      createdAtEpochSeconds: number;
    }>;

/**
 * Verify a signed event's signature and recover the typed Sol ledger entry it
 * carries. Throws `SolClaimLedgerSignatureError` on a bad signature and
 * `SolClaimLedgerNotAnEntryError` when the event is not a tagged ledger entry.
 */
export const verifiedSolClaimLedgerEntry = (
  event: SignedNostrEvent,
): VerifiedSolClaimLedgerEntry => {
  if (!verifyEvent(event)) {
    throw new SolClaimLedgerSignatureError();
  }
  const ledgerEvent = signedEventToLedgerEvent(event);
  const entryType = solClaimLedgerEntryType(ledgerEvent);
  if (entryType === null) {
    throw new SolClaimLedgerNotAnEntryError(event.kind);
  }
  const common = {
    signer: event.pubkey,
    eventId: event.id,
    createdAtEpochSeconds: event.created_at,
  } as const;
  switch (entryType) {
    case "work_item":
      return { type: "work_item", workItem: parseSolWorkItemEvent(ledgerEvent), ...common };
    case "claim":
      return { type: "claim", claim: parseSolClaimEvent(ledgerEvent), ...common };
    case "claim_status":
      return {
        type: "claim_status",
        claimStatus: parseSolClaimStatusEvent(ledgerEvent),
        ...common,
      };
    case "claim_release":
      return {
        type: "claim_release",
        claimRelease: parseSolClaimReleaseEvent(ledgerEvent),
        ...common,
      };
  }
};

/**
 * Verify and recover a Sol ledger entry directly from a relay EVENT frame.
 * This is the full read path an agent uses when consuming ledger events from a
 * relay subscription.
 */
export const recoverSolClaimLedgerEntryFromRelayMessage = (
  raw: string,
): VerifiedSolClaimLedgerEntry => verifiedSolClaimLedgerEntry(parseRelayEventMessage(raw));

// =============================================================================
// Typed write helpers: record -> signed relay publish frame
// =============================================================================

export const solWorkItemToRelayPublishMessage = (
  item: SolClaimWorkItem,
  options: SolClaimLedgerSignOptions,
): string =>
  toRelayPublishMessage(signSolClaimLedgerEvent(solWorkItemToLedgerEvent(item), options));

export const solClaimToRelayPublishMessage = (
  claim: SolClaim,
  options: SolClaimLedgerSignOptions,
): string => toRelayPublishMessage(signSolClaimLedgerEvent(solClaimToLedgerEvent(claim), options));

export const solClaimStatusToRelayPublishMessage = (
  status: SolClaimStatus,
  options: SolClaimLedgerSignOptions,
): string =>
  toRelayPublishMessage(signSolClaimLedgerEvent(solClaimStatusToLedgerEvent(status), options));

export const solClaimReleaseToRelayPublishMessage = (
  release: SolClaimRelease,
  options: SolClaimLedgerSignOptions,
): string =>
  toRelayPublishMessage(signSolClaimLedgerEvent(solClaimReleaseToLedgerEvent(release), options));

// =============================================================================
// Read-side subscription filter for a repository coordinate
// =============================================================================

/**
 * A NIP-01 request filter, narrowed to the fields the Sol claim ledger uses.
 * `#a` selects the NIP-34 repository coordinate (`30617:<pubkey>:<repoId>`).
 */
export type SolClaimLedgerFilter = Readonly<{
  kinds: ReadonlyArray<number>;
  "#a"?: ReadonlyArray<string>;
  "#sol.work_item"?: ReadonlyArray<string>;
  authors?: ReadonlyArray<string>;
  since?: number;
  limit?: number;
}>;

export type SolClaimLedgerFilterOptions = Readonly<{
  /** Restrict to a single Sol work item ref. */
  workItemRef?: string;
  /** Restrict to specific signer pubkeys. */
  authors?: ReadonlyArray<string>;
  /** Only events at or after this epoch-second cutoff. */
  sinceEpochSeconds?: number;
  /** Cap the number of returned events. */
  limit?: number;
  /** Restrict to a subset of the ledger kinds (defaults to all five). */
  kinds?: ReadonlyArray<number>;
}>;

/**
 * Build a subscription filter that reads the whole Sol claim ledger for one
 * repository coordinate: work items, claims, claim-status, and claim-release.
 * This is the "each agent reads open work + claims from the repository
 * coordinate" read contract.
 */
export const solClaimLedgerRepositoryFilter = (
  repositoryCoordinate: string,
  options: SolClaimLedgerFilterOptions = {},
): SolClaimLedgerFilter => {
  const kinds = options.kinds !== undefined ? [...options.kinds] : [...SOL_CLAIM_LEDGER_KINDS];
  const filter: {
    kinds: Array<number>;
    "#a": Array<string>;
    "#sol.work_item"?: Array<string>;
    authors?: Array<string>;
    since?: number;
    limit?: number;
  } = {
    kinds,
    "#a": [repositoryCoordinate],
  };
  if (options.workItemRef !== undefined) {
    filter["#sol.work_item"] = [options.workItemRef];
  }
  if (options.authors !== undefined) {
    filter.authors = [...options.authors];
  }
  if (options.sinceEpochSeconds !== undefined) {
    filter.since = options.sinceEpochSeconds;
  }
  if (options.limit !== undefined) {
    filter.limit = options.limit;
  }
  return filter;
};

/** Serialize a subscription REQ frame for a repository-coordinate read. */
export const toRelayRequestMessage = (
  subscriptionId: string,
  filter: SolClaimLedgerFilter,
): string => JSON.stringify(["REQ", subscriptionId, filter]);
