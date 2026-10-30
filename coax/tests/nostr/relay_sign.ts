// Signs event templates for the relay's scripted sessions with a TEST-ONLY
// key (sha256 of a fixed label: no one's key, never used anywhere else).
// The oracle is @noble/curves (BIP-340 schnorr), independent of Coax.
// stdin: one JSON template per line ({kind, created_at, tags, content});
// stdout: the signed events, one per line. Tooling only.
//   bun coax/tests/nostr/relay_sign.ts < templates.jsonl
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";

const label = process.env.RELAY_TEST_KEY_LABEL ?? "coax relay r1 test key";
const secret = sha256(new TextEncoder().encode(label));
const pubkey = bytesToHex(schnorr.getPublicKey(secret));
const input = await new Response(Bun.stdin.stream()).text();
for (const line of input.split("\n")) {
  if (!line.trim()) continue;
  const t = JSON.parse(line);
  const ser = JSON.stringify([0, pubkey, t.created_at, t.kind, t.tags, t.content]);
  const id = sha256(new TextEncoder().encode(ser));
  const sig = bytesToHex(schnorr.sign(id, secret, new Uint8Array(32)));
  console.log(JSON.stringify({ id: bytesToHex(id), pubkey, created_at: t.created_at, kind: t.kind, tags: t.tags, content: t.content, sig }));
}
