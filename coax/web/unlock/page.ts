// The opener module: loaded only after a phrase matches (gate.ts). It finds
// the recipient key on this device (a NIP-07 signer, or a key imported here
// once and kept in this site's IndexedDB), fetches the gift wrap from the
// relay as that key, opens it (lib.ts) and hands the slides to the page's
// Coax code, which draws them, and the key prompt (ui.coax).
import { bech32 } from "@scure/base"   // NIP-19 nsec only (nostr-effect/nip19 brings all of Effect)
import { deckLines, fetchEvent, localOpener, openDeck, type Opener } from "./lib.ts"
import { send, type Action, type Pointer } from "./gate.ts"

const MSG_KEY_NEEDED = 19, MSG_KEY_WRONG = 20, MSG_LOADING = 21, MSG_DECK = 22, MSG_NOTICE = 23
const ACT_KEY = 17, ACT_CANCEL = 18

type Nip07 = {
  getPublicKey(): Promise<string>
  signEvent(e: object): Promise<any>
  nip44?: { decrypt(peer: string, ciphertext: string): Promise<string> }
}

const STORE = "keys"
function db(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open("coaxial-unlock", 1)
    r.onupgradeneeded = () => r.result.createObjectStore(STORE)
    r.onsuccess = () => resolve(r.result)
    r.onerror = () => reject(r.error)
  })
}
async function kept(pubkey: string): Promise<string | undefined> {
  try {
    const d = await db()
    return await new Promise((res) => { const g = d.transaction(STORE).objectStore(STORE).get(pubkey); g.onsuccess = () => res(g.result); g.onerror = () => res(undefined) })
  } catch { return undefined }
}
async function keep(pubkey: string, secret: string | null) {
  const d = await db()
  await new Promise<void>((res, rej) => {
    const t = d.transaction(STORE, "readwrite")
    secret ? t.objectStore(STORE).put(secret, pubkey) : t.objectStore(STORE).delete(pubkey)
    t.oncomplete = () => res(); t.onerror = () => rej(t.error)
  })
}

const unhex = (s: string) => Uint8Array.from(s.match(/../g) ?? [], (h) => parseInt(h, 16))
const hexOf = (b: Uint8Array) => [...b].map((x) => x.toString(16).padStart(2, "0")).join("")
function parseSecret(text: string): Uint8Array | null {
  const t = text.trim()
  if (/^[0-9a-f]{64}$/i.test(t)) return unhex(t.toLowerCase())
  try {
    const { prefix, words } = bech32.decode(t as `${string}1${string}`, 1500)
    const bytes = bech32.fromWords(words)
    if (prefix === "nsec" && bytes.length === 32) return Uint8Array.from(bytes)
  } catch {}
  return null
}

async function signerFor(recipient: string): Promise<Opener | null> {
  const nostr = (globalThis as { nostr?: Nip07 }).nostr
  if (nostr?.nip44) {
    try {
      if ((await nostr.getPublicKey()) === recipient)
        return { pubkey: recipient, decrypt: (p, c) => nostr.nip44!.decrypt(p, c), sign: (t) => nostr.signEvent(t) }
    } catch { /* declined: fall back to a key kept here */ }
  }
  const secret = await kept(recipient)
  return secret ? localOpener(unhex(secret)) : null
}

/** Calls back with each action the page raises until stop is called. */
function actions(on: (a: Action) => void): () => void {
  const listener = (e: Event) => on((e as CustomEvent<Action>).detail)
  addEventListener("coaxial", listener)
  return () => removeEventListener("coaxial", listener)
}

/** Asks for this presentation's key once on a new device: the page draws
 * the prompt and sends what was typed or pasted (ACT_KEY). */
function askForKey(recipient: string): Promise<Opener | null> {
  return new Promise((resolve) => {
    const stop = actions(async ({ code, text }) => {
      if (code === ACT_CANCEL) { stop(); resolve(null); return }
      if (code !== ACT_KEY) return
      const secret = parseSecret(text)
      const opener = secret && localOpener(secret)
      if (!opener || opener.pubkey !== recipient) { send(MSG_KEY_WRONG); return }
      stop()
      await keep(recipient, hexOf(secret!))
      resolve(opener)
    })
    send(MSG_KEY_NEEDED)
  })
}

export async function open(pointer: Pointer) {
  let cancelled = false
  const stop = actions(({ code }) => { if (code === ACT_CANCEL) cancelled = true })
  try {
    const opener = (await signerFor(pointer.recipient)) ?? (await askForKey(pointer.recipient))
    if (!opener || cancelled) return
    send(MSG_LOADING)
    const wrap = await fetchEvent(pointer.relay, pointer.id, opener)
    if (cancelled) return
    if (!wrap) return send(MSG_NOTICE, "Not found.")
    const { deck } = await openDeck(wrap, opener)
    if (cancelled) return
    document.title = deck.title
    send(MSG_DECK, deckLines(deck))
  } catch (e) {
    if (!cancelled) send(MSG_NOTICE, e instanceof Error ? e.message : "Couldn't open it.")
  } finally { stop() }
}

/** Forgets this device's copy of a key (from the console). */
export const forget = (pubkey: string) => keep(pubkey, null)
