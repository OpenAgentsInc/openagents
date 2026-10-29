// The playtest flow, walked in headless Chrome with WebGPU (no window opens,
// nothing takes focus): the landing, ENTER as a guest, the galaxy, a click
// on the Haunted House, the house; then every other realm in realms.json,
// each opened from the galaxy (/verse/galaxy?realms=all, the arrows to its
// world, ENTER), unlisted ones too, so a realm is walked before its launch;
// then the pages of their own (the Garden) by their address. Each page must start (its engine says
// ready, nothing reports an error or throws) and draw (enough lit pixels),
// and looks like its golden screenshot. It closes the gap the HTTP smoke
// leaves: a page that answers 200 but doesn't start, or draws nothing.
// Every analytics batch the walk sends is read too (beacons go as fetches,
// which the browser reports): in the house it runs in through the front
// door, clicks and says it is leaving, and each event in analytics-contract.json must have
// been sent by its page, or the walk fails and names what is missing
// (INVARIANTS.md, "Analytics never lose visibility").
// release.sh runs it on staging before promoting (goldens and all) and on
// production after (--walk); pin.sh walks a pinned demo (--walk --root).
//
//   bun coax/web/goldens.ts BASE                  # walk, and compare each page with its golden
//   bun coax/web/goldens.ts BASE --update         # approve: this walk's shots become the goldens
//   bun coax/web/goldens.ts BASE --walk           # walk only: starts, draws, the flow goes through
//   bun coax/web/goldens.ts BASE --root /demo/N   # a pinned demo's pages
//
// A golden is coax/web/goldens/NAME.png at 1280x800, NAME a realm's slug for
// a realm; what a realm's page must light, how long it settles and how many
// blocks may change are its "golden" in realms.json. A page differs when
// more of its 8 px blocks changed than the page allows (a block changed
// when its mean differs by more than SHADE in a channel; 16,000 blocks a
// page). Every page runs in golden mode (host.js): an app that knows it
// holds its effects (the house's candles, fireflies, embers and dust) at
// one moment. Blocks that still move on their own (the galaxy's orbits, the
// stars) are found by three more shots of the page and left out, with the
// blocks round them.
// A moved panel or a changed word does not pass. A shot that differs is kept and its path printed: look at it, and
// if the change is meant, run --update against staging and commit the
// goldens with the change.
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync, mkdirSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { inflateSync } from "node:zlib"
import { worlds } from "./realms"
import { missingGolden } from "./golden-policy"

const W = 1280, H = 800, BLOCK = Number(process.env.BLOCK ?? 8), SHADE = Number(process.env.SHADE ?? 8)
const args = process.argv.slice(2)
const flag = (n: string) => args.includes(`--${n}`)
const value = (n: string, d = "") => { const i = args.indexOf(`--${n}`); return i >= 0 ? args[i + 1] : d }
const base = (args[0] ?? "").replace(/\/$/, ""), root = value("root"), update = flag("update"), walkOnly = flag("walk")
if (!base.startsWith("http")) { console.error("usage: bun coax/web/goldens.ts BASE [--update] [--walk] [--root /demo/NAME]"); process.exit(2) }
const goldens = join(import.meta.dir, "goldens")
const contract: Record<string, string[]> = JSON.parse(readFileSync(join(import.meta.dir, "analytics-contract.json"), "utf8"))
const sent = new Set<string>()
// The least each page draws: the share of pixels lit (a channel over 40).
// The landing and the galaxy are words and stars on black; the house, a dark night,
// lights about 6% (its loading screen, or one that failed, under 1%).
// blocks: how many of its blocks may change against its golden.
type Page = { path: string; lit: number; settle: number; blocks: number }
const PAGES: Record<string, Page> = { landing: { path: "/verse", lit: 0.004, settle: 2500, blocks: 4 },
  galaxy: { path: "/verse/galaxy", lit: 0.01, settle: 2500, blocks: 8 } }
// Every realm, and the open worlds in the order the galaxy's arrows step
// through them with ?realms=all (realms.json's order).
const table = worlds(), open = table.filter((w) => w.kind === "open" && w.realm)
for (const w of open) PAGES[w.realm!.slug] = { path: `/realm/${w.realm!.slug}`, ...w.realm!.golden }
// Realms the galaxy doesn't list yet: with no golden, their shot waits in pending/ (golden-policy.ts).
const unlisted = new Set(open.filter((w) => w.listed === false).map((w) => w.realm!.slug))
const pendingDir = process.env.GOLDENS_PENDING || join(goldens, "pending")
// Pages of their own the galaxy doesn't open (not in realms.json), walked by their address.
const BY_ADDRESS: Record<string, Page> = { garden: { path: "/realm/garden", lit: 0.5, settle: 3000, blocks: 6 } }
for (const [name, page] of Object.entries(BY_ADDRESS)) PAGES[name] ??= page
type Name = string

// ---- PNG: Chrome's screenshots (8-bit RGB or RGBA, not interlaced) to RGBA
type Image = { w: number; h: number; px: Uint8Array }
function png(bytes: Uint8Array): Image {
  const b = Buffer.from(bytes)
  let at = 8, w = 0, h = 0, type = 0
  const data: Buffer[] = []
  while (at < b.length) {
    const n = b.readUInt32BE(at), kind = b.toString("latin1", at + 4, at + 8)
    if (kind === "IHDR") { w = b.readUInt32BE(at + 8); h = b.readUInt32BE(at + 12); type = b[at + 17]; if (b[at + 16] !== 8 || b[at + 20] !== 0) throw new Error("png: not 8-bit") }
    if (kind === "IDAT") data.push(b.subarray(at + 8, at + 8 + n))
    at += 12 + n
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0
  if (!bpp) throw new Error(`png: colour type ${type}`)
  const raw = inflateSync(Buffer.concat(data)), stride = w * bpp
  const rows = new Uint8Array(h * stride), px = new Uint8Array(w * h * 4)
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)], src = y * (stride + 1) + 1, o = y * stride
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? rows[o + x - bpp] : 0, up = y ? rows[o - stride + x] : 0, c = x >= bpp && y ? rows[o - stride + x - bpp] : 0
      const p = a + up - c, pa = Math.abs(p - a), pb = Math.abs(p - up), pc = Math.abs(p - c)
      const pred = f === 1 ? a : f === 2 ? up : f === 3 ? (a + up) >> 1 : f === 4 ? (pa <= pb && pa <= pc ? a : pb <= pc ? up : c) : 0
      rows[o + x] = (raw[src + x] + pred) & 255
    }
    for (let x = 0; x < w; x++) for (let k = 0; k < 4; k++) px[(y * w + x) * 4 + k] = k < bpp ? rows[o + x * bpp + k] : 255
  }
  return { w, h, px }
}
const lit = (m: Image) => { let n = 0; for (let i = 0; i < m.px.length; i += 4) if (Math.max(m.px[i], m.px[i + 1], m.px[i + 2]) > 40) n++; return n / (m.w * m.h) }
// Which blocks differ between two shots (1 a block).
function differ(a: Image, b: Image) {
  const bw = Math.ceil(a.w / BLOCK), bh = Math.ceil(a.h / BLOCK), out = new Uint8Array(bw * bh)
  if (a.w !== b.w || a.h !== b.h) return out.fill(1)
  for (let by = 0; by < bh; by++) for (let bx = 0; bx < bw; bx++) {
    const sum = [0, 0, 0]; let n = 0
    for (let y = by * BLOCK; y < Math.min(a.h, by * BLOCK + BLOCK); y++) for (let x = bx * BLOCK; x < Math.min(a.w, bx * BLOCK + BLOCK); x++) {
      const i = (y * a.w + x) * 4
      for (let k = 0; k < 3; k++) sum[k] += a.px[i + k] - b.px[i + k]
      n++
    }
    if (sum.some((s) => Math.abs(s / n) > SHADE)) out[by * bw + bx] = 1
  }
  return out
}
// The blocks that changed against the golden, leaving out those that moved
// between this page's own shots, and the box round them.
function against(shots: Image[], golden: Image) {
  const moving = new Uint8Array(differ(shots[0], shots[0]).length)
  for (let i = 1; i < shots.length; i++) differ(shots[0], shots[i]).forEach((v, k) => { moving[k] |= v })
  // a flickering light reaches past the blocks seen moving: their neighbours too
  const bw = Math.ceil(golden.w / BLOCK), near = moving.slice()
  moving.forEach((v, k) => { if (v) for (const o of [-bw - 1, -bw, -bw + 1, -1, 1, bw - 1, bw, bw + 1]) if (near[k + o] !== undefined) near[k + o] = 1 })
  const d = differ(shots[0], golden)
  let changed = 0, x0 = Infinity, y0 = Infinity, x1 = -1, y1 = -1, moved = 0
  d.forEach((v, k) => {
    moved += near[k]
    if (!v || near[k]) return
    changed++; const bx = k % bw, by = (k / bw) | 0
    x0 = Math.min(x0, bx); y0 = Math.min(y0, by); x1 = Math.max(x1, bx); y1 = Math.max(y1, by)
  })
  return { changed, moved, box: changed ? `x ${x0 * BLOCK}-${(x1 + 1) * BLOCK}, y ${y0 * BLOCK}-${(y1 + 1) * BLOCK}` : "" }
}

// ---- Chrome
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "coax-goldens-"))
const proc = Bun.spawn([chrome, "--headless=new", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars",
  "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal", `--window-size=${W},${H}`,
  "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
const failed: string[] = []
const check = (ok: boolean, what: string) => { console.log((ok ? "  ok    " : "  FAIL  ") + what); if (!ok) failed.push(what) }
try {
  const portFile = join(profile, "DevToolsActivePort")
  for (let i = 0; i < 200 && !existsSync(portFile); i++) await Bun.sleep(50)
  const port = readFileSync(portFile, "utf8").split("\n")[0]
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((r) => ws.addEventListener("open", r, { once: true }))
  let id = 0
  const pending = new Map<number, (v: any) => void>()
  let errors: string[] = []
  const inflight = new Set<string>()
  let quietSince = Date.now()
  ws.addEventListener("message", (e) => {
    const m = JSON.parse(String(e.data))
    if (m.id) { pending.get(m.id)?.(m); pending.delete(m.id); return }
    const p = m.params
    if (m.method === "Runtime.exceptionThrown") errors.push(p.exceptionDetails?.exception?.description?.split("\n")[0] ?? p.exceptionDetails?.text)
    if (m.method === "Runtime.consoleAPICalled" && p.type === "error") errors.push(p.args.map((a: any) => a.value ?? a.description).join(" ").slice(0, 200))
    if (m.method === "Network.requestWillBeSent" && !p.request.url.includes("/api/analytics")) inflight.add(p.requestId)
    if (m.method === "Network.requestWillBeSent" && p.request.url.includes("/api/analytics/events") && p.request.method === "POST") {
      const take = (body: string) => { try { for (const e of JSON.parse(body).events ?? []) sent.add(`${e.page}/${e.ev}`) } catch {} }
      if (p.request.postData) take(p.request.postData)
      else if (p.request.hasPostData) send("Network.getRequestPostData", { requestId: p.requestId }).then((r) => r.result?.postData && take(r.result.postData))
    }
    if (m.method === "Network.loadingFinished" || m.method === "Network.loadingFailed") { if (inflight.delete(p.requestId) && !inflight.size) quietSince = Date.now() }
  })
  const send = (method: string, params: any = {}) => new Promise<any>((r) => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  const evaluate = async (expression: string) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result?.result?.value
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false })
  await send("Runtime.enable"); await send("Network.enable")
  await send("Page.enable")
  // golden mode on every page of the walk: particles, flicker and other effects hold still (host.js coaxial_golden)
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialGolden = true" })
  if (!root) await send("Page.addScriptToEvaluateOnNewDocument", { source: "Object.defineProperty(Navigator.prototype, 'sendBeacon', { value: () => false })" })
  const shot = async () => Buffer.from((await send("Page.captureScreenshot", { format: "png" })).result.data, "base64")
  const until = async (test: () => Promise<boolean>, ms: number) => { const end = Date.now() + ms; while (Date.now() < end) { if (await test()) return true; await Bun.sleep(200) } return false }

  // A page reached: it starts, its files arrive, it settles, and it draws.
  const arrive = async (name: Name) => {
    const page = PAGES[name], want = base + root + page.path
    const there = await until(async () => (await evaluate("location.href")).split(/[?#]/)[0] === want, 15000)
    check(there, `${name}: at ${want}`)
    if (!there) return null
    const ready = await until(async () => (await evaluate("globalThis.coaxial?.ready === true")) === true, 20000)
    check(ready, `${name}: the engine started`)
    await until(async () => !inflight.size && Date.now() - quietSince > 1500, 30000)
    await Bun.sleep(page.settle)
    const bytes = await shot(), image = png(bytes), share = lit(image)
    check(errors.length === 0, `${name}: no errors${errors.length ? ` (${errors.join("; ")})` : ""}`)
    errors = []
    check(share >= page.lit, `${name}: draws (${(share * 100).toFixed(1)}% lit, at least ${(page.lit * 100).toFixed(1)}%)`)
    if (walkOnly) return image
    const golden = join(goldens, `${name}.png`)
    if (update) { mkdirSync(goldens, { recursive: true }); writeFileSync(golden, bytes); console.log(`  new   ${golden}`); return image }
    if (!existsSync(golden)) {
      const m = missingGolden(name, unlisted.has(name), pendingDir, base)
      if (m.fail) { check(false, m.line); return image }
      mkdirSync(pendingDir, { recursive: true }); writeFileSync(m.pending, bytes)
      console.log(`  ${m.line}`)
      return image
    }
    // more shots: what moves on its own (candles, orbits) isn't compared
    const shots = [image]
    for (let i = 0; i < 3; i++) { await Bun.sleep(500); shots.push(png(await shot())) }
    const d = against(shots, png(readFileSync(golden)))
    const ok = d.changed <= page.blocks
    let kept = ""
    if (!ok) { kept = join(mkdtempSync(join(tmpdir(), "coax-golden-")), `${name}.png`); writeFileSync(kept, bytes) }
    check(ok, `${name}: matches its golden (${d.changed} blocks changed, ${page.blocks} allowed, ${d.moved} moving${d.box ? `, ${d.box}` : ""}${kept ? `; this shot: ${kept}` : ""})`)
    return image
  }
  const key = async (key: string, code: string, vk: number) => {
    for (const type of ["keyDown", "keyUp"]) await send("Input.dispatchKeyEvent", { type, key, code, windowsVirtualKeyCode: vk })
  }

  await send("Page.navigate", { url: base + root + PAGES.landing.path })
  if (await arrive("landing")) {
    await key("Enter", "Enter", 13)   // ENTER: play now as a guest
    const galaxy = await arrive("galaxy")
    if (galaxy) {
      // The Haunted House's tag (orange words under its planet) moves with the
      // orbit: click the middle of the row with the most orange in it.
      const fresh = png(await shot()), rows = new Array(fresh.h).fill(0), xs: number[][] = Array.from({ length: fresh.h }, () => [])
      for (let y = 0; y < fresh.h; y++) for (let x = 0; x < fresh.w; x++) {
        const i = (y * fresh.w + x) * 4, r = fresh.px[i], g = fresh.px[i + 1], b = fresh.px[i + 2]
        if (r > 170 && g > 80 && g < 190 && b < 100 && r - g > 50) { rows[y]++; xs[y].push(x) }
      }
      const y = rows.indexOf(Math.max(...rows)), x = xs[y].sort((a, b) => a - b)[xs[y].length >> 1]
      check(rows[y] > 20, `galaxy: the Haunted House's tag is drawn (${rows[y]} orange pixels at y ${y})`)
      if (rows[y] > 20) {
        for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
          await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
        if (await arrive("haunted-house") && !root) {
          const t0 = Date.now()
          const want = Object.entries(contract).flatMap(([page, evs]) => evs.map((ev) => `${page}/${ev}`))
          const seen = (leave: boolean) => want.every((w) => sent.has(w) || (!leave && w.endsWith("/world-leave")))
          for (const type of ["keyDown", "keyUp"]) {
            if (type === "keyUp") await until(async () => sent.has("haunt/quest"), 8000)
            await send("Input.dispatchKeyEvent", { type, key: "Shift", code: "ShiftLeft", windowsVirtualKeyCode: 16, modifiers: type === "keyDown" ? 8 : 0 })
            await send("Input.dispatchKeyEvent", { type, key: "w", code: "KeyW", windowsVirtualKeyCode: 87, modifiers: type === "keyDown" ? 8 : 0 })
          }
          for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
            await send("Input.dispatchMouseEvent", { type, x: W / 2, y: H / 3, button: "left", clickCount: 1 })
          await until(async () => seen(false), 3000)
          await evaluate("dispatchEvent(new Event('pagehide'))")
          await until(async () => seen(true), 4000)
          const missing = want.filter((w) => !sent.has(w))
          check(!missing.length, `analytics contract: every contract event sent (${missing.length ? "missing " + missing.join(", ") : want.length + " events"}, ${((Date.now() - t0) / 1000).toFixed(1)} s)`)
        }
      }
    }
  }
  // Every other realm, opened from the galaxy with the keyboard (a pinned demo has only the house).
  if (!root) for (const [k, w] of open.entries()) {
    const slug = w.realm!.slug
    if (slug === "haunted-house") continue
    await send("Page.navigate", { url: `${base}/verse/galaxy?realms=all` })
    const ready = await until(async () => (await evaluate("location.pathname")) === "/verse/galaxy" && (await evaluate("globalThis.coaxial?.ready === true")) === true, 20000)
    check(ready, `${slug}: the galaxy started, to open it from`)
    if (!ready) continue
    await Bun.sleep(1000)
    errors = []
    for (let i = 0; i <= k; i++) await key("ArrowRight", "ArrowRight", 39)
    await key("Enter", "Enter", 13)
    await arrive(slug)
  }
  if (!root) for (const name of Object.keys(BY_ADDRESS)) {
    if (open.some((w) => w.realm!.slug === name)) continue
    await send("Page.navigate", { url: base + PAGES[name].path })
    errors = []
    await arrive(name)
  }
  if (!root && !sent.size) check(false, "analytics contract: no analytics batch was sent")
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(`goldens ${base}${root}: ` + (failed.length ? `${failed.length} failed` : update ? "updated" : "passed"))
process.exit(failed.length ? 1 : 0)
