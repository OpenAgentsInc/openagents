/**
 * IDR-03 public-safe decode result and typed decode error.
 *
 * A decoder produces two things: a bounded `RecoveredSecret` (see `boundary.ts`)
 * and a PUBLIC-safe `CandidateDecodeResult`. The result carries the format
 * label, the format version, the source label, the success/failure status, and
 * — once a caller derives them through the bounded boundary — the public
 * identifiers. It NEVER carries the mnemonic, `nsec`, raw key, or seed.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Schema as S } from "effect";
import { HISTORICAL_FORMAT_IDS, Npub } from "../contract/index.ts";
import type { RecoveredSecret } from "./boundary.ts";

/** A lowercase hex string. */
const HexString = S.String.check(S.isPattern(/^[0-9a-f]+$/));

/** The historical-format literal set, as an Effect Schema. */
export const HistoricalFormatIdSchema = S.Literals(HISTORICAL_FORMAT_IDS);

/**
 * The decode status of one candidate.
 *
 * - `decoded`: the mnemonic was recovered into a bounded `RecoveredSecret`.
 * - `owner_attended_required`: the envelope is platform-opaque (for example a
 *   normal Electron OS `safeStorage` record). An offline decode is impossible;
 *   an owner-attended run is required. The tool records this and continues other
 *   checks rather than failing the whole recovery.
 */
export const CandidateDecodeStatus = S.Literals(["decoded", "owner_attended_required"]);
export type CandidateDecodeStatus = typeof CandidateDecodeStatus.Type;

/**
 * The public identifiers a decoded candidate yields once a caller derives them
 * through the bounded boundary. Every field is public and safe to log or persist.
 */
export const PublicDecodedIdentity = S.Struct({
  npub: Npub,
  nostrPublicKeyHex: HexString,
  sparkPublicKeyHex: HexString,
  sparkBip32FingerprintHex: HexString,
});
export interface PublicDecodedIdentity extends S.Schema.Type<typeof PublicDecodedIdentity> {}

/**
 * The public-safe decode result. It describes the source and outcome with public
 * data only. `publicIdentity` is `null` until a caller derives it through the
 * bounded boundary; see `deriveAndAttachPublicIdentity` in `decode-candidate.ts`.
 */
export const CandidateDecodeResult = S.Struct({
  format: HistoricalFormatIdSchema,
  /** A short public description of the source. */
  formatLabel: S.String.check(S.isMinLength(1)),
  /** The decoded format version string. */
  formatVersion: S.String.check(S.isMinLength(1)),
  /** A PUBLIC source label, never a raw private path. */
  sourcePathLabel: S.String.check(S.isMinLength(1)),
  /** The decode status. */
  status: CandidateDecodeStatus,
  /** Whether a mnemonic was recovered into a bounded `RecoveredSecret`. */
  decoded: S.Boolean,
  /** The public identifiers, once derived through the bounded boundary. */
  publicIdentity: S.NullOr(PublicDecodedIdentity),
  /** A public-safe note, for example why an owner-attended run is required. */
  note: S.optionalKey(S.String),
});
export interface CandidateDecodeResult extends S.Schema.Type<typeof CandidateDecodeResult> {}

/** The decode-failure reasons. None carries secret material. */
export const DecodeFailureReason = S.Literals([
  "malformed_envelope",
  "invalid_mnemonic",
  "decrypt_failed",
  "missing_password",
]);
export type DecodeFailureReason = typeof DecodeFailureReason.Type;

/**
 * A typed decode failure. It carries the format and a coarse reason ONLY. It
 * never carries the mnemonic, a cipher key, a password, or any plaintext, so a
 * logged error can never leak the secret.
 */
export class DecodeCandidateError extends S.TaggedErrorClass<DecodeCandidateError>()(
  "sovereign-identity.DecodeCandidateError",
  {
    format: HistoricalFormatIdSchema,
    reason: DecodeFailureReason,
  },
) {}

/**
 * The output of one decode: the PUBLIC-safe result plus, when a phrase was
 * recovered, the bounded `RecoveredSecret`. The secret is `null` for an
 * owner-attended (platform-opaque) candidate. The secret is the ONLY path to the
 * mnemonic; the result never carries it.
 */
export interface DecodedCandidate {
  readonly result: CandidateDecodeResult;
  readonly secret: RecoveredSecret | null;
}
