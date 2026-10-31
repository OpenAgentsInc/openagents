// The playtest flow, walked in headless Chrome with WebGPU (no window opens,
// nothing takes focus): the landing, ENTER as a guest directly to the house,
// then the galaxy and a click on the Haunted House; then every other realm in realms.json,
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
// production after: --starts by default, --walk with release.sh --thorough;
// pin.sh walks a pinned demo (--walk --root).
// The walk runs in lanes side by side, each in its own headless tab
// (GOLDENS_POOL tabs, default 4; GOLDENS_CHROMES=1 for one Chrome a lane):
// the journey (landing, house, galaxy, the house's analytics contract), each
// other realm from the galaxy, the Garden. Each page keeps its own checks.
// With --unchanged-against ORIGIN --since COMMIT (release.sh, on staging), a
// page byte-identical to what production serves, whose golden inputs haven't
// changed since COMMIT, skips its golden comparison but still starts and
// draws (see "Pages unchanged" below); --full or GOLDENS_FULL=1 compares all.
//
//   bun coax/web/goldens.ts BASE                  # walk, and compare each page with its golden
//   bun coax/web/goldens.ts BASE --update         # approve: this walk's shots become the goldens
//   bun coax/web/goldens.ts BASE --walk           # walk only: starts, draws, the flow goes through
//   bun coax/web/goldens.ts BASE --starts         # brief: each page by its address answers and its engine starts, no errors
//   bun coax/web/goldens.ts BASE --root /demo/N   # a pinned demo's pages
//   bun coax/web/goldens.ts BASE --unchanged-against https://openagents.com --since COMMIT [--full]
//   bun coax/web/goldens.ts BASE --only landing,galaxy   # compare only these pages (golden-check.sh)
//   bun coax/web/goldens.ts BASE --report FILE    # also write the failures as JSON (release.sh's hold-back)
//
// A golden is coax/web/goldens/NAME.png (the Mac's; Linux's in goldens/linux/, golden-pages.ts GOLDEN_DIR) at 1280x800 (or its page's own size), NAME a realm's slug for
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
import { type World } from "./realms"
import { goldenPages, GOLDEN_DIR, type Page } from "./golden-pages"
import { classify, missingGolden } from "./golden-policy"
import { launch as launchChrome } from "./chrome"

const W = 1280, H = 800, BLOCK = Number(process.env.BLOCK ?? 8), SHADE = Number(process.env.SHADE ?? 8)
const args = process.argv.slice(2)
const flag = (n: string) => args.includes(`--${n}`)
const value = (n: string, d = "") => { const i = args.indexOf(`--${n}`); return i >= 0 ? args[i + 1] : d }
const base = (args[0] ?? "").replace(/\/$/, ""), root = value("root"), update = flag("update"), walkOnly = flag("walk") || flag("starts"), startsOnly = flag("starts")
if (!base.startsWith("http")) { console.error("usage: bun coax/web/goldens.ts BASE [--update] [--walk] [--root /demo/NAME] [--starts] [--unchanged-against ORIGIN --since COMMIT] [--full] [--only NAME,NAME] [--report FILE]"); process.exit(2) }
const goldens = join(import.meta.dir, "../..", GOLDEN_DIR)
const contract: Record<string, string[]> = JSON.parse(readFileSync(join(import.meta.dir, "analytics-contract.json"), "utf8"))
const sent = new Set<string>()
// The pages and their numbers (golden-pages.ts): the least share of pixels
// each lights (the landing and the galaxy are words and stars on black; the
// house, a dark night, lights about 6%, its loading screen or one that failed
// under 1%), how long it settles, how many of its blocks may change.
// --only NAME,NAME compares only those pages with their goldens (golden-check.sh):
// the lanes that reach them still run, and every page they reach must start and draw.
const { pages: PAGES, byAddress: BY_ADDRESS, open, unlisted } = goldenPages()
const only = new Set(value("only").split(",").filter(Boolean))
for (const n of only) if (!PAGES[n]) { console.error(`goldens: --only names no page ${n}`); process.exit(2) }
const wanted = (name: string) => !only.size || only.has(name)
const pendingDir = process.env.GOLDENS_PENDING || join(goldens, "pending")
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

// ---- Pages unchanged since production's last walked release (--unchanged-against)
// A page may skip its golden comparison when what staging serves for it is
// byte-identical to what production serves now, production's pages passed a
// walk with their goldens (release.sh passes --since only then: the newest
// release in the production bucket says it was staged and went live), and
// what the comparison would use is unchanged since that release's commit:
// the page's golden, its numbers in realms.json, and this file. The bytes
// are the page's HTML, every file the HTML names, and, once the page has
// started, every file it fetched; if any differs the page is walked in full.
// A skipped page still starts, shows no errors and draws, opened by its
// address. GOLDENS_FULL=1 (or --full) walks every page in full.
const reference = value("unchanged-against").replace(/\/$/, ""), since = value("since")
const full = flag("full") || process.env.GOLDENS_FULL === "1"
const sha = (b: Uint8Array | string) => new Bun.CryptoHasher("sha256").update(b).digest("hex")
const bodies = new Map<string, Promise<string>>()
const body = (url: string) => {
  if (!bodies.has(url)) bodies.set(url, fetch(url, { headers: { "cache-control": "no-cache" } })
    .then(async (r) => r.ok ? sha(new Uint8Array(await r.arrayBuffer())) : `HTTP ${r.status}`).catch((e) => `error ${e}`))
  return bodies.get(url)!
}
// The same-origin files a page's HTML names (src, href, data-*: "/a/b.js" or "name:/a/b").
const named = (html: string) => {
  const out = new Set<string>()
  for (const m of html.matchAll(/(?:src|href|data-[a-z-]+)="([^"]+)"/g))
    for (const part of m[1].split(/[,\s]+/)) { const p = part.replace(/^[a-z]+:(?=\/)/, ""); if (p.startsWith("/") && !p.startsWith("//")) out.add(p.split("#")[0]) }
  return out
}
// Paths the same bytes must answer on both: same origin, not the API.
const comparable = (path: string) => path.startsWith("/") && !path.startsWith("/api/")
async function differs(paths: Iterable<string>): Promise<string> {
  const list = [...paths].filter(comparable).sort()
  const same = await Promise.all(list.map(async (p) => { const [a, b] = await Promise.all([body(base + p), body(reference + p)]); return a === b && /^[0-9a-f]{64}$/.test(a) }))
  return list.find((_, i) => !same[i]) ?? ""
}
// What a golden comparison of a page uses, at a commit: its golden, its numbers, this file.
async function compareInputs(name: string, at: string): Promise<string> {
  const git = async (...a: string[]) => {
    const p = Bun.spawn(["git", "-C", import.meta.dir, ...a], { stdout: "pipe", stderr: "ignore" })
    const out = new Uint8Array(await new Response(p.stdout).arrayBuffer())
    return (await p.exited) === 0 ? out : null
  }
  const file = async (rel: string) => at ? await git("show", `${at}:${rel}`) : existsSync(join(import.meta.dir, "../..", rel)) ? new Uint8Array(readFileSync(join(import.meta.dir, "../..", rel))) : null
  const golden = await file(`${GOLDEN_DIR}/${name}.png`), self = await file("coax/web/goldens.ts"), table = await file("apps/openagents-web/realms.json")
  let page: unknown = BY_ADDRESS[name] ?? (name === "landing" || name === "galaxy" ? PAGES[name] : null), listed: unknown = null
  if (table && !page) {
    const w = (JSON.parse(new TextDecoder().decode(table)).worlds as World[]).find((w) => w.kind === "open" && w.realm?.slug === name)
    page = w ? { path: `/realms/${name}`, ...w.realm!.golden } : null; listed = w?.listed ?? null
  }
  return sha(JSON.stringify([golden ? sha(golden) : null, self ? sha(self) : null, page, listed, BLOCK, SHADE]))
}
// name → why it is skipped; a page not here is walked in full.
const unchanged = new Map<Name, string>(), compared = new Map<Name, string>()
if (reference && since && !full && !update && !walkOnly && !root) {
  await Promise.all(Object.keys(PAGES).map(async (name) => {
    const path = PAGES[name].path
    const [mine, theirs] = await Promise.all([fetch(base + path).then((r) => r.ok ? r.text() : null).catch(() => null), fetch(reference + path).then((r) => r.ok ? r.text() : null).catch(() => null)])
    if (mine === null || mine !== theirs) return compared.set(name, mine === null ? "its page didn't answer" : "its HTML changed")
    const changed = await differs(named(mine))
    if (changed) return compared.set(name, `${changed} changed`)
    if (await compareInputs(name, since) !== await compareInputs(name, "")) return compared.set(name, `its golden, its numbers or goldens.ts changed since ${since}`)
    unchanged.set(name, `its HTML and the ${named(mine).size} files it names are byte-identical to ${reference}, and its golden is unchanged since ${since}`)
  }))
}
const skipping = (name: Name) => unchanged.has(name)

// ---- Chrome: a small pool of tabs (GOLDENS_POOL, default 4) in one headless
// Chrome, or one Chrome each (GOLDENS_CHROMES=1). Each tab keeps its own
// errors, requests and analytics, so each page's checks are its own.
const POOL = Math.max(1, Number(process.env.GOLDENS_POOL ?? 4)), CHROMES = process.env.GOLDENS_CHROMES === "1"
const failed: string[] = []
const procs: { proc: ReturnType<typeof Bun.spawn>; profile: string }[] = []
// Chrome under load can take a while to say its port: wait up to 30 s, and start it once more if it doesn't.
async function launch(): Promise<string> {
  for (let attempt = 0; ; attempt++) {
    const port = await start()
    if (port) return port
    if (attempt) throw new Error("headless Chrome didn't start (no DevToolsActivePort in 30 s, twice)")
  }
}
async function start() {
  const profile = mkdtempSync(join(tmpdir(), "coax-goldens-"))
  const proc = launchChrome(profile, [`--window-size=${W},${H}`])
  procs.push({ proc, profile })
  const portFile = join(profile, "DevToolsActivePort")
  const port = () => existsSync(portFile) ? readFileSync(portFile, "utf8").split("\n")[0].trim() : ""
  for (let i = 0; i < 600 && !/^\d+$/.test(port()); i++) await Bun.sleep(50)
  if (/^\d+$/.test(port())) return port()
  proc.kill(); return ""
}

class Tab {
  ws!: WebSocket
  id = 0
  pending = new Map<number, (v: any) => void>()
  errors: string[] = []
  startupNetworkFailures: string[] = []
  inflight = new Set<string>()
  urlOf = new Map<string, string>()
  recent: [number, string][] = []
  quietSince = Date.now()
  sent = new Set<string>()
  requested = new Set<string>()
  lines: string[] = []
  static async open(url: string) {
    const t = new Tab()
    t.ws = new WebSocket(url)
    await new Promise((r) => t.ws.addEventListener("open", r, { once: true }))
    t.ws.addEventListener("message", (e) => t.event(JSON.parse(String(e.data))))
    await t.send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false })
    // every tab acts as the focused one, as the walk's single tab was
    await t.send("Emulation.setFocusEmulationEnabled", { enabled: true })
    await t.send("Runtime.enable"); await t.send("Network.enable")
    await t.send("Page.enable")
    // golden mode on every page of the walk: particles, flicker and other effects hold still (host.js coaxial_golden)
    await t.send("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialGolden = true" })
    if (!root) await t.send("Page.addScriptToEvaluateOnNewDocument", { source: "Object.defineProperty(Navigator.prototype, 'sendBeacon', { value: () => false })" })
    return t
  }
  event(m: any) {
    if (m.id) { this.pending.get(m.id)?.(m); this.pending.delete(m.id); return }
    const p = m.params
    if (m.method === "Runtime.exceptionThrown") this.errors.push(p.exceptionDetails?.exception?.description?.split("\n")[0] ?? p.exceptionDetails?.text)
    if (m.method === "Runtime.consoleAPICalled" && p.type === "error") this.errors.push(p.args.map((a: any) => a.value ?? a.description).join(" ").slice(0, 200))
    if (m.method === "Network.requestWillBeSent") { this.requested.add(p.request.url); this.urlOf.set(p.requestId, `${p.type ?? ""} ${p.request.url}`); this.recent.push([Date.now(), `${p.type ?? ""} ${p.request.url}`]) }
    // "its files arrive": analytics batches and the presence heartbeat (polled every second or so) aren't files
    if (m.method === "Network.requestWillBeSent" && !p.request.url.includes("/api/analytics") && !p.request.url.includes("/api/playtest/presence/")) this.inflight.add(p.requestId)
    if (m.method === "Network.requestWillBeSent" && p.request.url.includes("/api/analytics/events") && p.request.method === "POST") {
      const take = (body: string) => { try { for (const e of JSON.parse(body).events ?? []) this.sent.add(`${e.page}/${e.ev}`) } catch {} }
      if (p.request.postData) take(p.request.postData)
      else if (p.request.hasPostData) this.send("Network.getRequestPostData", { requestId: p.requestId }).then((r) => r.result?.postData && take(r.result.postData))
    }
    if (m.method === "Network.loadingFailed" && !p.canceled) this.startupNetworkFailures.push(p.errorText ?? "network failure")
    if (m.method === "Network.responseReceived" && p.response.status >= 500) this.startupNetworkFailures.push(`HTTP ${p.response.status}`)
    if (m.method === "Network.loadingFinished" || m.method === "Network.loadingFailed") { if (this.inflight.delete(p.requestId) && !this.inflight.size) this.quietSince = Date.now() }
  }
  send(method: string, params: any = {}) { return new Promise<any>((r) => { this.pending.set(++this.id, r); this.ws.send(JSON.stringify({ id: this.id, method, params })) }) }
  async evaluate(expression: string) { return (await this.send("Runtime.evaluate", { expression, returnByValue: true })).result?.result?.value }
  async shot() { return Buffer.from((await this.send("Page.captureScreenshot", { format: "png" })).result.data, "base64") }
  async until(test: () => Promise<boolean>, ms: number) { const end = Date.now() + ms; while (Date.now() < end) { if (await test()) return true; await Bun.sleep(200) } return false }
  check(ok: boolean, what: string) { this.lines.push((ok ? "  ok    " : "  FAIL  ") + what); if (!ok) failed.push(what) }
  log(line: string) { this.lines.push(line) }
  async key(key: string, code: string, vk: number) {
    for (const type of ["keyDown", "keyUp"]) await this.send("Input.dispatchKeyEvent", { type, key, code, windowsVirtualKeyCode: vk })
  }
  // The same-origin paths this document fetched (the browser's own list, cache hits too) and the tab asked for.
  async fetched() {
    const urls: string[] = (await this.evaluate("performance.getEntriesByType('resource').map((e) => e.name).concat([location.href])")) ?? []
    const paths = new Set<string>()
    for (const u of [...urls, ...this.requested]) { try { const x = new URL(u); if (x.origin === new URL(base).origin) paths.add(x.pathname) } catch {} }
    return paths
  }

  // A page reached: it starts, its files arrive, it settles, and it draws.
  // cheap: a page unchanged since production's walk, opened by its address: no golden comparison.
  async arrive(name: Name, cheap = false) {
    const a0 = Date.now(), r = await this.reach(name, cheap)
    this.log(`  time  ${name}: ${((Date.now() - a0) / 1000).toFixed(1)} s`)
    return r
  }
  async reach(name: Name, cheap: boolean) {
    const page = PAGES[name], want = base + root + page.path
    const there = await this.until(async () => (await this.evaluate("location.href")).split(/[?#]/)[0] === want.split(/[?#]/)[0], 15000)
    this.check(there, `${name}: at ${want}`)
    if (!there) return null
    const ready = await this.until(async () => (await this.evaluate("globalThis.coaxial?.ready === true")) === true, 20000)
    this.check(ready, `${name}: the engine started`)
    const quiet = await this.until(async () => !this.inflight.size && Date.now() - this.quietSince > 1500, 30000)
    if (!quiet) this.log(`  info  ${name}: still loading after 30 s: ${[...this.inflight].map((r) => this.urlOf.get(r)).join(", ") || "nothing in flight"}; asked in the last 5 s: ${[...new Set(this.recent.filter(([t]) => t > Date.now() - 5000).map(([, u]) => u))].slice(0, 6).join(", ") || "nothing"}`)
    // Engine readiness precedes GPU setup and asset delivery. The house must
    // submit a nonempty indexed scene frame before its unchanged pixel check.
    if (name === "haunted-house") {
      const drawn = await this.until(async () => (await this.evaluate("globalThis.coaxial?.sceneReady === true")) === true, 20000)
      this.check(drawn, `${name}: a scene frame completed`)
    }
    await Bun.sleep(page.settle)
    let bytes = await this.shot(), image = png(bytes), share = lit(image)
    // A dark page (the fixture: a night sky, a lamp and its HUD, under 2% lit) can be caught on a
    // frame the compositor hasn't filled yet, all black; it gets a few more frames to show itself
    // before it counts as drawing nothing (2026-10-29: fixture "0.0% lit" on a staging walk while
    // the same shot matched its golden).
    for (let again = 0; share < page.lit && again < 4; again++) {
      this.log(`  info  ${name}: ${(share * 100).toFixed(1)}% lit, under ${(page.lit * 100).toFixed(1)}%; shooting it again`)
      await Bun.sleep(750)
      bytes = await this.shot(); image = png(bytes); share = lit(image)
    }
    this.check(this.errors.length === 0, `${name}: no errors${this.errors.length ? ` (${this.errors.join("; ")})` : ""}`)
    this.errors = []
    this.check(share >= page.lit, `${name}: draws (${(share * 100).toFixed(1)}% lit, at least ${(page.lit * 100).toFixed(1)}%)`)
    if (walkOnly || !wanted(name)) return image
    if (skipping(name)) {
      // the files it fetched once started must be the same bytes too, or it is compared after all
      const changed = await differs(await this.fetched())
      if (!changed) { this.log(`  skip  ${name}: golden comparison skipped: ${unchanged.get(name)}, and so are the files it fetched`); return image }
      this.log(`  info  ${name}: ${changed} differs from ${reference}; compared with its golden after all`)
      unchanged.delete(name); compared.set(name, `${changed} changed`)
      if (cheap) return "again" as const
    }
    const golden = join(goldens, `${name}.png`)
    if (update) { mkdirSync(goldens, { recursive: true }); writeFileSync(golden, bytes); this.log(`  new   ${golden}`); return image }
    if (!existsSync(golden)) {
      const m = missingGolden(name, unlisted.has(name), pendingDir, base)
      if (m.fail) { this.check(false, m.line); return image }
      mkdirSync(pendingDir, { recursive: true }); writeFileSync(m.pending, bytes)
      this.log(`  ${m.line}`)
      return image
    }
    // more shots: what moves on its own (candles, orbits) isn't compared
    const shots = [image]
    for (let i = 0; i < 3; i++) { await Bun.sleep(500); shots.push(png(await this.shot())) }
    const d = against(shots, png(readFileSync(golden)))
    const ok = d.changed <= page.blocks
    let kept = ""
    if (!ok) { kept = join(mkdtempSync(join(tmpdir(), "coax-golden-")), `${name}.png`); writeFileSync(kept, bytes) }
    this.check(ok, `${name}: matches its golden (${d.changed} blocks changed, ${page.blocks} allowed, ${d.moved} moving${d.box ? `, ${d.box}` : ""}${kept ? `; this shot: ${kept}` : ""})`)
    return image
  }
  // A realm named in the contract (by its slug) sends its events too, world-leave on pagehide.
  async realmContract(slug: string) {
    const t0 = Date.now(), want = contract[slug].map((ev) => `${slug}/${ev}`)
    await this.until(async () => want.every((w) => this.sent.has(w) || w.endsWith("/world-leave")), 5000)
    await this.evaluate("dispatchEvent(new Event('pagehide'))")
    await this.until(async () => want.every((w) => this.sent.has(w)), 4000)
    const missing = want.filter((w) => !this.sent.has(w))
    this.check(!missing.length, `analytics contract: every ${slug} event sent (${missing.length ? "missing " + missing.join(", ") : want.length + " events"}, ${((Date.now() - t0) / 1000).toFixed(1)} s)`)
  }
}

// ---- The walk, in lanes: each lane runs in one tab, the lanes side by side.
type Lane = { name: string; run: (t: Tab) => Promise<void> }
const lanes: Lane[] = []
// The journey: the landing, ENTER into the house, the galaxy, a click on the
// Haunted House, and the house's analytics contract (its own lane, first).
const journey = ["landing", "galaxy", "haunted-house"].some(wanted)
if (journey) lanes.push({ name: "journey", run: async (t) => {
  await t.send("Page.navigate", { url: base + root + PAGES.landing.path })
  if (!(await t.arrive("landing"))) return
  await t.key("Enter", "Enter", 13)   // ENTER: play now as a guest
  // Current landing opens the house directly. Pinned demos preserve their
  // historical landing-to-galaxy route; check the current direct route
  // before separately exercising the galaxy's house selector below.
  if (!root) {
    await t.arrive("haunted-house")
    await t.send("Page.navigate", { url: `${base}/verse/galaxy` })
  }
  const galaxy = await t.arrive("galaxy")
  if (!galaxy) return
  // The Haunted House's tag (orange words under its planet) moves with the
  // orbit: click the middle of the row with the most orange in it.
  const fresh = png(await t.shot()), rows = new Array(fresh.h).fill(0), xs: number[][] = Array.from({ length: fresh.h }, () => [])
  for (let y = 0; y < fresh.h; y++) for (let x = 0; x < fresh.w; x++) {
    const i = (y * fresh.w + x) * 4, r = fresh.px[i], g = fresh.px[i + 1], b = fresh.px[i + 2]
    if (r > 170 && g > 80 && g < 190 && b < 100 && r - g > 50) { rows[y]++; xs[y].push(x) }
  }
  const y = rows.indexOf(Math.max(...rows)), x = xs[y].sort((a, b) => a - b)[xs[y].length >> 1]
  t.check(rows[y] > 20, `galaxy: the Haunted House's tag is drawn (${rows[y]} orange pixels at y ${y})`)
  if (rows[y] <= 20) return
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
    await t.send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
  if (!(await t.arrive("haunted-house")) || root) return
  const t0 = Date.now()
  const want = Object.entries(contract).filter(([page]) => !open.some((w) => w.realm!.slug === page)).flatMap(([page, evs]) => evs.map((ev) => `${page}/${ev}`))
  const seen = (leave: boolean) => want.every((w) => t.sent.has(w) || (!leave && w.endsWith("/world-leave")))
  for (const type of ["keyDown", "keyUp"]) {
    if (type === "keyUp") await t.until(async () => t.sent.has("haunt/quest"), 8000)
    await t.send("Input.dispatchKeyEvent", { type, key: "Shift", code: "ShiftLeft", windowsVirtualKeyCode: 16, modifiers: type === "keyDown" ? 8 : 0 })
    await t.send("Input.dispatchKeyEvent", { type, key: "w", code: "KeyW", windowsVirtualKeyCode: 87, modifiers: type === "keyDown" ? 8 : 0 })
  }
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
    await t.send("Input.dispatchMouseEvent", { type, x: W / 2, y: H / 3, button: "left", clickCount: 1 })
  await t.until(async () => seen(false), 3000)
  await t.evaluate("dispatchEvent(new Event('pagehide'))")
  await t.until(async () => seen(true), 4000)
  const missing = want.filter((w) => !t.sent.has(w))
  t.check(!missing.length, `analytics contract: every contract event sent (${missing.length ? "missing " + missing.join(", ") : want.length + " events"}, ${((Date.now() - t0) / 1000).toFixed(1)} s)`)
} })
// Every other realm, opened from the galaxy with the keyboard (a pinned demo has only the house).
const fromGalaxy = async (t: Tab, slug: string, k: number) => {
  t.errors = []; t.startupNetworkFailures = []
  await t.send("Page.navigate", { url: `${base}/verse/galaxy?realms=all` })
  const ready = await t.until(async () => (await t.evaluate("location.pathname")) === "/verse/galaxy" && (await t.evaluate("globalThis.coaxial?.ready === true")) === true, 20000)
  t.check(ready, `${slug}: the galaxy started, to open it from`)
  if (!ready) {
    const state = await t.evaluate("({path:location.pathname,document:document.readyState,host:typeof globalThis.coaxial,ready:globalThis.coaxial?.ready === true})")
    t.log(`  info   ${slug}: startup failure ${JSON.stringify({ state, errors: t.errors.slice(-8), network: t.startupNetworkFailures.slice(-8), inflight: t.inflight.size })}`)
    return
  }
  await Bun.sleep(1000)
  t.errors = []
  for (let i = 0; i <= k; i++) await t.key("ArrowRight", "ArrowRight", 39)
  await t.key("Enter", "Enter", 13)
  if (!(await t.arrive(slug)) || !contract[slug]) return
  await t.realmContract(slug)
}
// A realm unchanged since production's walk: opened by its address, it must start and draw.
const byAddress = async (t: Tab, slug: string, k: number) => {
  t.errors = []; t.startupNetworkFailures = []
  await t.send("Page.navigate", { url: base + PAGES[slug].path })
  const r = await t.arrive(slug, true)
  if (r === "again") return fromGalaxy(t, slug, k)
  if (r && contract[slug]) await t.realmContract(slug)
}
if (!root) for (const [k, w] of open.entries()) {
  const slug = w.realm!.slug
  if (slug === "haunted-house" || !wanted(slug)) continue
  lanes.push({ name: slug, run: (t) => skipping(slug) ? byAddress(t, slug, k) : fromGalaxy(t, slug, k) })
}
// Pages of their own, by their address.
if (!root) for (const name of Object.keys(BY_ADDRESS)) {
  if (open.some((w) => w.realm!.slug === name) || !wanted(name)) continue
  const size = PAGES[name].size
  lanes.push({ name, run: async (t) => {
    // a page at its own size (a phone's): the tab takes it for this page and gives it back after
    if (size) await t.send("Emulation.setDeviceMetricsOverride", { width: size[0], height: size[1], deviceScaleFactor: 1, mobile: false })
    try {
      await t.send("Page.navigate", { url: base + PAGES[name].path })
      t.errors = []
      await t.arrive(name)
    } finally {
      if (size) await t.send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false })
    }
  } })
}

// --starts (release.sh's brief production check): every page, by its address, answers and starts.
if (startsOnly) lanes.splice(0, lanes.length, ...Object.keys(PAGES).filter((n) => !root || n !== "garden").map((name) => ({ name, run: async (t: Tab) => {
  const want = base + root + PAGES[name].path
  t.errors = []
  const answer = await fetch(want).then((r) => r.status).catch(() => 0)
  t.check(answer === 200, `${name}: ${want} answers (${answer || "no answer"})`)
  await t.send("Page.navigate", { url: want })
  const ready = await t.until(async () => (await t.evaluate("location.href"))?.split(/[?#]/)[0] === want.split(/[?#]/)[0] && (await t.evaluate("globalThis.coaxial?.ready === true")) === true, 20000)
  t.check(ready, `${name}: the engine started`)
  t.check(t.errors.length === 0, `${name}: no errors${t.errors.length ? ` (${t.errors.join("; ")})` : ""}`)
} })))
const allSent = new Set<string>()
const t0 = Date.now()
try {
  const ports: string[] = []
  const tabs: Tab[] = []
  const workers = Math.min(POOL, lanes.length)
  if (CHROMES) ports.push(...await Promise.all(Array.from({ length: workers }, () => launch())))
  else ports.push(await launch())
  for (let i = 0; i < workers; i++) {
    const port = ports[CHROMES ? i : 0]
    const target = CHROMES || i === 0
      ? (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
      : await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" })).json()
    tabs.push(await Tab.open(target.webSocketDebuggerUrl))
  }
  let next = 0
  await Promise.all(tabs.map(async (t) => {
    while (next < lanes.length) {
      const lane = lanes[next++], l0 = Date.now()
      t.sent = new Set(); t.requested = new Set(); t.lines = []
      try { await lane.run(t) } catch (e) { t.check(false, `${lane.name}: the walk threw (${e})`) }
      for (const s of t.sent) allSent.add(s)
      console.log(`lane ${lane.name} (${((Date.now() - l0) / 1000).toFixed(1)} s)\n${t.lines.join("\n")}`)
    }
    t.ws.close()
  }))
  if (!root && !startsOnly && journey && !allSent.size) { console.log("  FAIL  analytics contract: no analytics batch was sent"); failed.push("analytics contract: no analytics batch was sent") }
} finally {
  for (const { proc, profile } of procs) { proc.kill(); rmSync(profile, { recursive: true, force: true }) }
}
const skipped = [...unchanged.keys()]
if (reference && since) console.log(skipped.length
  ? `goldens: golden comparison skipped for ${skipped.join(", ")} (unchanged since ${since}, which went live after its walk); GOLDENS_FULL=1 compares every page`
  : `goldens: every page compared with its golden (none unchanged since ${since})`)
if (reference && since && compared.size) console.log(`goldens: compared in full: ${[...compared].map(([n, why]) => `${n} (${why})`).join("; ")}`)
console.log(`goldens ${base}${root}: ` + (failed.length ? `${failed.length} failed` : update ? "updated" : "passed") + ` in ${((Date.now() - t0) / 1000).toFixed(1)} s (${Math.min(POOL, lanes.length)} ${CHROMES ? "Chromes" : "tabs"})`)
// --report FILE: the failures as JSON for release.sh's hold-back. golden: the
// pages whose only failures are their golden comparisons (each named with its
// path); other: every other failure (a page that doesn't start, errors, draws
// nothing, a missing analytics event), which a release never ships past.
const report = value("report")
if (report) {
  writeFileSync(report, JSON.stringify({ base, failed, ...classify(failed, PAGES) }, null, 2) + "\n")
}
process.exit(failed.length ? 1 : 0)
