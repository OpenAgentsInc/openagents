/**
 * Sovereign identity local-mode IPC contract (IDR-BS, #9103).
 *
 * The Electron main process owns the one BIP-39 mnemonic (read or created by the
 * existing Pylon identity loader) and the pure public derivation from
 * `@openagentsinc/sovereign-identity`. This file owns the ONLY renderer-visible
 * surface: a single additive, schema-validated IPC channel (`identity.status`)
 * whose payload is bounded and PUBLIC-safe.
 *
 * Security posture: the renderer is never part of the trusted boundary. It NEVER
 * learns the mnemonic, the `nsec`, the raw private key, or the BIP-39 seed. It
 * renders only the public projection: the availability status, the Nostr `npub`,
 * the Spark wallet public fingerprint, whether the identity was rehydrated from
 * an existing mnemonic or freshly created, and the frozen derivation profile id.
 * The `npub1` / hex-fingerprint patterns below are a defensive gate — an `nsec1`
 * or any raw secret shape can never pass the decoder.
 */
import { Schema } from "effect"

/** Additive IPC channel (main ↔ renderer). Public-safe payload only. */
export const IdentityStatusChannel = "openagents-desktop/identity-status" as const

export const IDENTITY_STATUS_SCHEMA_ID = "openagents.desktop.identity.status.v1" as const

// ---------------------------------------------------------------------------
// Bounded vocabularies (never free text).
// ---------------------------------------------------------------------------

/** Availability of the local sovereign identity projection. */
export const identityStatusValues = ["available", "unavailable"] as const
export type IdentityStatusValue = (typeof identityStatusValues)[number]

/** Whether the mnemonic was loaded from disk or freshly created this run. */
export const identitySourceValues = ["rehydrated", "created"] as const
export type IdentitySourceValue = (typeof identitySourceValues)[number]

/** The admitted Spark wallet mode (IDR-07). Status-only only — never a send mode. */
export const walletModeValues = ["status_only"] as const
export type WalletModeValue = (typeof walletModeValues)[number]

/** A public Nostr `npub` bech32 identifier. An `nsec1...` can never match. */
const Npub = Schema.String.check(Schema.isMaxLength(120), Schema.isPattern(/^npub1[a-z0-9]+$/))
/** A public wallet fingerprint as lower-hex (the Spark BIP-32 fingerprint). */
const WalletFingerprint = Schema.String.check(Schema.isMaxLength(64), Schema.isPattern(/^[0-9a-f]+$/))
/** A bounded public profile identifier (the frozen derivation profile id). */
const BoundedRef = Schema.String.check(Schema.isMaxLength(120), Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/))

// ---------------------------------------------------------------------------
// Status projection.
// ---------------------------------------------------------------------------

export const IdentityStatusSchema = Schema.Struct({
  schema: Schema.Literal(IDENTITY_STATUS_SCHEMA_ID),
  status: Schema.Literals(identityStatusValues),
  /**
   * The ONE canonical cross-surface identity reference (IDR-08). It is the `npub`
   * resolved by the shared `@openagentsinc/sovereign-identity` service, so Desktop
   * and Pylon carry the identical reference; null when unavailable. An `nsec1...`
   * or raw-secret shape can never match the `npub1` pattern.
   */
  identityRef: Schema.NullOr(Npub),
  /** The Nostr NIP-06 `npub`; null when unavailable. */
  npub: Schema.NullOr(Npub),
  /** The Spark wallet public fingerprint (lower-hex); null when unavailable. */
  walletFingerprint: Schema.NullOr(WalletFingerprint),
  /** `rehydrated` when an existing mnemonic was found, `created` for a fresh one. */
  source: Schema.NullOr(Schema.Literals(identitySourceValues)),
  /** The frozen derivation profile id; null when unavailable. */
  profileId: Schema.NullOr(BoundedRef),
  /**
   * The STATUS-ONLY Spark wallet mode (IDR-07). `status_only` when the app-side
   * Spark adapter opened the recovered wallet; null when unavailable. There is no
   * send mode on this surface.
   */
  walletMode: Schema.NullOr(Schema.Literals(walletModeValues)),
})
export type IdentityStatus = typeof IdentityStatusSchema.Type

// ---------------------------------------------------------------------------
// Decoder (returns null on invalid — the boundary never throws).
// ---------------------------------------------------------------------------

const decodeStatusExit = Schema.decodeUnknownExit(IdentityStatusSchema)

export const decodeIdentityStatus = (value: unknown): IdentityStatus | null => {
  const decoded = decodeStatusExit(value)
  return decoded._tag === "Success" ? decoded.value : null
}

// ---------------------------------------------------------------------------
// Public-safe constructor used at the boundary fallbacks.
// ---------------------------------------------------------------------------

/** The safe default when the identity is unreachable or the response invalid. */
export const unavailableIdentityStatus = (): IdentityStatus => ({
  schema: IDENTITY_STATUS_SCHEMA_ID,
  status: "unavailable",
  identityRef: null,
  npub: null,
  walletFingerprint: null,
  source: null,
  profileId: null,
  walletMode: null,
})
