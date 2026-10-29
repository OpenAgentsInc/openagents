// The Sandbox's set pieces (rl-sandbox-pieces), played on a live host in
// headless Chrome (no window opens, no sound): a guest opens /realms/sandbox,
// presses B and P, types "a flock of birds follows me" and MAKE; the piece
// comes from the real selector, is published to the shared Sandbox at once
// (its log names it), and Z takes it back in one step, which DONE publishes.
// Requests carry the automation mark sandbox-pieces-flow.
//   bun coax/web/api/sandbox-pieces-flow.ts ORIGIN [SHOTS_DIR]
// PIECE_SENTENCE types another sentence (any piece but none).
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const origin = process.argv[2]
const shots = process.argv[3] ?? ""
const sentence = process.env.PIECE_SENTENCE ?? "a flock of birds follows me"
if (!origin) { console.error("usage: bun coax/web/api/sandbox-pieces-flow.ts ORIGIN [SHOTS_DIR]"); process.exit(2) }
const H = { "Content-Type": "application/json", "X-Verse-Automation": "sandbox-pieces-flow" }
const api = async (method: string, path: string, body?: any) => {
  const r = await fetch(origin + "/api/playtest/" + path, { method, headers: H, body: body === undefined ? undefined : JSON.stringify(body) })
  return { status: r.status, body: await r.json() }
}
let failed = 0
const check = (name: string, ok: boolean, detail?: any) => {
  console.log((ok ? "  ok    " : "  FAIL  ") + name + (ok ? "" : "  " + JSON.stringify(detail ?? "").slice(0, 400)))
  if (!ok) failed++
}

const W = 1440, Hh = 900
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "pieces-flow-"))
const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars",
  "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal", `--window-size=${W},${Hh}`,
  "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
try {
  const portFile = join(profile, "DevToolsActivePort")
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await Bun.sleep(50)
  const port = readFileSync(portFile, "utf8").split("\n")[0]
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise(r => ws.addEventListener("open", r, { once: true }))
  let id = 0
  const pending = new Map<number, (v: any) => void>()
  ws.addEventListener("message", e => { const m = JSON.parse(String(e.data)); pending.get(m.id)?.(m); pending.delete(m.id) })
  const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: Hh, deviceScaleFactor: 1, mobile: false })
  const js = async (expression: string) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.result?.value
  const key = async (k: string, wait = 250) => {
    const named = k === "Enter" ? { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 } : { key: k, text: k }
    await send("Input.dispatchKeyEvent", { type: "keyDown", ...named })
    await Bun.sleep(40)
    await send("Input.dispatchKeyEvent", { type: "keyUp", ...named })
    await Bun.sleep(wait)
  }
  const shot = async (name: string) => {
    if (!shots) return
    const s = await send("Page.captureScreenshot", { format: "png" })
    writeFileSync(join(shots, name + ".png"), Buffer.from(s.result.data, "base64"))
  }
  const before = (await api("GET", "log/sandbox")).body
  await send("Page.navigate", { url: origin + "/404.html" })
  await Bun.sleep(800)
  await js(`document.cookie = "verse-automation=sandbox-pieces-flow; path=/"; 1`)
  await send("Page.navigate", { url: origin + "/realms/sandbox" })
  await Bun.sleep(10000)
  await shot("1-sandbox")
  await key("b", 600); await key("p", 400)
  for (const c of sentence) await key(c, 30)
  await shot("2-sentence")
  await key("Enter", 9000)
  await shot("3-flock")
  let log = (await api("GET", "log/sandbox")).body
  check("the sentence makes its piece, published to the Sandbox at once",
        log.length === before.length + 1 && (process.env.PIECE_SENTENCE ? /^Made an? .+ in the Sandbox$/ : /^Made a flock of \d+ birds in the Sandbox$/).test(log[0]?.summary ?? ""), log.slice(0, 2))
  const made = (await api("GET", "world/sandbox")).body
  await key("z", 600)
  await shot("4-undone")
  await key("b", 4000)
  log = (await api("GET", "log/sandbox")).body
  const after = (await api("GET", "world/sandbox")).body
  check("Z takes the whole piece back in one step, and DONE publishes that", log.length === before.length + 2 && after.version === made.version + 1, log.slice(0, 2))
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(failed ? `sandbox pieces flow: ${failed} failed` : "sandbox pieces flow: passed")
process.exit(failed ? 1 : 0)
