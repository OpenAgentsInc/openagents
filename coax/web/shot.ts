// Screenshot a Coaxial page with WebGPU in headless Chrome: no window opens and
// nothing takes focus. Agents never open a visible browser for checks.
//   bun coax/web/shot.ts URL OUT.png [WIDTHxHEIGHT] [WAIT_MS] [--eval JS]
// It waits WAIT_MS of real time (the pages animate in), then captures.
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const [url, out, size = "1440x900", wait = "4000"] = process.argv.slice(2)
const evalAt = process.argv.indexOf("--eval")
const script = evalAt > 0 ? process.argv[evalAt + 1] : ""
if (!url || !out) { console.error("usage: bun coax/web/shot.ts URL OUT.png [WxH] [WAIT_MS] [--eval JS]"); process.exit(2) }
const [w, h] = size.split("x").map(Number)
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "coax-shot-"))
const proc = Bun.spawn([chrome, "--headless=new", `--user-data-dir=${profile}`, "--no-first-run",
  "--hide-scrollbars", "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal",
  `--window-size=${w},${h}`, "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
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
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: w < 600 })
  await send("Page.navigate", { url })
  await Bun.sleep(Number(wait))
  if (script) { await send("Runtime.evaluate", { expression: script, awaitPromise: true }); await Bun.sleep(500) }
  const shot = await send("Page.captureScreenshot", { format: "png" })
  writeFileSync(out, Buffer.from(shot.result.data, "base64"))
  console.log(out)
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
