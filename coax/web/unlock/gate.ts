// The always-loaded half of hidden presentations: watches what's typed on
// the page for a phrase it can't read. The page carries, per phrase, a salt,
// a short check (truncated SHA-256 of salt and phrase) and a pointer
// AES-GCM-encrypted under a PBKDF2 key from the phrase. Only a typed phrase
// that passes the check is stretched and tried; then the opener module loads.
export type Entry = { salt: string; check: string; iv: string; data: string }
export type Unlocks = { module: string; entries: Entry[] }
export type Pointer = { relay: string; id: string; recipient: string }

export const ROUNDS = 210_000
const CHECK_BYTES = 8
const MIN = 4, MAX = 40

const enc = new TextEncoder()
const hex = (b: ArrayBuffer | Uint8Array) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, "0")).join("")
const unhex = (s: string) => Uint8Array.from(s.match(/../g) ?? [], (h) => parseInt(h, 16))
const b64 = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b)))
const unb64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0))

/** Letters and digits, lower case: "Open Sesame!" and "opensesame" match. */
export const normalize = (phrase: string) => phrase.toLowerCase().replace(/[^a-z0-9]/g, "")

async function check(salt: Uint8Array, phrase: string): Promise<string> {
  const bytes = new Uint8Array(salt.length + phrase.length * 3)
  bytes.set(salt)
  const n = enc.encodeInto(phrase, bytes.subarray(salt.length)).written
  return hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes.subarray(0, salt.length + n))).subarray(0, CHECK_BYTES))
}

async function keyFor(phrase: string, salt: Uint8Array, rounds = ROUNDS): Promise<CryptoKey> {
  const base = await crypto.subtle.importKey("raw", enc.encode(phrase), "PBKDF2", false, ["deriveKey"])
  return crypto.subtle.deriveKey({ name: "PBKDF2", hash: "SHA-256", salt, iterations: rounds }, base,
    { name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"])
}

/** Deploy time: one entry for a phrase and where its presentation is. */
export async function makeEntry(phrase: string, pointer: Pointer, rounds = ROUNDS): Promise<Entry> {
  const p = normalize(phrase)
  if (p.length < MIN || p.length > MAX) throw new Error(`a phrase is ${MIN} to ${MAX} letters or digits`)
  const salt = crypto.getRandomValues(new Uint8Array(16)), iv = crypto.getRandomValues(new Uint8Array(12))
  const data = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, await keyFor(p, salt, rounds), enc.encode(JSON.stringify(pointer)))
  return { salt: hex(salt), check: await check(salt, p), iv: b64(iv), data: b64(data) }
}

/** The pointer if the end of what was typed is an entry's phrase, else null. */
export async function match(typed: string, entries: Entry[], rounds = ROUNDS): Promise<Pointer | null> {
  const t = normalize(typed)
  for (const e of entries) {
    const salt = unhex(e.salt)
    for (let n = MIN; n <= Math.min(MAX, t.length); n++) {
      const tail = t.slice(t.length - n)
      if ((await check(salt, tail)) !== e.check) continue
      try {
        const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(e.iv) }, await keyFor(tail, salt, rounds), unb64(e.data))
        return JSON.parse(new TextDecoder().decode(plain)) as Pointer
      } catch { /* a check collision: keep looking */ }
    }
  }
  return null
}

/** Watches typing (and, after a long press, a phone's keyboard) and opens
 * the presentation behind a matching phrase. */
export function watch(unlocks: Unlocks) {
  let typed = "", busy = false
  const feed = async (text: string) => {
    typed = normalize(typed + text).slice(-MAX)
    if (busy || typed.length < MIN) return
    busy = true
    try {
      const pointer = await match(typed, unlocks.entries)
      if (pointer) { typed = ""; (await import(unlocks.module)).open(pointer) }
    } finally { busy = false }
  }
  addEventListener("keydown", (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey || e.key.length !== 1) return
    if ((e.target as HTMLElement)?.closest?.("input, textarea, [contenteditable]")) return
    feed(e.key)
  })
  // Phones have no keyboard until something asks for one: a long press on
  // the scene focuses an invisible field.
  const field = document.createElement("input")
  Object.assign(field, { type: "text", autocomplete: "off", autocapitalize: "off", spellcheck: false })
  field.setAttribute("aria-hidden", "true")
  field.tabIndex = -1
  field.style.cssText = "position:fixed;left:0;bottom:0;width:1px;height:1px;opacity:0;border:0;padding:0;"
  document.body.append(field)
  field.addEventListener("input", () => { feed(field.value); field.value = "" })
  let press = 0
  addEventListener("pointerdown", (e) => { if (e.pointerType !== "mouse") press = window.setTimeout(() => field.focus(), 900) })
  for (const end of ["pointerup", "pointercancel"]) addEventListener(end, () => clearTimeout(press))
}

const config = typeof document === "undefined" ? null : document.getElementById("coaxial-unlocks")
if (config) watch(JSON.parse(config.textContent ?? "{}") as Unlocks)
