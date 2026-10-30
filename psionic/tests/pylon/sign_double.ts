// TEST-ONLY signing double for the chapter 19 Pylon
// (psionic/lib/pylon/run.coax), until openagents-d7's
// coax/lib/nostr/event_sign.coax (nostr_event_sign_step) lands on main.
// NIP-44's conversation key is Coax now (coax/lib/nostr/nip44-key.coax). Tooling only: never shipped,
// never given a real key (run.coax refuses --key-file while this double is
// the signer). The oracle is @noble/curves (BIP-340 Schnorr), independent
// of Coax. One JSON request per stdin line, one JSON
// answer per stdout line:
//   {"op":"pub","key":K}                      -> {"pub":X}
//   {"op":"sign","key":K,"aux":A,"created_at":T,"kind":N,"tags":[...],"content":S}
//                                             -> the signed event
// A refusal is {"error":"..."}. NODE_PATH names a node_modules with @noble.
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";

const hex64 = (s: unknown) => typeof s === "string" && /^[0-9a-f]{64}$/.test(s);
const answer = (t: any): unknown => {
  if (!hex64(t.key)) return { error: "key" };
  const secret = hexToBytes(t.key);
  const pubkey = bytesToHex(schnorr.getPublicKey(secret));
  if (t.op === "pub") return { pub: pubkey };
  if (t.op === "sign") {
    if (!hex64(t.aux)) return { error: "aux" };
    const ser = JSON.stringify([0, pubkey, t.created_at, t.kind, t.tags, t.content]);
    const id = sha256(new TextEncoder().encode(ser));
    const sig = bytesToHex(schnorr.sign(id, secret, hexToBytes(t.aux)));
    return { id: bytesToHex(id), pubkey, created_at: t.created_at, kind: t.kind, tags: t.tags, content: t.content, sig };
  }
  return { error: "op" };
};
const decoder = new TextDecoder();
let pending = "";
for await (const chunk of Bun.stdin.stream()) {
  pending += decoder.decode(chunk, { stream: true });
  let at: number;
  while ((at = pending.indexOf("\n")) >= 0) {
    const line = pending.slice(0, at);
    pending = pending.slice(at + 1);
    if (!line.trim()) continue;
    let out: unknown;
    try { out = answer(JSON.parse(line)); } catch { out = { error: "json" }; }
    process.stdout.write(JSON.stringify(out) + "\n");
  }
}
