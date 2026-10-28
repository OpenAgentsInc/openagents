// Hidden presentations: a deck sealed for one recipient key and published as
// a Nostr gift wrap (NIP-59). Inside the wrap is a seal from the author, and
// inside that an OpenAgents private artifact (kind 3188, NIP-44 v2 content,
// nips/openagents/contracts.md) whose envelope carries the deck inline.
// The same code runs in the sealer (bun) and in the page (bundled).
//
// The seal and wrap are built here from nostr-effect's NIP-44 v2 and event
// signing, not its nip59 module: that module's NIP-44 leaves the nonce out of
// the MAC, so its wraps don't open with spec NIP-44 or a NIP-07 signer.
import { getConversationKey, encrypt as nip44Encrypt, decrypt as nip44Decrypt } from "nostr-effect/nip44"
import { finalizeEvent, generateSecretKey, getEventHash, getPublicKey, verifyEvent, type Event, type EventTemplate } from "nostr-effect/pure"
import { sha256 } from "@noble/hashes/sha256"
import { bytesToHex, randomBytes } from "@noble/hashes/utils"

export const ARTIFACT_KIND = 3188
export const SEAL_KIND = 13
export const WRAP_KIND = 1059
export const ENVELOPE = "openagents.artifact-envelope.v1"
export const DECK_SCHEMA = "coaxial.presentation.v1"
const MAX_BODY = 1_048_576

export type Slide = { title: string; subtitle?: string; bullets?: string[]; body?: string; notes?: string }
export type Deck = { v: typeof DECK_SCHEMA; title: string; slides: Slide[] }

/** Whoever can open things on this device: a local key or a NIP-07 signer. */
export interface Opener {
  pubkey: string
  decrypt(peer: string, ciphertext: string): Promise<string>
  sign(template: EventTemplate): Promise<Event>
}

export function localOpener(secret: Uint8Array): Opener {
  const pubkey = getPublicKey(secret)
  return {
    pubkey,
    decrypt: async (peer, ciphertext) => nip44Decrypt(ciphertext, getConversationKey(secret, peer)),
    sign: async (template) => finalizeEvent(template, secret),
  }
}

/** RFC 8785 JCS for the JSON a deck holds (strings, integers, arrays, objects). */
export function jcs(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value)
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new Error("only integers are allowed")
    return String(value)
  }
  if (Array.isArray(value)) return `[${value.map(jcs).join(",")}]`
  if (typeof value === "object") {
    const keys = Object.keys(value as object).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
    return `{${keys.map((k) => `${JSON.stringify(k)}:${jcs((value as Record<string, unknown>)[k])}`).join(",")}}`
  }
  throw new Error(`not JSON: ${typeof value}`)
}

export function checkDeck(deck: unknown): Deck {
  const d = deck as Deck
  const text = (s: unknown, max: number) => typeof s === "string" && s.length <= max
  if (!d || d.v !== DECK_SCHEMA || !text(d.title, 200) || !Array.isArray(d.slides) || d.slides.length < 1 || d.slides.length > 60)
    throw new Error("not a presentation")
  for (const s of d.slides) {
    if (!text(s.title, 200) || !s.title.trim()) throw new Error("a slide needs a title")
    for (const k of ["subtitle", "body", "notes"] as const) if (s[k] !== undefined && !text(s[k], 4000)) throw new Error(`bad ${k}`)
    if (s.bullets !== undefined && (!Array.isArray(s.bullets) || s.bullets.length > 20 || !s.bullets.every((b) => text(b, 600))))
      throw new Error("bad bullets")
  }
  return d
}

/** A deck as the lines the page's Coax code reads (ui.coax): "D" and the
 * title, then per slide "=", "T" title, "S" subtitle, "P" body lines, "B"
 * bullets and "N" notes. Text keeps its "**" bold marks. */
export function deckLines(deck: Deck): string {
  const one = (s: string) => s.replace(/\s*[\r\n]+\s*/g, " ").trim()
  const lines = (s: string | undefined) => (s ?? "").split(/\r?\n/).map((l) => l.trim()).filter(Boolean)
  const out = ["D" + one(deck.title)]
  for (const s of deck.slides) {
    out.push("=", "T" + one(s.title))
    if (s.subtitle) out.push("S" + one(s.subtitle))
    for (const p of lines(s.body)) out.push("P" + p)
    for (const b of s.bullets ?? []) out.push("B" + one(b))
    for (const n of lines(s.notes)) out.push("N" + n)
  }
  return out.join("\n") + "\n"
}

/** Reads the deck format: slides split by a line of ---, each "# title",
 * an optional "## subtitle", "- " bullets, "> " speaker notes, other lines body. */
export function parseDeck(markdown: string): Deck {
  const blocks = markdown.split(/^---\s*$/m).map((b) => b.trim()).filter(Boolean)
  const slides: Slide[] = blocks.map((block) => {
    const s: Slide = { title: "" }
    const body: string[] = [], notes: string[] = [], bullets: string[] = []
    for (const raw of block.split("\n")) {
      const line = raw.trimEnd()
      if (line.startsWith("# ")) s.title = line.slice(2).trim()
      else if (line.startsWith("## ")) s.subtitle = line.slice(3).trim()
      else if (/^\s*[-*] /.test(line)) bullets.push(line.replace(/^\s*[-*] /, "").trim())
      else if (line.startsWith(">")) notes.push(line.replace(/^>\s?/, ""))
      else if (line.trim()) body.push(line.trim())
    }
    if (bullets.length) s.bullets = bullets
    if (body.length) s.body = body.join("\n")
    if (notes.length) s.notes = notes.join("\n")
    return s
  })
  return checkDeck({ v: DECK_SCHEMA, title: slides[0]?.title ?? "", slides })
}

const digestOf = (bytes: Uint8Array) => "sha256:" + bytesToHex(sha256(bytes))

/** Seals a deck for one recipient: the 3188 artifact as a rumor, sealed by
 * the author and gift-wrapped from a throwaway key. Returns the wrap to publish. */
export function sealDeck(deck: Deck, author: Uint8Array, recipient: string, now = Math.floor(Date.now() / 1000)): Event {
  checkDeck(deck)
  const bytes = new TextEncoder().encode(jcs(deck))
  const envelope = {
    v: ENVELOPE,
    requires: [],
    artifact: { digest: digestOf(bytes), size: bytes.length, media_type: "application/json", schema: DECK_SCHEMA },
    inline: deck,
    issued_at: now,
    retain_until: now + 365 * 86400,
  }
  const content = nip44Encrypt(JSON.stringify(envelope), getConversationKey(author, recipient))
  const pubkey = getPublicKey(author)
  const unsigned = { kind: ARTIFACT_KIND, pubkey, created_at: now, content,
    tags: [["p", recipient], ["h", bytesToHex(randomBytes(32))], ["t", "oa:artifact:v1"]] }
  const rumor = { ...unsigned, id: getEventHash(unsigned) }   // unsigned: the seal authenticates it
  // NIP-59: seal and wrap times are randomized up to two days back
  const jitter = () => now - Math.floor(Math.random() * 2 * 86400)
  const seal = finalizeEvent({ kind: SEAL_KIND, created_at: jitter(), tags: [],
    content: nip44Encrypt(JSON.stringify(rumor), getConversationKey(author, recipient)) }, author)
  const once = generateSecretKey()
  return finalizeEvent({ kind: WRAP_KIND, created_at: jitter(), tags: [["p", recipient]],
    content: nip44Encrypt(JSON.stringify(seal), getConversationKey(once, recipient)) }, once)
}

/** Opens a gift wrap with this device's opener: verifies the wrap and seal
 * signatures, that the seal's author wrote the artifact, that it is addressed
 * to us, and that the inline deck matches its digest and size. */
export async function openDeck(wrap: Event, opener: Opener): Promise<{ deck: Deck; author: string }> {
  if (wrap.kind !== WRAP_KIND || !verifyEvent(wrap)) throw new Error("not a valid gift wrap")
  if (!wrap.tags.some((t) => t[0] === "p" && t[1] === opener.pubkey)) throw new Error("not addressed to this key")
  const seal = JSON.parse(await opener.decrypt(wrap.pubkey, wrap.content)) as Event
  if (seal.kind !== SEAL_KIND || !verifyEvent(seal)) throw new Error("not a valid seal")
  const rumor = JSON.parse(await opener.decrypt(seal.pubkey, seal.content)) as Event
  if (rumor.pubkey !== seal.pubkey) throw new Error("the seal and the artifact disagree on the author")
  if (getEventHash(rumor) !== rumor.id) throw new Error("the artifact's id doesn't match it")
  if (rumor.kind !== ARTIFACT_KIND) throw new Error("not a private artifact")
  const tag = (name: string) => rumor.tags.filter((t) => t[0] === name)
  if (tag("p").length !== 1 || tag("p")[0][1] !== opener.pubkey || tag("h").length !== 1 || !/^[0-9a-f]{64}$/.test(tag("h")[0][1]) ||
      !tag("t").some((t) => t[1] === "oa:artifact:v1"))
    throw new Error("not a private artifact for this key")
  const body = await opener.decrypt(rumor.pubkey, rumor.content)
  if (body.length > MAX_BODY) throw new Error("too large")
  const envelope = JSON.parse(body)
  if (envelope.v !== ENVELOPE || !Array.isArray(envelope.requires) || envelope.requires.length) throw new Error("unsupported envelope")
  const bytes = new TextEncoder().encode(jcs(envelope.inline))
  const ref = envelope.artifact
  if (!ref || ref.schema !== DECK_SCHEMA || ref.size !== bytes.length || ref.digest !== digestOf(bytes))
    throw new Error("the deck doesn't match its digest")
  return { deck: checkDeck(envelope.inline), author: rumor.pubkey }
}

/** Fetches one event by id from a relay, answering NIP-42 AUTH as the opener
 * (gift wraps are only served to their recipient). */
export function fetchEvent(relay: string, id: string, opener: Opener | null, timeoutMs = 10000): Promise<Event | null> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relay)
    let found: Event | null = null, authed = false, accepted = false, done = false
    const finish = (e: Event | null, err?: Error) => {
      if (done) return
      done = true; clearTimeout(timer); try { ws.close() } catch {}
      err ? reject(err) : resolve(e)
    }
    const timer = setTimeout(() => finish(found), timeoutMs)
    let asked = 0   // a fresh subscription id per ask: the answer after AUTH is a new query
    const ask = () => ws.send(JSON.stringify(["REQ", `unlock-${++asked}`, { ids: [id] }]))
    // With a key, wait for the relay's AUTH challenge and ask only once
    // authenticated (this relay answers a connection's pre-AUTH lookup of a
    // gift wrap with nothing, and keeps answering it that way); without
    // one, or if no challenge comes, ask anonymously.
    ws.onopen = () => (opener ? setTimeout(() => { if (!authed) ask() }, 1500) : ask())
    ws.onerror = () => finish(null, new Error("relay unreachable"))
    ws.onmessage = async (m) => {
      const msg = JSON.parse(String(m.data))
      if (msg[0] === "AUTH" && opener && !authed) {
        authed = true
        const auth = await opener.sign({ kind: 22242, created_at: Math.floor(Date.now() / 1000), content: "",
          tags: [["relay", relay], ["challenge", msg[1]]] })
        ws.send(JSON.stringify(["AUTH", auth]))
      } else if (msg[0] === "OK" && authed && !accepted) {
        if (!msg[2]) return finish(null, new Error(`relay refused this key: ${msg[3]}`))
        accepted = true; ask()   // ask again, now as the recipient
      } else if (msg[0] === "EVENT" && msg[2]?.id === id) { found = msg[2]; finish(found) }
      else if (msg[0] === "CLOSED" && /auth-required/.test(msg[2] ?? "")) { if (!opener) finish(null) }
      // an answer before AUTH completes is the anonymous one; wait for ours
      else if (msg[0] === "EOSE" && (accepted || !opener || !authed)) finish(found)
    }
  })
}

/** Publishes an event, answering AUTH with the given key if the relay asks. */
export function publish(relay: string, event: Event, authKey: Uint8Array, timeoutMs = 10000): Promise<string> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relay)
    let tries = 0
    const timer = setTimeout(() => { ws.close(); reject(new Error("relay timed out")) }, timeoutMs)
    const send = () => { tries++; ws.send(JSON.stringify(["EVENT", event])) }
    ws.onopen = send
    ws.onerror = () => { clearTimeout(timer); reject(new Error("relay unreachable")) }
    ws.onmessage = (m) => {
      const msg = JSON.parse(String(m.data))
      if (msg[0] === "AUTH") {
        const auth = finalizeEvent({ kind: 22242, created_at: Math.floor(Date.now() / 1000), content: "",
          tags: [["relay", relay], ["challenge", msg[1]]] }, authKey)
        ws.send(JSON.stringify(["AUTH", auth]))
      } else if (msg[0] === "OK" && msg[1] === event.id) {
        if (msg[2]) { clearTimeout(timer); ws.close(); resolve(event.id) }
        else if (/auth-required/.test(msg[3]) && tries < 3) setTimeout(send, 300)
        else { clearTimeout(timer); ws.close(); reject(new Error(`relay refused: ${msg[3]}`)) }
      } else if (msg[0] === "OK" && msg[2] && tries < 3 && msg[1] !== event.id) setTimeout(send, 50)
    }
  })
}
