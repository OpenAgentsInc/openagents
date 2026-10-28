// The always-loaded half of hidden presentations: checks the codes typed into
// the page's access terminal for a phrase it can't read. The terminal is
// drawn by the page's Coax code (apps/openagents-web/ui.coax); this file only
// answers it. The page carries, per phrase, a salt, a short check (truncated
// SHA-256 of salt and phrase) and a pointer AES-GCM-encrypted under a PBKDF2
// key from the phrase. Only a code that passes the check is stretched and
// tried; then the opener module loads.
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

/** The pointer if what was typed is an entry's phrase, else null. With
 * exact false, any ending of what was typed may be the phrase. */
export async function match(typed: string, entries: Entry[], rounds = ROUNDS, exact = false): Promise<Pointer | null> {
  const t = normalize(typed)
  for (const e of entries) {
    const salt = unhex(e.salt)
    for (let n = exact ? t.length : MIN; n <= Math.min(MAX, t.length); n++) {
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

// The page's Coax code and this file talk through the Coaxial host
// (coax/web/host.js): actions arrive as "coaxial" events, answers go back
// with coaxial.send. The numbers are ui.coax's.
export const MSG_ARMED = 16, MSG_REFUSED = 17, MSG_GRANTED = 18
export const ACT_READY = 0, ACT_CODE = 16
export type Action = { code: number; text: string }
type Host = { send(kind: number, text?: string): void; ready?: boolean }
export const host = () => (globalThis as { coaxial?: Host }).coaxial
export const send = (kind: number, text = "") => host()?.send(kind, text)

/** Arms the page's access terminal and answers each code it sends: refused,
 * or granted, and then the opener module opens its presentation. */
export function arm(unlocks: Unlocks) {
  addEventListener("coaxial", async (e) => {
    const { code, text } = (e as CustomEvent<Action>).detail
    if (code === ACT_READY) return send(MSG_ARMED)
    if (code !== ACT_CODE) return
    const typed = normalize(text)
    const pointer = typed.length >= MIN && typed.length <= MAX ? await match(typed, unlocks.entries, ROUNDS, true) : null
    if (!pointer) return send(MSG_REFUSED)
    send(MSG_GRANTED)
    ;(await import(unlocks.module)).open(pointer)
  })
  if (host()?.ready) send(MSG_ARMED)
}

const config = typeof document === "undefined" ? null : document.getElementById("coaxial-unlocks")
if (config) arm(JSON.parse(config.textContent ?? "{}") as Unlocks)
