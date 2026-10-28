// The opener module: loaded only after a phrase matches (gate.ts). It finds
// the recipient key on this device (a NIP-07 signer, or a key imported here
// once and kept in this site's IndexedDB), fetches the gift wrap from the
// relay as that key, opens it (lib.ts) and shows the presentation.
import { bech32 } from "@scure/base"   // NIP-19 nsec only (nostr-effect/nip19 brings all of Effect)
import { fetchEvent, localOpener, openDeck, type Deck, type Opener } from "./lib.ts"
import type { Pointer } from "./gate.ts"

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

// ---- the overlay ---------------------------------------------------------

const css = `
.cx-deck{position:fixed;inset:0;z-index:10;display:grid;place-items:center;background:rgba(0,0,0,.74);
  backdrop-filter:blur(3px);-webkit-backdrop-filter:blur(3px);color:#ededed;opacity:0;transition:opacity .5s;
  font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif}
.cx-deck.on{opacity:1}
.cx-slide{width:min(880px,calc(100vw - 32px));max-height:calc(100vh - 96px);overflow:auto;padding:8px 0}
.cx-slide h2{margin:0 0 .35em;font-size:clamp(22px,4.2vw,40px);font-weight:500;letter-spacing:.14em;text-transform:uppercase;
  text-shadow:0 0 18px rgba(255,255,255,.28)}
.cx-slide h3{margin:0 0 1.2em;font-size:clamp(14px,2vw,18px);font-weight:400;color:#a9a9b0;letter-spacing:.04em}
.cx-slide ul{margin:0;padding:0;list-style:none}
.cx-slide li{position:relative;margin:.55em 0;padding-left:1.3em;font-size:clamp(15px,2vw,20px);line-height:1.45}
.cx-slide li::before{content:"";position:absolute;left:0;top:.62em;width:.42em;height:.42em;border-radius:50%;
  background:#f7931a;box-shadow:0 0 10px rgba(247,147,26,.8)}
.cx-slide p{white-space:pre-line;font-size:clamp(15px,2vw,20px);line-height:1.5;color:#d6d6da}
.cx-slide strong{color:#fff;font-weight:600}
.cx-notes{margin-top:1.6em;padding-top:1em;border-top:1px solid #2a2a30;color:#8d8d96;font-size:14px;white-space:pre-line;display:none}
.cx-deck.notes .cx-notes{display:block}
.cx-foot{position:fixed;left:0;right:0;bottom:18px;display:flex;justify-content:center;gap:18px;font-size:12px;
  letter-spacing:.3em;color:#77777f}
.cx-bar{position:fixed;left:0;top:0;height:2px;background:#f7931a;box-shadow:0 0 8px #f7931a;transition:width .3s}
.cx-ask{width:min(440px,calc(100vw - 32px));text-align:left}
.cx-ask p{color:#a9a9b0;font-size:14px;line-height:1.5}
.cx-ask textarea{width:100%;box-sizing:border-box;height:84px;background:#0d0d10;color:#ededed;border:1px solid #2f2f36;
  border-radius:6px;padding:10px;font:13px ui-monospace,Menlo,monospace}
.cx-ask button{margin-top:10px;background:#f7931a;color:#000;border:0;border-radius:6px;padding:9px 16px;font-weight:600;cursor:pointer}
.cx-ask .cx-err{color:#ff8a7a}
`

let root: HTMLDivElement | null = null
function overlay(): HTMLDivElement {
  if (root) return root
  const style = document.createElement("style"); style.textContent = css; document.head.append(style)
  root = document.createElement("div"); root.className = "cx-deck"; root.setAttribute("role", "dialog")
  document.body.append(root)
  requestAnimationFrame(() => root!.classList.add("on"))
  return root
}
function close() {
  if (!root) return
  const r = root; root = null
  removeEventListener("keydown", onKey)
  r.classList.remove("on"); setTimeout(() => r.remove(), 500)
}

/** Text with **bold**, as DOM nodes: deck text never becomes markup. */
function rich(el: HTMLElement, text: string) {
  text.split(/(\*\*[^*]+\*\*)/).forEach((part) => {
    if (/^\*\*[^*]+\*\*$/.test(part)) { const b = document.createElement("strong"); b.textContent = part.slice(2, -2); el.append(b) }
    else if (part) el.append(document.createTextNode(part))
  })
}
const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string) => {
  const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) rich(e, text); return e
}

let deck: Deck | null = null, at = 0
function show() {
  const r = overlay(); r.replaceChildren()
  if (!deck) return
  const s = deck.slides[at]
  const slide = el("div", "cx-slide")
  slide.append(el("h2", undefined, s.title))
  if (s.subtitle) slide.append(el("h3", undefined, s.subtitle))
  if (s.body) slide.append(el("p", undefined, s.body))
  if (s.bullets) { const ul = el("ul"); s.bullets.forEach((b) => ul.append(el("li", undefined, b))); slide.append(ul) }
  if (s.notes) slide.append(el("div", "cx-notes", s.notes))
  const bar = el("div", "cx-bar"); bar.style.width = `${((at + 1) / deck.slides.length) * 100}%`
  r.append(bar, slide, el("div", "cx-foot", `${at + 1} / ${deck.slides.length}`))
  r.dataset.slide = String(at + 1)
}
function go(d: number) { if (deck) { at = Math.max(0, Math.min(deck.slides.length - 1, at + d)); show() } }
function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement)?.closest?.("textarea, input")) { if (e.key === "Escape") close(); return }
  if (e.key === "Escape") close()
  else if (["ArrowRight", "PageDown", " ", "Enter"].includes(e.key)) { e.preventDefault(); go(1) }
  else if (["ArrowLeft", "PageUp", "Backspace"].includes(e.key)) { e.preventDefault(); go(-1) }
  else if (e.key === "Home") { at = 0; show() }
  else if (e.key === "n") root?.classList.toggle("notes")
}

function present(d: Deck) {
  deck = d; at = 0; show()
  const r = overlay()
  r.addEventListener("click", (e) => { if (!(e.target as HTMLElement).closest("textarea, button")) go(e.clientX > innerWidth / 2 ? 1 : -1) })
  let x0 = 0
  r.addEventListener("touchstart", (e) => { x0 = e.touches[0].clientX }, { passive: true })
  r.addEventListener("touchend", (e) => { const dx = e.changedTouches[0].clientX - x0; if (Math.abs(dx) > 40) go(dx < 0 ? 1 : -1) })
  document.title = d.title
}

/** Asks for this presentation's key once on a new device. */
function askForKey(recipient: string): Promise<Opener | null> {
  return new Promise((resolve) => {
    const r = overlay(); r.replaceChildren()
    const box = el("div", "cx-ask")
    box.append(el("h2", undefined, "Key needed"),
      el("p", undefined, "This needs the key it was sealed to. Paste it (nsec or hex) to keep it on this device, in this browser only."))
    const area = document.createElement("textarea"); area.autocomplete = "off"; area.spellcheck = false
    const err = el("p", "cx-err")
    const go = el("button", undefined, "Open")
    go.onclick = async () => {
      const secret = parseSecret(area.value)
      const opener = secret && localOpener(secret)
      if (!opener || opener.pubkey !== recipient) { err.textContent = "That isn't the key this was sealed to."; return }
      area.value = ""
      await keep(recipient, hexOf(secret!))
      resolve(opener)
    }
    box.append(area, go, err)
    r.append(box)
    area.focus()
  })
}

function notice(text: string) {
  const r = overlay(); r.replaceChildren(el("div", "cx-ask", text))
  setTimeout(close, 2600)
}

export async function open(pointer: Pointer) {
  addEventListener("keydown", onKey)
  overlay()
  try {
    const opener = (await signerFor(pointer.recipient)) ?? (await askForKey(pointer.recipient))
    if (!opener) return close()
    const wrap = await fetchEvent(pointer.relay, pointer.id, opener)
    if (!wrap) return notice("Not found.")
    present((await openDeck(wrap, opener)).deck)
  } catch (e) {
    notice(e instanceof Error ? e.message : "Couldn't open it.")
  }
}

/** Forgets this device's copy of a key (from the console). */
export const forget = (pubkey: string) => keep(pubkey, null)
