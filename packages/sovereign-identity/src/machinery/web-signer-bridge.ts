/**
 * IDR-08 — the web signer bridge / NIP-46 seam.
 *
 * Version-one web use may NEVER hold the shared root. The audit is explicit: the
 * web application must not store the mnemonic in local storage or plain
 * IndexedDB; it uses a local signer bridge or a NIP-46 remote signer. Only PUBLIC
 * identifiers and signer OPERATIONS cross to the browser — never the mnemonic,
 * `nsec`, raw private key, or seed.
 *
 * This module gives that boundary a typed shape:
 *
 * - `WebPublicIdentity` — the ONLY identity data a browser receives. It is public
 *   by construction (npub + pubkey + profile id). The Effect Schema below has no
 *   secret field, so a decoder cannot admit one.
 * - `WebSignerBridge` — the ONLY operations the browser can invoke. They are the
 *   narrow signer operations (get public key, sign an admitted event, NIP-44
 *   encrypt/decrypt). There is NO method that returns key material; the key stays
 *   on the trusted side (the local host process, or the NIP-46 remote signer).
 * - `assertWebBridgePayloadPublicSafe` — a runtime guard that rejects any payload
 *   shaped like raw key material before it crosses to the browser. The static
 *   test proves the seam names no raw-key field.
 * - `Nip46RemoteSignerConfig` — the declared NIP-46 remote-signer transport
 *   descriptor. A full remote-signer runtime is a later packet; this typed seam
 *   already forbids a raw-key transfer by shape.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Schema as S } from "effect";
import type { LocalSignerPort } from "./local-signer.ts";

/**
 * The PUBLIC identity a browser is allowed to receive. Public identifiers only.
 * The schema has NO mnemonic/`nsec`/private-key/seed field, so a decoder can
 * never admit one and the browser can never learn a secret through this shape.
 */
export const WebPublicIdentity = S.Struct({
  /** The ONE canonical identity reference (the `npub`). */
  identityRef: S.String.check(S.isPattern(/^npub1[a-z0-9]+$/)),
  /** The Nostr NIP-19 `npub`. */
  npub: S.String.check(S.isPattern(/^npub1[a-z0-9]+$/)),
  /** The Nostr x-only public key (hex). */
  pubkey: S.String.check(S.isPattern(/^[0-9a-f]{64}$/)),
  /** The frozen derivation profile id. */
  profileId: S.String.check(S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]*$/)),
});
export type WebPublicIdentity = typeof WebPublicIdentity.Type;

/**
 * The ONLY operations a browser may invoke through the bridge. These are exactly
 * the narrow signer operations — no method returns key material. The private key
 * stays on the trusted signer side (the local host, or the NIP-46 remote signer)
 * and never crosses to the browser.
 */
export interface WebSignerBridge {
  /** The public identity the browser is allowed to render. */
  readonly identity: WebPublicIdentity;
  /** Return the public key. */
  readonly getPublicKey: () => Promise<string>;
  /** Sign an admitted event template. The browser never sees the key. */
  readonly signEvent: LocalSignerPort["signEvent"];
  /** NIP-44 encrypt to a recipient. */
  readonly nip44Encrypt: LocalSignerPort["nip44Encrypt"];
  /** NIP-44 decrypt from a sender. */
  readonly nip44Decrypt: LocalSignerPort["nip44Decrypt"];
}

/**
 * The raw-key-shaped field names a web payload must NEVER carry. The guard below
 * rejects any object that names one, so a coding mistake that tried to hand the
 * browser a secret fails closed at the boundary instead of leaking.
 */
export const FORBIDDEN_WEB_SECRET_FIELDS: ReadonlyArray<string> = [
  "mnemonic",
  "nsec",
  "privateKey",
  "privateKeyHex",
  "privateKeyBytes",
  "seed",
  "seedHex",
  "secret",
  "secretKey",
  "priv",
] as const;

/** Thrown when a payload bound for the browser carries a raw-key-shaped field. */
export class WebBridgeRawKeyRefusedError extends Error {
  readonly code = "web_bridge_raw_key_refused" as const;
  readonly field: string;
  constructor(field: string) {
    super(`refusing to send raw-key-shaped field "${field}" to the web surface`);
    this.name = "WebBridgeRawKeyRefusedError";
    this.field = field;
  }
}

/**
 * Assert a payload is safe to cross to the browser: it must be a plain object
 * that carries NO raw-key-shaped field, at any depth. This is the structural
 * forbiddance the web boundary requires — the browser receives public data and
 * signer operations only.
 */
export function assertWebBridgePayloadPublicSafe(payload: unknown): void {
  const seen = new Set<object>();
  const walk = (value: unknown): void => {
    if (value === null || typeof value !== "object") return;
    if (seen.has(value as object)) return;
    seen.add(value as object);
    if (Array.isArray(value)) {
      for (const entry of value) walk(entry);
      return;
    }
    for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
      if (FORBIDDEN_WEB_SECRET_FIELDS.includes(key)) {
        throw new WebBridgeRawKeyRefusedError(key);
      }
      walk(entry);
    }
  };
  walk(payload);
}

/**
 * Build a `WebSignerBridge` from a resolved public identity and a narrow signer.
 * Structurally, the bridge can ONLY forward the signer's public operations:
 * there is no path from here to the mnemonic, `nsec`, raw key, or seed, because
 * the `LocalSignerPort` itself exposes none. The returned public identity is
 * guarded before it is captured, so a caller cannot slip a secret field through.
 */
export function webSignerBridgeFromSigner(
  identity: WebPublicIdentity,
  signer: LocalSignerPort,
): WebSignerBridge {
  assertWebBridgePayloadPublicSafe(identity);
  return {
    identity,
    getPublicKey: () => signer.getPublicKey(),
    signEvent: (event) => signer.signEvent(event),
    nip44Encrypt: (recipientPubkey, plaintext) => signer.nip44Encrypt(recipientPubkey, plaintext),
    nip44Decrypt: (senderPubkey, ciphertext) => signer.nip44Decrypt(senderPubkey, ciphertext),
  };
}

/**
 * The NIP-46 remote-signer transport descriptor. It names the public routing
 * data only — the relay, the remote signer public key, and an optional bounded
 * secret token used for the NIP-46 connect handshake (NOT the identity key). A
 * full NIP-46 runtime is a later packet; this typed seam already carries no
 * identity secret and forbids a raw-key transfer by shape.
 */
export const Nip46RemoteSignerConfig = S.Struct({
  transport: S.Literal("nip46"),
  /** The relay URL the remote signer listens on. */
  relay: S.String.check(S.isPattern(/^wss?:\/\/.+/)),
  /** The remote signer's public key (hex). Public routing data. */
  remoteSignerPubkey: S.String.check(S.isPattern(/^[0-9a-f]{64}$/)),
  /** The public identity the remote signer represents. Public data only. */
  identity: WebPublicIdentity,
});
export type Nip46RemoteSignerConfig = typeof Nip46RemoteSignerConfig.Type;
