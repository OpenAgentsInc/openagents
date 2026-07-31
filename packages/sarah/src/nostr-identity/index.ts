/**
 * Sarah Nostr identity — SARAH-NR-04
 *
 * Sealed signing boundary, Secret Manager custody mount, NIP-OA/AA helpers,
 * and lifecycle transitions for principal.sarah.
 *
 * @see docs/omega/2026-07-24-sarah-nostr-identity-contract.md
 */
export {
  AGENT_AUTH_DOMAIN,
  NIP42_AUTH_KIND,
  NIP_IA_ARCHIVE_REQUEST_KIND,
  buildArchiveRequestTemplate,
  buildAttestedAuthTemplate,
  generateSecretKeyBytes,
  parseSecretMaterial,
  publicKeyFromSecret,
  signOwnerAuthTag,
  verifyOwnerAuthTag,
  verifySignedEvent,
} from "./crypto.ts";
export { makeNip44OwnerCipher, secretKeyFromHex } from "./nip44-cipher.ts";
export { decrypt as nip44Decrypt, encrypt as nip44Encrypt, getConversationKey } from "./nip44.ts";
export {
  SarahNostrSecretLeakError,
  assertSarahNostrPublicSafe,
  toPublicSafeJson,
} from "./redaction.ts";
export {
  assertLifecycleTransition,
  createSealedSarahNostrSigner,
  createSealedSarahNostrStack,
  generateSarahNostrSigner,
  loadSealedSarahNostrStackFromSecretManagerMount,
  loadSarahNostrSignerFromSecretManagerMount,
  type SarahNostrSecretSource,
} from "./signer.ts";
export {
  FORBIDDEN_SARAH_NOSTR_SECRET_FIELDS,
  SARAH_NOSTR_IDENTITY_SCHEMA,
  SARAH_NOSTR_IDENTITY_SECRET_ENV,
  SARAH_NOSTR_IDENTITY_SECRET_ID,
  SARAH_NOSTR_OWNER_AUTH_TAG_ENV,
  SARAH_NOSTR_PRINCIPAL,
  SarahNostrLifecycleState,
  SarahNostrPublicIdentity,
  type SarahNostrEventTemplate,
  type SarahNostrSignedEvent,
  type SarahNostrSigner,
  type SarahOwnerAuthTag,
} from "./types.ts";
