// Everglade's frame rate in headless Chrome: the check that keeps it at 60 FPS.
//   bun coax/web/perf.ts [BASE_URL] [--dpr 1|2] [--seconds S] [--json]
// BASE_URL defaults to https://staging.openagents.com. For each device pixel
// ratio (1 and 2, at 1440x900, two Chromes side by side) it opens
// /realms/everglade at four places and times of day (the road at noon, the
// commons at dusk, the river at night, the market at dusk), waits for the
// scene to draw and settle, then walks for about S seconds (default 13),
// casting a meteor every 6.7 s, and times every requestAnimationFrame. A case
// fails when its 95th percentile frame is over 17.5 ms or any frame is over
// 33 ms.
//
// Headless Chrome on a shared Mac sees other work on the GPU. A model
// deciding on Metal (psionic-serve --elephant-metal) keeps the GPU busy for
// seconds and drops Everglade to 10-30 FPS (measured 2026-10-29: with the GPU
// free, Everglade holds 16.7 ms frames at dpr 1 and 2 with ~1 ms of CPU a
// frame; while Elephant decides, frames take 33-117 ms, and drawing fewer
// pixels doesn't help). So a failing case is tried again, up to three times,
// and a control page (production's Everglade, standing still in its own
// Chrome) is timed over the same seconds: when it misses frames too, the
// machine is busy, not the page, and the check waits 10 s and tries again (at
// most 6 waits). The run fails when Everglade misses frames while the control
// doesn't, or when the GPU never comes free; the output says which.
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const args = process.argv.slice(2)
const flag = (name: string) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined }
const base = (args.find((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--")) ?? "https://staging.openagents.com").replace(/\/$/, "")
// one round of the walk is 6.71 s; S seconds is the nearest whole number of rounds (at least one)
const ROUND = 6710, rounds = Math.max(1, Math.round(Number(flag("--seconds") ?? 13) * 1000 / ROUND)), seconds = rounds * ROUND / 1000
const dprs = flag("--dpr") ? [Number(flag("--dpr"))] : [1, 2]
const asJson = args.includes("--json")
const W = 1440, H = 900, P95_MAX = 17.5, FRAME_MAX = 33, ATTEMPTS = 3, WAITS = 6
const SCENES = [
  { name: "road, noon", path: "/realms/everglade?clock=0.5#road" },
  { name: "commons, dusk", path: "/realms/everglade?clock=0.75#commons" },
  { name: "river, night", path: "/realms/everglade?clock=0#river" },
  { name: "market, dusk", path: "/realms/everglade?clock=0.75#market" },
]
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

type Frames = { n: number; fps: number; p50: number; p95: number; max: number; over: number }
const stats = (d: number[]): Frames => {
  const s = [...d].sort((a, b) => a - b), at = (p: number) => s[Math.min(s.length - 1, Math.floor(p * s.length))] ?? 0
  const total = d.reduce((a, b) => a + b, 0)
  return { n: d.length, fps: total ? Math.round(d.length / total * 10000) / 10 : 0, p50: at(0.5), p95: at(0.95), max: s[s.length - 1] ?? 0, over: d.filter((x) => x > 20).length }
}
const passes = (f: Frames) => f.n > 30 && f.p95 <= P95_MAX && f.max <= FRAME_MAX
const show = (f: Frames) => `${f.fps} FPS, p50 ${f.p50.toFixed(1)} ms, p95 ${f.p95.toFixed(1)} ms, max ${f.max.toFixed(1)} ms, ${f.over}/${f.n} over 20 ms`

// The walk, run in the page: forward, a turn, a meteor cast ahead, away again.
const WALK = (n: number) => `(async () => {
  const sleep = (t) => new Promise((r) => setTimeout(r, t));
  const key = (k, down) => dispatchEvent(new KeyboardEvent(down ? "keydown" : "keyup", { key: k, bubbles: true }));
  const ptr = (type, x, y) => dispatchEvent(new PointerEvent(type, { clientX: x, clientY: y, pointerType: "mouse", pointerId: 1, bubbles: true, button: 0 }));
  const d = []; let last = 0, stop = false;
  const tick = (t) => { if (last) d.push(t - last); last = t; if (!stop) requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
  const t0 = performance.now(), W = innerWidth, H = innerHeight;
  for (let round = 0; round < ${n}; round++) {
    key("w", true); await sleep(1800);
    key("d", true); await sleep(500); key("d", false); key("w", false);
    key("1", true); key("1", false); await sleep(250);
    ptr("pointermove", W / 2, H * 0.6); ptr("pointerdown", W / 2, H * 0.6); await sleep(60); ptr("pointerup", W / 2, H * 0.6);
    await sleep(3200);
    key("Escape", true); key("Escape", false);
    key("a", true); key("s", true); await sleep(900); key("a", false); key("s", false);
  }
  stop = true; await sleep(20);
  return d;
})()`
// The control: production's Everglade (PERF_CONTROL names another page), standing at the market in
// its own Chrome while the page under test walks. A trivial WebGPU page holds 60 FPS through a model
// deciding on the GPU when Everglade doesn't (it waits behind the model at each of its nine passes),
// so the control is the same kind of page.
const CONTROL = process.env.PERF_CONTROL ?? "https://openagents.com/realms/everglade?clock=0.5#market"
const TIMES = (ms: number) => `new Promise((ok) => { const d = []; let last = 0; const t0 = performance.now(); const f = (t) => { if (last) d.push(t - last); last = t; if (t - t0 < ${ms}) requestAnimationFrame(f); else ok(d); }; requestAnimationFrame(f); })`

class Browser {
  proc!: ReturnType<typeof Bun.spawn>
  profile = mkdtempSync(join(tmpdir(), "coax-perf-"))
  ws!: WebSocket
  id = 0
  pending = new Map<number, (v: any) => void>()
  constructor(public dpr: number) {}
  async start() {
    this.proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${this.profile}`, "--no-first-run", "--hide-scrollbars",
      "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal", `--window-size=${W},${H}`,
      "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
    const portFile = join(this.profile, "DevToolsActivePort")
    const port = () => existsSync(portFile) ? readFileSync(portFile, "utf8").split("\n")[0].trim() : ""
    for (let i = 0; i < 600 && !/^\d+$/.test(port()); i++) await Bun.sleep(50)
    if (!/^\d+$/.test(port())) throw new Error("headless Chrome didn't start")
    const target = (await (await fetch(`http://127.0.0.1:${port()}/json/list`)).json()).find((t: any) => t.type === "page")
    this.ws = new WebSocket(target.webSocketDebuggerUrl)
    await new Promise((r) => this.ws.addEventListener("open", r, { once: true }))
    this.ws.addEventListener("message", (e) => { const m = JSON.parse(String(e.data)); this.pending.get(m.id)?.(m); this.pending.delete(m.id) })
    await this.send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: this.dpr, mobile: false })
  }
  send(method: string, params: any = {}) { return new Promise<any>((r) => { this.pending.set(++this.id, r); this.ws.send(JSON.stringify({ id: this.id, method, params })) }) }
  async evaluate(expression: string) {
    const m = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })
    if (m.result?.exceptionDetails) throw new Error(m.result.exceptionDetails.exception?.description ?? "evaluation failed")
    return m.result?.result?.value
  }
  async open(url: string) {
    await this.send("Page.navigate", { url: "about:blank" })   // a retry of the same address loads it again, not just its hash
    await this.send("Page.navigate", { url })
    await Bun.sleep(300)
  }
  // open a realm and wait until its scene has drawn and settled
  async ready(url: string) {
    await this.open(url)
    const t0 = Date.now()
    while (Date.now() - t0 < 30000 && (await this.evaluate("globalThis.coaxial?.sceneReady === true").catch(() => false)) !== true) await Bun.sleep(200)
    if (Date.now() - t0 >= 30000) throw new Error(`${url}: the scene didn't draw in 30 s`)
    await Bun.sleep(2000)   // the first frames after the scene draws (pipelines warming, textures arriving) settle
  }
  async scene(path: string): Promise<[Frames, Frames]> {
    await this.ready(base + path)
    // the control's frames over the same seconds
    const [walk, control] = await Promise.all([this.evaluate(WALK(rounds)), controlBrowser.evaluate(TIMES(rounds * ROUND))])
    return [stats(walk), stats(control)]
  }
  close() { try { this.ws.close() } catch {} this.proc?.kill(); rmSync(this.profile, { recursive: true, force: true }) }
}

type Result = { dpr: number; scene: string; frames: Frames; attempts: number; busy: number; ok: boolean; why: string }
async function run(dpr: number): Promise<Result[]> {
  const b = new Browser(dpr)
  const out: Result[] = []
  try {
    await b.start()
    for (const s of SCENES) {
      let [frames, control] = await b.scene(s.path), attempts = 1, busy = 0, why = ""
      while (!passes(frames)) {
        if (!passes(control)) {
          // the GPU is busy with someone else's work: wait for it, then try again
          why = `the control page missed frames too (${show(control)}): another process keeps the GPU busy`
          if (++busy > WAITS) break
          await Bun.sleep(10000)
        } else {
          why = "the control page held 60 FPS: the page itself misses frames"
          if (attempts >= ATTEMPTS) break
          attempts++
        }
        [frames, control] = await b.scene(s.path)
      }
      const ok = passes(frames)
      out.push({ dpr, scene: s.name, frames, attempts, busy, ok, why: ok ? "" : why })
      if (!asJson) console.log(`${ok ? "ok  " : "FAIL"} dpr ${dpr} ${s.name.padEnd(14)} ${show(frames)}${attempts > 1 || busy ? ` (attempt ${attempts}, ${busy} busy waits)` : ""}${ok ? "" : `\n     ${why}`}`)
    }
  } finally { b.close() }
  return out
}

if (!asJson) console.log(`perf: ${base}/realms/everglade, ${W}x${H}, dpr ${dprs.join(" and ")}, ${seconds} s a scene, p95 <= ${P95_MAX} ms and every frame <= ${FRAME_MAX} ms`)
const controlBrowser = new Browser(1)
let results: Result[]
try {
  await controlBrowser.start()
  await controlBrowser.ready(CONTROL)
  results = (await Promise.all(dprs.map(run))).flat()
} finally { controlBrowser.close() }
const bad = results.filter((r) => !r.ok)
if (asJson) console.log(JSON.stringify(results))
else console.log(bad.length ? `perf: ${bad.length} of ${results.length} cases missed 60 FPS` : `perf: every case held 60 FPS`)
process.exit(bad.length ? 1 : 0)
