// Seals a presentation for one recipient key and publishes it as a NIP-59
// gift wrap, printing the pointer COAXIAL_UNLOCKS takes (RELAY,EVENT_ID,RECIPIENT).
//   bun seal.ts keygen FILE                      # a new key in FILE (hex, mode 600); prints its public key
//   bun seal.ts DECK.md --author FILE --to PUBKEY [--relay URL]
// The deck format is in lib.ts (parseDeck): slides split by ---, "# title",
// "## subtitle", "- " bullets, "> " notes.
import { existsSync, readFileSync, writeFileSync } from "node:fs"
import { generateSecretKey, getPublicKey } from "nostr-effect/pure"
import { parseDeck, publish, sealDeck } from "./lib.ts"

const hex = (b: Uint8Array) => Buffer.from(b).toString("hex")
const readKey = (file: string) => {
  const t = readFileSync(file, "utf8").trim()
  if (!/^[0-9a-f]{64}$/.test(t)) throw new Error(`${file} doesn't hold a hex key`)
  return Uint8Array.from(Buffer.from(t, "hex"))
}

const [cmd, ...rest] = process.argv.slice(2)
const flag = (name: string, fallback?: string) => {
  const i = rest.indexOf(`--${name}`)
  const v = i >= 0 ? rest[i + 1] : fallback
  if (v === undefined) throw new Error(`--${name} is required`)
  return v
}

if (cmd === "keygen") {
  const file = rest[0]
  if (!file) throw new Error("usage: bun seal.ts keygen FILE")
  if (existsSync(file)) throw new Error(`${file} exists; not overwriting a key`)
  const key = generateSecretKey()
  writeFileSync(file, hex(key) + "\n", { mode: 0o600 })
  console.log(getPublicKey(key))
} else if (cmd) {
  const deck = parseDeck(readFileSync(cmd, "utf8"))
  const author = readKey(flag("author"))
  const to = flag("to"), relay = flag("relay", "wss://relay.openagents.com")
  if (!/^[0-9a-f]{64}$/.test(to)) throw new Error("--to is a 64-hex public key")
  const wrap = sealDeck(deck, author, to)
  await publish(relay, wrap, author)
  console.error(`sealed "${deck.title}" (${deck.slides.length} slides) for ${to.slice(0, 8)}…`)
  console.log(`${relay},${wrap.id},${to}`)
} else {
  console.error("usage: bun seal.ts keygen FILE | bun seal.ts DECK.md --author FILE --to PUBKEY [--relay URL]")
  process.exit(2)
}
