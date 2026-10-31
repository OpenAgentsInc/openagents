// Record a clip of a Coaxial page in headless Chrome: every frame drawn at a fixed
// step of game time (host.js clip mode), captured one by one, then encoded with ffmpeg.
// Nothing opens on screen and no sound plays.
//   bun coax/web/clip.ts URL OUT.mp4 [--size 1280x720] [--seconds 3] [--fps 30]
//       [--skip SECONDS] [--eval JS] [--frames DIR]
// --skip runs that much game time first without capturing (the lead-up to the moment);
// --eval runs a script once the page has started, before the skip (a staged moment's inputs).
// A page stages its own moments from its URL (?stage=NAME): see docs/verse/clips.md.
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync, mkdirSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { chrome as chromePath, gpuFlags } from "./chrome"

const args = process.argv.slice(2)
const flag = (name: string, fallback: string) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback }
const [url, out] = args
if (!url || !out || url.startsWith("--")) {
  console.error("usage: bun coax/web/clip.ts URL OUT.mp4 [--size WxH] [--seconds S] [--fps N] [--skip S] [--eval JS] [--frames DIR]")
  process.exit(2)
}
const [w, h] = flag("size", "1280x720").split("x").map(Number)
const seconds = Number(flag("seconds", "3")), fps = Number(flag("fps", "30")), skip = Number(flag("skip", "0"))
const script = flag("eval", "")
const frames = flag("frames", "") || mkdtempSync(join(tmpdir(), "coax-clip-frames-"))
mkdirSync(frames, { recursive: true })

const profile = mkdtempSync(join(tmpdir(), "coax-clip-"))
const proc = Bun.spawn([chromePath(), "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run",
  "--hide-scrollbars", ...gpuFlags(), `--window-size=${w},${h}`, "--remote-debugging-port=0", "about:blank"],
  { stdout: "ignore", stderr: "ignore" })
let failed = ""
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
  const run = async (expression: string) =>
    (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }))?.result?.result?.value
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: w < 600 })
  await send("Emulation.setFocusEmulationEnabled", { enabled: true })
  await send("Runtime.enable"); await send("Page.enable")
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialClip = true" })
  // a clip never counts as a visit
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "Object.defineProperty(Navigator.prototype, 'sendBeacon', { value: () => false })" })
  await send("Page.navigate", { url })
  // Ready when a step draws and the loading card is gone. Until then each step is a
  // thousandth of a millisecond of game time, so loading never eats the lead-up.
  let up = false
  for (let i = 0; i < 600 && !up; i++) {
    await Bun.sleep(100)
    up = (await run("typeof coaxialClipStep === 'function' ? coaxialClipStep(1, 1e6).then(d => d && !document.getElementById('coaxial-load')) : false")) === true
  }
  if (!up) throw new Error("the page never finished loading (no WebGPU, or the engine failed)")
  if (script) await run(script)
  for (let left = Math.round(skip * fps); left > 0; left -= 30) await run(`coaxialClipStep(${Math.min(30, left)}, ${fps})`)
  const total = Math.round(seconds * fps)
  for (let i = 0; i < total; i++) {
    await run(`coaxialClipStep(1, ${fps})`)
    const shot = await send("Page.captureScreenshot", { format: "png" })
    writeFileSync(join(frames, `f${String(i).padStart(5, "0")}.png`), Buffer.from(shot.result.data, "base64"))
  }
} catch (e: any) {
  failed = e.message
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
if (failed) { console.error(`clip: ${failed}`); process.exit(1) }
const enc = Bun.spawnSync(["ffmpeg", "-v", "error", "-y", "-framerate", String(fps), "-i", join(frames, "f%05d.png"),
  "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", out])
if (enc.exitCode !== 0) { console.error(`clip: ffmpeg failed: ${enc.stderr}`); process.exit(1) }
if (!args.includes("--frames")) rmSync(frames, { recursive: true, force: true })
console.log(out)
