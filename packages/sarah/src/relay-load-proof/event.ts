/**
 * Minimal NIP-01 event create for the load harness.
 * Uses the same @noble stack as packages/sarah identity crypto.
 */
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { randomBytes } from "node:crypto";

export type LoadProofNostrEvent = {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
  readonly sig: string;
};

export const generatePrivateKeyHex = (): string =>
  bytesToHex(randomBytes(32));

export const getPublicKeyHex = (privateKeyHex: string): string =>
  bytesToHex(schnorr.getPublicKey(hexToBytes(privateKeyHex)));

export const createSignedEvent = (input: {
  privateKeyHex: string;
  kind?: number;
  content?: string;
  tags?: ReadonlyArray<ReadonlyArray<string>>;
  createdAt?: number;
}): LoadProofNostrEvent => {
  const privateKey = hexToBytes(input.privateKeyHex);
  const pubkey = bytesToHex(schnorr.getPublicKey(privateKey));
  const created_at = input.createdAt ?? Math.floor(Date.now() / 1000);
  const kind = input.kind ?? 1;
  const tags = (input.tags ?? []).map((tag) => [...tag]);
  const content = input.content ?? `load-proof ${created_at}`;
  const serialized = JSON.stringify([
    0,
    pubkey,
    created_at,
    kind,
    tags,
    content,
  ]);
  const id = bytesToHex(sha256(new TextEncoder().encode(serialized)));
  const sig = bytesToHex(schnorr.sign(hexToBytes(id), privateKey));
  return { id, pubkey, created_at, kind, tags, content, sig };
};
