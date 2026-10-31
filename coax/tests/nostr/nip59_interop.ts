// NIP-59 interoperability between coax/lib/nostr/nip59.coax and nostr-effect,
// both directions, including the hidden test deck (coax/web/unlock/lib.ts).
// Firm reason: 3 (runs the reference implementation we test against).
// Test tooling only; test keys only.
//
//   bun coax/tests/nostr/nip59_interop.ts [COAX]   (COAX defaults to coax/bin/coax)
//
// Coax to nostr-effect: nip59_dump.coax prints three wraps; each must pass
// nostr-effect's verifyEvent on the wrap and the seal, decrypt with spec
// NIP-44 v2 for Bob, carry an unsigned rumor whose id is its own and whose
// pubkey is the seal's, and the deck's must open with the sealer's openDeck.
// nostr-effect to Coax: fresh wraps made now (new random keys and nonces) are
// opened by a generated Coax program, which prints what it read.
import { getConversationKey, decrypt } from "../../web/project/node_modules/nostr-effect/src/wrappers/nip44.ts"
import { getEventHash, verifyEvent } from "../../web/project/node_modules/nostr-effect/src/wrappers/pure.ts"
import { localOpener, openDeck, parseDeck, sealDeck } from "../../web/unlock/lib.ts"
import { deepStrictEqual, ok, strictEqual } from "node:assert"
import { mkdtempSync, writeFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { ALICE, ALICE_PUB, BOB, BOB_PUB, DECK_MD, RUMOR_AT } from "./nip59_fixtures.ts"

const root = new URL("../../../", import.meta.url).pathname
const coax = process.argv[2] ?? `${root}coax/bin/coax`
const run = (file: string) => {
  const r = Bun.spawnSync([coax, "run", file, "--run-steps", "250000000"], { stdout: "pipe", stderr: "pipe" })
  if (r.exitCode !== 0) throw new Error(`coax run ${file} failed:\n${r.stderr.toString()}`)
  return r.stdout.toString().trimEnd().split("\n")
}

// ---- Coax wraps, opened by nostr-effect
const NOW = 1793300000
const lines = run(`${root}coax/tests/nostr/nip59_dump.coax`)
strictEqual(lines.length, 3)
const wraps = lines.map((l) => JSON.parse(l))
const wrapKeys = new Set<string>()
for (const [i, wrap] of wraps.entries()) {
  ok(verifyEvent(wrap), `wrap ${i} verifies`)
  strictEqual(wrap.kind, i === 1 ? 21059 : 1059)
  deepStrictEqual(wrap.tags, [["p", BOB_PUB]])
  ok(wrap.pubkey !== ALICE_PUB && !wrapKeys.has(wrap.pubkey), `wrap ${i} has a fresh key`)
  wrapKeys.add(wrap.pubkey)
  ok(wrap.created_at <= NOW && wrap.created_at > NOW - 2 * 86400, `wrap ${i} time is within two days back`)
  const seal = JSON.parse(decrypt(wrap.content, getConversationKey(BOB, wrap.pubkey)))
  ok(verifyEvent(seal), `seal ${i} verifies`)
  strictEqual(seal.kind, 13); deepStrictEqual(seal.tags, []); strictEqual(seal.pubkey, ALICE_PUB)
  ok(seal.created_at <= NOW && seal.created_at > NOW - 2 * 86400)
  const rumor = JSON.parse(decrypt(seal.content, getConversationKey(BOB, seal.pubkey)))
  strictEqual(rumor.sig, undefined); strictEqual(rumor.pubkey, seal.pubkey); strictEqual(getEventHash(rumor), rumor.id)
  strictEqual(rumor.created_at, NOW)
}
strictEqual(JSON.parse(decrypt(JSON.parse(decrypt(wraps[0].content, getConversationKey(BOB, wraps[0].pubkey))).content,
  getConversationKey(BOB, ALICE_PUB))).content, "Bob: the decision job is ready. \"quoted\", tab\t and é.")
// The deck wrapped again by Coax opens with the sealer's own opener.
deepStrictEqual(await openDeck(wraps[2], localOpener(BOB)), { deck: parseDeck(DECK_MD), author: ALICE_PUB })
console.log("coax -> nostr-effect: 3 wraps open (chat, ephemeral, deck)")

// ---- fresh nostr-effect wraps, opened by Coax
const deck = sealDeck(parseDeck(DECK_MD), ALICE, BOB_PUB, RUMOR_AT)
const dir = mkdtempSync(`${tmpdir()}/nip59-interop-`)
try {
  const file = `${dir}/open.coax`
  writeFileSync(file, `#import "Basic";
#load "${root}coax/lib/nostr/nip59.coax";
W :: ${JSON.stringify(JSON.stringify(deck))};
main :: () {
  bob:=n59_bob();me:=N59_BOB_PUB;work:[NIP59_WORK]u8;out:[NIP59_REGION]u8;
  o:=nip59_unwrap_checked(bob[..],me[..],W,work[..],out[..],false);
  if o.error!=NIP59_OK {print("error %\\n",o.error);return;}
  print("%\\n",cast(string)out[0..<o.written]);
}
`)
  const [opened] = run(file)
  ok(!opened.startsWith("error"), `Coax opens a fresh deck wrap: ${opened}`)
  const rumor = JSON.parse(opened)
  const seal = JSON.parse(decrypt(deck.content, getConversationKey(BOB, deck.pubkey)))
  deepStrictEqual(rumor, JSON.parse(decrypt(seal.content, getConversationKey(BOB, seal.pubkey))))
  strictEqual(rumor.kind, 3188); strictEqual(rumor.pubkey, ALICE_PUB)
  console.log("nostr-effect -> coax: a fresh deck wrap opens to the same rumor")
} finally {
  rmSync(dir, { recursive: true, force: true })
}
