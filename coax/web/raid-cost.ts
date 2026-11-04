// What the Cultist Raid preview's pathing costs a frame (#12747), in headless Chrome: Everglade's
// ?raid=preview with two builds of the page's module, one with the raid's pathing and one without
// it (rt_way and rt_slide stubbed to go straight), each timed the same way. Staging serves the page
// and its pack; this machine's coax/web scripts, everglade5e.wgsl and the module under test stand in
// for staging's (Fetch interception), and nothing is sent to the API (every POST is answered 204).
// The main thread's time in coaxial_frame is timed every frame through the raid; the pathing's
// cost is the difference of the two builds' means. It fails over LIMIT ms (default 1).
//   bun coax/web/raid-cost.ts WITH.wasm WITHOUT.wasm [SECONDS] [--rounds N] [--limit MS] [--shot PREFIX]
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { startChrome } from "./chrome"

const args = process.argv.slice(2)
const flag = (name: string, d: string) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : d }
const [withWasm, withoutWasm, secS = "40"] = args.filter((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"))
if (!withWasm || !withoutWasm) { console.error("usage: bun coax/web/raid-cost.ts WITH.wasm WITHOUT.wasm [SECONDS] [--rounds N] [--limit MS]"); process.exit(2) }
const rounds = Number(flag("--rounds", "2")), limit = Number(flag("--limit", "1"))
const repo = join(import.meta.dir, "..", "..")
const wgsl = readFileSync(join(repo, "apps/openagents-web/everglade5e.wgsl"))
const URL_ = "https://staging.openagents.com/worlds/everglade?raid=preview"

// coaxial_frame timed: the host calls it once a frame
const before = `(() => {
  window.__fc = []; window.__timing = false;
  const orig = WebAssembly.instantiate;
  WebAssembly.instantiate = async function (m, imp) {
    const r = await orig.call(this, m, imp);
    const inst = r instanceof WebAssembly.Instance ? r : r.instance;
    const ex = inst.exports;
    if (!ex.coaxial_frame) return r;
    const f = ex.coaxial_frame, nex = Object.assign(Object.create(null), ex);
    nex.coaxial_frame = (...a) => { const t = performance.now(); const v = f(...a); if (window.__timing) window.__fc.push(performance.now() - t); return v; };
    const fake = { exports: nex };
    return r instanceof WebAssembly.Instance ? fake : { module: r.module, instance: fake };
  };
})();`

async function run(wasm: string): Promise<number[]> {
  const bytes = readFileSync(wasm)
  const { proc, port } = await startChrome("coax-raidcost-", ["--window-size=1280,800", "--disable-frame-rate-limit", "--disable-gpu-vsync"])
  try {
    const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
    const ws = new WebSocket(target.webSocketDebuggerUrl)
    await new Promise(r => ws.addEventListener("open", r, { once: true }))
    let id = 0; const pending = new Map<number, (v: any) => void>()
    const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
    ws.addEventListener("message", e => {
      const m = JSON.parse(String(e.data))
      if (m.id) { pending.get(m.id)?.(m); pending.delete(m.id); return }
      if (m.method !== "Fetch.requestPaused") return
      const { requestId, request } = m.params
      const path = new URL(request.url).pathname
      const body = (b: Buffer, type: string) => send("Fetch.fulfillRequest", { requestId, responseCode: 200, responseHeaders: [{ name: "Content-Type", value: type }], body: b.toString("base64") })
      if (request.method !== "GET" && request.method !== "HEAD") send("Fetch.fulfillRequest", { requestId, responseCode: 204 })
      else if (/\/build\/everglade5e\.[0-9a-f]+\.wasm$/.test(path)) body(bytes, "application/wasm")
      else if (/\/build\/everglade5e\.[0-9a-f]+\.wgsl$/.test(path)) body(wgsl, "text/plain")
      else if (path.startsWith("/coaxial/") && path.endsWith(".js")) {
        try { body(readFileSync(join(repo, "coax/web", path.slice("/coaxial/".length))), "text/javascript") } catch { send("Fetch.continueRequest", { requestId }) }
      } else send("Fetch.continueRequest", { requestId })
    })
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false })
    await send("Page.enable"); await send("Runtime.enable")
    await send("Fetch.enable", { patterns: [{ urlPattern: "*" }] })
    await send("Page.addScriptToEvaluateOnNewDocument", { source: before })
    await send("Page.navigate", { url: URL_ })
    // the scene drawn, then the raid under way
    for (let i = 0; i < 120; i++) {
      const r = await send("Runtime.evaluate", { expression: "!!(globalThis.coaxial && globalThis.coaxial.sceneReady)", returnByValue: true })
      if (r.result?.result?.value) break
      await Bun.sleep(500)
    }
    await Bun.sleep(4000)
    await send("Runtime.evaluate", { expression: "window.__fc.length = 0; window.__timing = true" })
    await Bun.sleep(Number(secS) * 1000)
    const r = await send("Runtime.evaluate", { expression: "JSON.stringify(window.__fc)", returnByValue: true })
    const shot = flag("--shot", "")
    if (shot) { const s = await send("Page.captureScreenshot", { format: "png" }); await Bun.write(`${shot}-${wasm.split("/").pop()}.png`, Buffer.from(s.result.data, "base64")) }
    return JSON.parse(r.result.result.value)
  } finally {
    try { process.kill(proc.pid, "SIGKILL") } catch {}
  }
}

const mean = (a: number[]) => a.reduce((x, y) => x + y, 0) / Math.max(1, a.length)
const q = (a: number[], p: number) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(p * s.length))] : NaN }
const all: Record<string, number[]> = { with: [], without: [] }
for (let k = 0; k < rounds; k++) {
  for (const [name, wasm] of [["with", withWasm], ["without", withoutWasm]] as const) {
    const fc = await run(wasm)
    all[name].push(...fc)
    console.log(`round ${k + 1} ${name}: ${fc.length} frames, coaxial_frame mean ${mean(fc).toFixed(3)} ms, p50 ${q(fc, 0.5).toFixed(3)}, p95 ${q(fc, 0.95).toFixed(3)}`)
  }
}
const cost = mean(all.with) - mean(all.without)
console.log(`pathing: ${cost.toFixed(3)} ms a frame (with ${mean(all.with).toFixed(3)}, without ${mean(all.without).toFixed(3)}; limit ${limit} ms)`)
if (all.with.length < 100 || all.without.length < 100) { console.log("could not run: too few frames timed"); process.exit(75) }
process.exit(cost > limit ? 1 : 0)
