/**
 * Minimal BIP-340 / NIP-01 helpers for the Sarah sealed signer.
 * Kept local so @openagentsinc/sarah does not need a full nostr-effect pin
 * for the identity boundary. Algorithms match nostr-effect/pure.
 */
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { bech32 } from "@scure/base";

import type { SarahNostrEventTemplate, SarahNostrSignedEvent } from "./types.ts";

const utf8 = new TextEncoder();

export const generateSecretKeyBytes = (): Uint8Array => schnorr.utils.randomPrivateKey();

export const publicKeyFromSecret = (secretKey: Uint8Array): string =>
  bytesToHex(schnorr.getPublicKey(secretKey));

/** Accept 64-hex or nsec1… and return 32 secret bytes. */
export const parseSecretMaterial = (raw: string): Uint8Array => {
  const trimmed = raw.trim();
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    return hexToBytes(trimmed.toLowerCase());
  }
  if (trimmed.startsWith("nsec1")) {
    const decoded = bech32.decode(trimmed as `${string}1${string}`, false);
    if (decoded.prefix !== "nsec") {
      throw new Error("sarah_nostr_identity: expected nsec prefix");
    }
    const bytes = new Uint8Array(bech32.fromWords(decoded.words));
    if (bytes.length !== 32) {
      throw new Error("sarah_nostr_identity: nsec payload must be 32 bytes");
    }
    return bytes;
  }
  throw new Error("sarah_nostr_identity: secret must be 64-hex or nsec1…");
};

export const serializeUnsigned = (
  pubkey: string,
  template: SarahNostrEventTemplate,
): string =>
  JSON.stringify([
    0,
    pubkey,
    template.created_at,
    template.kind,
    template.tags.map((t) => [...t]),
    template.content,
  ]);

export const eventIdOf = (pubkey: string, template: SarahNostrEventTemplate): string =>
  bytesToHex(sha256(utf8.encode(serializeUnsigned(pubkey, template))));

export const signEventTemplate = (
  secretKey: Uint8Array,
  template: SarahNostrEventTemplate,
): SarahNostrSignedEvent => {
  const pubkey = publicKeyFromSecret(secretKey);
  const id = eventIdOf(pubkey, template);
  const sig = bytesToHex(schnorr.sign(id, secretKey));
  return {
    id,
    pubkey,
    created_at: template.created_at,
    kind: template.kind,
    tags: template.tags.map((t) => [...t]),
    content: template.content,
    sig,
  };
};

export const verifySignedEvent = (event: SarahNostrSignedEvent): boolean => {
  const id = eventIdOf(event.pubkey, {
    kind: event.kind,
    created_at: event.created_at,
    tags: event.tags,
    content: event.content,
  });
  if (id !== event.id) return false;
  try {
    return schnorr.verify(event.sig, id, event.pubkey);
  } catch {
    return false;
  }
};

/** NIP-OA domain separator. */
export const AGENT_AUTH_DOMAIN = "nostr:agent-auth:";

export const ownerAttestationPreimage = (
  agentPubkey: string,
  conditions: string,
): Uint8Array => utf8.encode(`${AGENT_AUTH_DOMAIN}${agentPubkey}:${conditions}`);

export const signOwnerAuthTag = (params: {
  readonly agentPubkey: string;
  readonly conditions: string;
  readonly ownerSeckeyHex: string;
}): readonly ["auth", string, string, string] => {
  const ownerSk = hexToBytes(params.ownerSeckeyHex.toLowerCase());
  const ownerPk = bytesToHex(schnorr.getPublicKey(ownerSk));
  const msg = sha256(ownerAttestationPreimage(params.agentPubkey, params.conditions));
  const sig = bytesToHex(schnorr.sign(msg, ownerSk));
  return ["auth", ownerPk, params.conditions, sig];
};

export const verifyOwnerAuthTag = (
  tag: readonly string[],
  agentPubkey: string,
): boolean => {
  if (tag.length < 4 || tag[0] !== "auth") return false;
  const ownerPk = tag[1] ?? "";
  const conditions = tag[2] ?? "";
  const sig = tag[3] ?? "";
  if (!/^[0-9a-f]{64}$/.test(ownerPk) || !/^[0-9a-f]{128}$/.test(sig)) return false;
  if (ownerPk === agentPubkey) return false; // self-attestation forbidden
  const msg = sha256(ownerAttestationPreimage(agentPubkey, conditions));
  try {
    return schnorr.verify(sig, msg, ownerPk);
  } catch {
    return false;
  }
};

/** NIP-42 AUTH event kind. */
export const NIP42_AUTH_KIND = 22242;

export const buildAttestedAuthTemplate = (params: {
  readonly challenge: string;
  readonly relayUrl: string;
  readonly ownerAuthTag: readonly string[];
  readonly createdAt?: number;
}): SarahNostrEventTemplate => {
  const tags: string[][] = [
    ["relay", params.relayUrl],
    ["challenge", params.challenge],
  ];
  // Exactly one auth tag
  const auth = params.ownerAuthTag;
  tags.push([
    "auth",
    auth[1] ?? "",
    auth[2] ?? "",
    auth[3] ?? "",
  ]);
  return {
    kind: NIP42_AUTH_KIND,
    created_at: params.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content: "",
  };
};

/** NIP-IA archive request kind. */
export const NIP_IA_ARCHIVE_REQUEST_KIND = 9035;

export const buildArchiveRequestTemplate = (params: {
  readonly targetPubkey: string;
  readonly reason: "rotated" | "retired" | "bot-rebuilt" | "left-organization" | "spam";
  readonly createdAt?: number;
}): SarahNostrEventTemplate => ({
  kind: NIP_IA_ARCHIVE_REQUEST_KIND,
  created_at: params.createdAt ?? Math.floor(Date.now() / 1000),
  tags: [
    ["-", ""],
    ["p", params.targetPubkey],
    ["reason", params.reason],
  ],
  content: "",
});
