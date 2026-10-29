// Plays Ice Cream Man in headless Chrome (WebGPU, no window) and saves the
// trace the page hands out (action 30, a "coaxial" event), so
// verse/icecream/test-browser.sh can replay the browser's inputs on the
// native rules and check every step's fingerprint.
//   bun verse/icecream/tools/browser-trace.ts URL OUT.trace [WxH] [--keys]
// URL carries the options (#seed=4&bot&trace=600 plays the bot for 600
// steps; #seed=4&trace=300 with --keys walks, turns, fires and jumps with
// real key events instead).
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const [url, out, size = "1280x800"] = process.argv.slice(2).filter(a => !a.startsWith("--"))
const keys = process.argv.includes("--keys")
if (!url || !out) { console.error("usage: bun browser-trace.ts URL OUT.trace [WxH] [--keys]"); process.exit(2) }
const [w, h] = size.split("x").map(Number)
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "icm-trace-"))
const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run",
  "--hide-scrollbars", "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal",
  `--window-size=${w},${h}`, "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
let code = 1
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
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: false })
  await send("Page.enable")
  await send("Page.addScriptToEvaluateOnNewDocument", { source:
    `addEventListener("coaxial", (e) => { if (e.detail && e.detail.code === 30) window.__trace = e.detail.text; });` })
  await send("Page.navigate", { url })
  const key = async (type: string, k: string, code: string) =>
    send("Input.dispatchKeyEvent", { type, key: k, code, windowsVirtualKeyCode: k.length === 1 ? k.toUpperCase().charCodeAt(0) : 0 })
  const hold = async (k: string, code: string, ms: number) => { await key("keyDown", k, code); await Bun.sleep(ms); await key("keyUp", k, code) }
  await Bun.sleep(3000)
  if (keys) {
    await hold("w", "KeyW", 1500)
    await hold("d", "KeyD", 600)
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x: w / 2, y: h / 2, button: "left", clickCount: 1 })
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: w / 2, y: h / 2, button: "left", clickCount: 1 })
    await hold(" ", "Space", 200)
    await hold("2", "Digit2", 100)
    await hold("a", "KeyA", 800)
    await hold("s", "KeyS", 700)
  }
  let text = ""
  for (let i = 0; i < 600 && !text; i++) {
    const r = await send("Runtime.evaluate", { expression: "window.__trace || ''", returnByValue: true })
    text = r.result?.result?.value ?? ""
    if (!text) await Bun.sleep(250)
  }
  if (!text) { console.error("no trace came from the page"); }
  else { writeFileSync(out, text); console.log(`${out}: ${text.split("\n").length} lines`); code = 0 }
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
process.exit(code)
