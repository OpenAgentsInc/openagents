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

/** The access terminal: the first keystroke on the scene does nothing you
 * can see; the second opens a HUD terminal along the bottom holding both,
 * and the rest is typed there. Enter sends it: an unknown code is refused,
 * a phrase is granted, the terminal fades and its presentation opens. On a
 * phone, a long press on the scene opens the terminal. */
export function watch(unlocks: Unlocks) {
  const style = document.createElement("style")
  style.textContent = TERMINAL_CSS
  document.head.append(style)
  const bar = document.createElement("form")
  bar.className = "oa-term"
  bar.setAttribute("autocomplete", "off")
  bar.innerHTML = `<div class="oa-term-head"><span>OA://ACCESS</span><span class="oa-term-link">LINK SECURE</span></div>
<label class="oa-term-line"><span class="oa-term-prompt">&gt;</span><input class="oa-term-input" maxlength="${MAX * 2}" spellcheck="false" autocapitalize="off" autocorrect="off" aria-label="Access code"></label>
<div class="oa-term-status" aria-live="polite"></div>`
  document.body.append(bar)
  const input = bar.querySelector("input")!, status = bar.querySelector(".oa-term-status") as HTMLElement
  const idle = "ENTER TO TRANSMIT · ESC TO CLOSE"
  let pending = "", pendingAt = 0, busy = false

  const show = (state: "" | "busy" | "error" | "granted", text: string) => {
    bar.dataset.state = state; status.textContent = text
  }
  const open = (text: string) => {
    bar.classList.add("open"); bar.classList.remove("closing")
    input.value = text; show("", idle)
    input.focus(); input.setSelectionRange(text.length, text.length)
  }
  const close = () => { bar.classList.remove("open"); input.blur(); input.value = ""; pending = "" }

  addEventListener("keydown", (e) => {
    if (bar.classList.contains("open") || e.metaKey || e.ctrlKey || e.altKey || e.key.length !== 1) return
    if ((e.target as HTMLElement)?.closest?.("input, textarea, [contenteditable]")) return
    const now = performance.now()
    pending = now - pendingAt < 2500 ? pending + e.key : e.key
    pendingAt = now
    if (pending.length >= 2) { e.preventDefault(); open(pending); pending = "" }
  })
  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.preventDefault(); close() }
  })
  input.addEventListener("input", () => { if (bar.dataset.state === "error") show("", idle) })
  bar.addEventListener("submit", async (e) => {
    e.preventDefault()
    if (busy || !input.value.trim()) return
    busy = true; show("busy", "VERIFYING")
    try {
      const code = normalize(input.value)
      const pointer = code.length >= MIN && code.length <= MAX ? await match(code, unlocks.entries, ROUNDS, true) : null
      if (!pointer) {
        show("error", `ERROR · UNRECOGNIZED CODE "${input.value.trim().toUpperCase().slice(0, 24)}"`)
        bar.classList.remove("shake"); void bar.offsetWidth; bar.classList.add("shake")
        input.select()
        return
      }
      show("granted", "ACCESS GRANTED")
      input.blur()
      const opener = import(unlocks.module)
      await new Promise((r) => setTimeout(r, 900))
      bar.classList.add("closing")
      await new Promise((r) => setTimeout(r, 600))
      close()
      ;(await opener).open(pointer)
    } finally { busy = false }
  })
  // Phones have no keyboard until something asks for one.
  let press = 0
  addEventListener("pointerdown", (e) => {
    if (e.pointerType === "mouse" || bar.contains(e.target as Node)) return
    press = window.setTimeout(() => open(""), 800)
  })
  for (const end of ["pointerup", "pointercancel", "pointermove"]) addEventListener(end, () => clearTimeout(press))
}

const TERMINAL_CSS = `
.oa-term{--o:#ff8c1f;position:fixed;left:50%;bottom:5vh;z-index:20;width:min(680px,calc(100vw - 32px));box-sizing:border-box;
  transform:translate(-50%,24px);opacity:0;pointer-events:none;transition:opacity .22s ease,transform .22s ease,border-color .2s,box-shadow .2s;
  padding:10px 16px 10px;background:linear-gradient(180deg,rgba(14,10,6,.86),rgba(4,4,6,.9));
  border:1px solid rgba(255,140,31,.42);box-shadow:0 0 24px rgba(255,140,31,.14),inset 0 0 18px rgba(255,140,31,.06);
  clip-path:polygon(12px 0,calc(100% - 12px) 0,100% 12px,100% calc(100% - 12px),calc(100% - 12px) 100%,12px 100%,0 calc(100% - 12px),0 12px);
  font:500 14px/1.4 ui-monospace,"SF Mono",SFMono-Regular,Menlo,Consolas,monospace;color:#eee;backdrop-filter:blur(6px);-webkit-backdrop-filter:blur(6px)}
.oa-term::after{content:"";position:absolute;inset:0;pointer-events:none;
  background:repeating-linear-gradient(180deg,rgba(255,255,255,.035) 0 1px,transparent 1px 3px)}
.oa-term.open{opacity:1;transform:translate(-50%,0);pointer-events:auto}
.oa-term.closing{opacity:0;transform:translate(-50%,-8px);transition:opacity .6s ease,transform .6s ease}
.oa-term-head{display:flex;justify-content:space-between;font-size:10px;letter-spacing:.32em;color:rgba(255,140,31,.75);margin-bottom:6px}
.oa-term-link::before{content:"";display:inline-block;width:6px;height:6px;margin-right:8px;background:var(--o);box-shadow:0 0 8px var(--o);animation:oa-pulse 1.6s ease-in-out infinite}
.oa-term-line{display:flex;align-items:center;gap:10px;cursor:text}
.oa-term-prompt{color:var(--o);text-shadow:0 0 8px rgba(255,140,31,.7)}
.oa-term-input{flex:1;min-width:0;background:transparent;border:0;outline:0;color:#f4f4f4;font:inherit;font-size:16px;letter-spacing:.22em;text-transform:uppercase;
  caret-color:var(--o);text-shadow:0 0 10px rgba(255,255,255,.25);padding:4px 0}
.oa-term-status{margin-top:6px;font-size:10px;letter-spacing:.24em;color:rgba(238,238,238,.45);min-height:1.4em;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.oa-term[data-state=busy] .oa-term-status{color:var(--o)}
.oa-term[data-state=busy] .oa-term-status::after{content:"";animation:oa-dots 1s steps(4) infinite}
.oa-term[data-state=error]{border-color:rgba(255,72,72,.7);box-shadow:0 0 26px rgba(255,60,60,.22),inset 0 0 18px rgba(255,60,60,.08)}
.oa-term[data-state=error] .oa-term-status{color:#ff5a5a;text-shadow:0 0 8px rgba(255,60,60,.6)}
.oa-term[data-state=granted]{border-color:rgba(255,190,110,.9);box-shadow:0 0 40px rgba(255,140,31,.45),inset 0 0 24px rgba(255,140,31,.18)}
.oa-term[data-state=granted] .oa-term-status{color:#ffd29a;letter-spacing:.5em;text-shadow:0 0 12px rgba(255,140,31,.9)}
.oa-term.shake{animation:oa-shake .36s ease}
@keyframes oa-shake{20%{transform:translate(calc(-50% - 7px),0)}40%{transform:translate(calc(-50% + 6px),0)}60%{transform:translate(calc(-50% - 4px),0)}80%{transform:translate(calc(-50% + 2px),0)}}
@keyframes oa-pulse{50%{opacity:.35}}
@keyframes oa-dots{0%{content:""}25%{content:"."}50%{content:".."}75%{content:"..."}}
@media (prefers-reduced-motion:reduce){.oa-term,.oa-term.closing{transition:opacity .2s}.oa-term.shake{animation:none}}
`

const config = typeof document === "undefined" ? null : document.getElementById("coaxial-unlocks")
if (config) watch(JSON.parse(config.textContent ?? "{}") as Unlocks)
