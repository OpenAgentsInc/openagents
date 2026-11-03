// Hidden presentations: a deck sealed for one recipient key and published as
// a Nostr gift wrap (NIP-59). Inside the wrap is a seal from the author, and
// inside that an OpenAgents private artifact (kind 3188, NIP-44 v2 content,
// nostr/nips/openagents/contracts.md) whose envelope carries the deck inline.
// The same code runs in the sealer (bun) and in the page (bundled).
//
// The seal and wrap are built here from nostr-effect's NIP-44 v2 and event
// signing, not its nip59 module: that module's NIP-44 leaves the nonce out of
// the MAC, so its wraps don't open with spec NIP-44 or a NIP-07 signer.
//
// The opener's signing, verifying and NIP-44 (its public key, its NIP-42
// AUTH, the wrap's and the seal's signatures, the artifact's id, and opening
// the wrap, the seal and the artifact) go through `nostr`: nostr-effect by
// default, or Coax (unlock-nostr.coax as Wasm, coaxNostr) once useNostr
// installs it, as page.ts does when the deploy set COAXIAL_UNLOCK_SIGNER=coax.
// sealDeck makes the artifact, the seal and the wrap through `nostr` too (the
// rumor's id hash and the throwaway key stay nostr-effect's). Unset, both are
// as they were: the rollback. A NIP-07 signer decrypts with its own NIP-44, and
// publish signs its AUTH with nostr-effect.
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

/** The signing and verifying the opener does: nostr-effect's, or Coax's. */
export interface Nostr {
  name: "nostr-effect" | "coax"
  getPublicKey(secret: Uint8Array): string
  finalizeEvent(template: EventTemplate, secret: Uint8Array): Event
  /** The id is the hash of the fields and the signature verifies. */
  verifyEvent(event: Event): boolean
  /** The id is the hash of the fields (a NIP-59 rumor is unsigned). */
  idMatches(event: Omit<Event, "sig">): boolean
  /** NIP-44 v2: the conversation key of a secret and a peer's public key (hex). */
  conversationKey(secret: Uint8Array, peer: string): Uint8Array
  /** NIP-44 v2: a payload; the nonce is 32 fresh random bytes unless given (test vectors). */
  encrypt(plaintext: string, conversationKey: Uint8Array, nonce?: Uint8Array): string
  /** NIP-44 v2: the plaintext, or a throw for a payload whose MAC doesn't hold. */
  decrypt(payload: string, conversationKey: Uint8Array): string
}
export const effectNostr: Nostr = {
  name: "nostr-effect",
  getPublicKey,
  finalizeEvent,
  verifyEvent: (event) => verifyEvent(event),
  idMatches: (event) => getEventHash(event) === event.id,
  conversationKey: getConversationKey,
  encrypt: (plaintext, key, nonce) => nonce ? nip44Encrypt(plaintext, key, nonce) : nip44Encrypt(plaintext, key),
  decrypt: nip44Decrypt,
}
let nostr: Nostr = effectNostr
/** Installs the opener's Nostr (null: back to nostr-effect); returns it. */
export function useNostr(n: Nostr | null): Nostr { nostr = n ?? effectNostr; return nostr }
export const currentNostr = () => nostr

const UN_OK = 0
/** unlock-nostr.coax, compiled to Wasm (unlock-nostr-face.c), as a Nostr.
 * This side copies bytes in and out; every verdict is the Coax one. */
export async function coaxNostr(wasm: BufferSource): Promise<Nostr> {
  const module = await WebAssembly.compile(wasm)
  const memory = new WebAssembly.Memory({ initial: 128, maximum: 1024 })
  const imports: Record<string, Record<string, unknown>> = { env: { memory } }
  // wasi-libc's stdio imports: nothing here prints, and a call fails (ENOSYS)
  for (const i of WebAssembly.Module.imports(module))
    if (i.kind === "function") (imports[i.module] ??= {})[i.name] = () => 52
  const x = (await WebAssembly.instantiate(module, imports as WebAssembly.Imports)).exports as Record<string, any>
  x._initialize?.()
  const enc = new TextEncoder(), dec = new TextDecoder()
  const cap = x.un_in_cap() as number
  const put = (slot: number, text: string): number => {
    const bytes = enc.encode(text)
    if (bytes.length > cap) throw new Error("too large")
    new Uint8Array(memory.buffer, x.un_in(slot), bytes.length).set(bytes)
    return bytes.length
  }
  const withSecret = <T>(secret: Uint8Array, f: () => T): T => {
    if (secret.length !== 32) throw new Error("a key is 32 bytes")
    try { new Uint8Array(memory.buffer, x.un_secret(), 32).set(secret); return f() } finally { x.un_wipe() }
  }
  const hex = (b: Uint8Array) => [...b].map((v) => v.toString(16).padStart(2, "0")).join("")
  const withKey = <T>(key: Uint8Array, f: () => T): T => {
    if (key.length !== 32) throw new Error("a conversation key is 32 bytes")
    try { new Uint8Array(memory.buffer, x.un_ck(), 32).set(key); return f() } finally { x.un_wipe() }
  }
  // the payload's bytes, or the Coax refusal's name (nip44.coax's codes)
  const NIP44 = ["ok", "version", "payload", "base64", "MAC", "padding", "length", "capacity", "key", "output", "alias", "profile", "internal", "peer", "secret"]
  const payload = (n: number, what: string) => {
    if (n <= 0) throw new Error(`NIP-44 refused ${what}: ${NIP44[-n] ?? -n}`)
    return new Uint8Array(memory.buffer, x.un_out(), n)
  }
  return {
    name: "coax",
    getPublicKey: (secret) => withSecret(secret, () => {
      if (x.un_pubkey() !== 1) throw new Error("not a key")
      return hex(new Uint8Array(memory.buffer, x.un_out(), 32).slice())
    }),
    finalizeEvent: (template, secret) => withSecret(secret, () => {
      crypto.getRandomValues(new Uint8Array(memory.buffer, x.un_aux(), 32))
      const t = put(0, JSON.stringify(template.tags)), c = put(1, template.content)
      const n = x.un_sign(BigInt(template.created_at), template.kind, t, c) as number
      if (n <= 0) throw new Error("couldn't sign")
      return JSON.parse(dec.decode(new Uint8Array(memory.buffer, x.un_out(), n))) as Event
    }),
    verifyEvent: (event) => x.un_verify(put(0, JSON.stringify(event))) === UN_OK,
    idMatches: (event) => x.un_id_matches(put(0, JSON.stringify({ ...event, sig: "0".repeat(128) }))) === 1,
    conversationKey: (secret, peer) => withSecret(secret, () => {
      if (!/^[0-9a-f]{64}$/.test(peer)) throw new Error("not a public key")
      new Uint8Array(memory.buffer, x.un_in(0), 32).set(peer.match(/../g)!.map((h) => parseInt(h, 16)))
      const r = x.un_conversation_key() as number
      if (r !== 0) throw new Error(`NIP-44 refused the keys: ${NIP44[r] ?? r}`)
      return new Uint8Array(memory.buffer, x.un_out(), 32).slice()
    }),
    encrypt: (plaintext, key, nonce) => withKey(key, () => {
      const n = new Uint8Array(memory.buffer, x.un_nonce(), 32)
      if (nonce && nonce.length !== 32) throw new Error("a nonce is 32 bytes")
      nonce ? n.set(nonce) : crypto.getRandomValues(n)
      return dec.decode(payload(x.un_encrypt(put(0, plaintext)), "the plaintext"))
    }),
    decrypt: (text, key) => withKey(key, () => dec.decode(payload(x.un_decrypt(put(0, text)), "the payload"))),
  }
}

/** Whoever can open things on this device: a local key or a NIP-07 signer. */
export interface Opener {
  pubkey: string
  decrypt(peer: string, ciphertext: string): Promise<string>
  sign(template: EventTemplate): Promise<Event>
}

export function localOpener(secret: Uint8Array): Opener {
  const pubkey = nostr.getPublicKey(secret)
  return {
    pubkey,
    decrypt: async (peer, ciphertext) => nostr.decrypt(ciphertext, nostr.conversationKey(secret, peer)),
    sign: async (template) => nostr.finalizeEvent(template, secret),
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
  const toRecipient = nostr.conversationKey(author, recipient)
  const content = nostr.encrypt(JSON.stringify(envelope), toRecipient)
  const pubkey = nostr.getPublicKey(author)
  const unsigned = { kind: ARTIFACT_KIND, pubkey, created_at: now, content,
    tags: [["p", recipient], ["h", bytesToHex(randomBytes(32))], ["t", "oa:artifact:v1"]] }
  const rumor = { ...unsigned, id: getEventHash(unsigned) }   // unsigned: the seal authenticates it
  // NIP-59: seal and wrap times are randomized up to two days back
  const jitter = () => now - Math.floor(Math.random() * 2 * 86400)
  const seal = nostr.finalizeEvent({ kind: SEAL_KIND, created_at: jitter(), tags: [],
    content: nostr.encrypt(JSON.stringify(rumor), toRecipient) }, author)
  const once = generateSecretKey()
  return nostr.finalizeEvent({ kind: WRAP_KIND, created_at: jitter(), tags: [["p", recipient]],
    content: nostr.encrypt(JSON.stringify(seal), nostr.conversationKey(once, recipient)) }, once)
}

/** Opens a gift wrap with this device's opener: verifies the wrap and seal
 * signatures, that the seal's author wrote the artifact, that it is addressed
 * to us, and that the inline deck matches its digest and size. */
export async function openDeck(wrap: Event, opener: Opener): Promise<{ deck: Deck; author: string }> {
  if (wrap.kind !== WRAP_KIND || !nostr.verifyEvent(wrap)) throw new Error("not a valid gift wrap")
  if (!wrap.tags.some((t) => t[0] === "p" && t[1] === opener.pubkey)) throw new Error("not addressed to this key")
  const seal = JSON.parse(await opener.decrypt(wrap.pubkey, wrap.content)) as Event
  if (seal.kind !== SEAL_KIND || !nostr.verifyEvent(seal)) throw new Error("not a valid seal")
  const rumor = JSON.parse(await opener.decrypt(seal.pubkey, seal.content)) as Event
  if (rumor.pubkey !== seal.pubkey) throw new Error("the seal and the artifact disagree on the author")
  if (!nostr.idMatches(rumor)) throw new Error("the artifact's id doesn't match it")
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
