import { Schema as S } from "effect";

/** Wire schema id for the Sarah Nostr public identity record. */
export const SARAH_NOSTR_IDENTITY_SCHEMA = "openagents.sarah.nostr_identity.v1" as const;

/** Stable principal ref. Matches the owner-orchestrator principal. */
export const SARAH_NOSTR_PRINCIPAL = "principal.sarah" as const;

/** Cloud Run env name mounted from Secret Manager. */
export const SARAH_NOSTR_IDENTITY_SECRET_ENV = "SARAH_NOSTR_IDENTITY_SECRET" as const;

/** Secret Manager secret id in project openagentsgemini. */
export const SARAH_NOSTR_IDENTITY_SECRET_ID = "sarah-nostr-identity-secret" as const;

/** Optional public owner-attestation tag (JSON array) for NIP-AA AUTH. */
export const SARAH_NOSTR_OWNER_AUTH_TAG_ENV = "SARAH_NOSTR_OWNER_AUTH_TAG_JSON" as const;

export const SarahNostrLifecycleState = S.Literals([
  "active",
  "rotating",
  "revoked",
  "archived",
]);
export type SarahNostrLifecycleState = S.Schema.Type<typeof SarahNostrLifecycleState>;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));

export const SarahNostrPublicIdentity = S.Struct({
  schema: S.Literal(SARAH_NOSTR_IDENTITY_SCHEMA),
  principal: S.Literal(SARAH_NOSTR_PRINCIPAL),
  pubkey: Hex64,
  lifecycle: SarahNostrLifecycleState,
  /** Optional NIP-19 npub when known. Public only. */
  npub: S.optional(S.String.check(S.isPattern(/^npub1[a-z0-9]+$/))),
  /** Secret Manager secret id that backs this identity (name only, never value). */
  custodySecretId: S.Literal(SARAH_NOSTR_IDENTITY_SECRET_ID),
});
export type SarahNostrPublicIdentity = S.Schema.Type<typeof SarahNostrPublicIdentity>;

/** Unsigned NIP-01 event template (no id/sig/pubkey). */
export interface SarahNostrEventTemplate {
  readonly kind: number;
  readonly created_at: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

/** Signed NIP-01 event. */
export interface SarahNostrSignedEvent {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
  readonly sig: string;
}

/**
 * Signing boundary for principal.sarah.
 * Returns signatures and public data only — never private key material.
 */
export interface SarahNostrSigner {
  readonly getPublicKey: () => string;
  readonly getPublicIdentity: () => SarahNostrPublicIdentity;
  readonly signEvent: (template: SarahNostrEventTemplate) => SarahNostrSignedEvent;
}

/** NIP-OA auth tag as a 4-element wire array. */
export type SarahOwnerAuthTag = readonly ["auth", string, string, string];

export const FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS: ReadonlyArray<string> = [
  "mnemonic",
  "nsec",
  "privateKey",
  "privateKeyHex",
  "privateKeyBytes",
  "seckey",
  "secretKey",
  "secretKeyHex",
  "seed",
  "seedHex",
  "rawKey",
  "SARAH_NOSTR_IDENTITY_SECRET",
];
