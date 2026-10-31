// A reload keeps your place (place.coax, host.js action/message 11): for each
// walkable realm, in one headless Chrome profile kept across the reload, it
// opens the page, steps once (where the spawn is), walks well away, waits for
// the keep, reloads, steps once more and checks you are back where you were
// (feet within a step, the camera's yaw as kept), not at the spawn. Then the
// same from a link whose hash names a place (Everglade #quest-board): once you
// walk off it the hash is dropped, so the reload keeps your place too.
//   bun coax/web/place-reload.ts https://staging.openagents.com [realm ...] [--profile DIR]
// Exit 1 names each realm that lost the place. Headless and muted; never a window.
import { mkdtempSync, readFileSync, existsSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { launch as launchChrome } from "./chrome"

// realm page, its save slot, how to walk away from the spawn
// (click: where to press first on a fresh profile at 1280x800, the fairy maker's DONE)
// (step: the key a measuring step takes, where ahead is a wall: the quest board)
const REALMS: Record<string, { slot: string; walk: string; hash?: string; click?: [number, number]; step?: string }> = {
  "everglade": { slot: "everglade", walk: "w5000 d900 w3000" },
  "everglade#quest-board": { slot: "everglade", walk: "s4000 d900 s2500", hash: "quest-board", step: "s" },
  "fairy-forest": { slot: "fairy-forest", walk: "w3500 d900 w2000", click: [1073, 591] },
  "hecates-cemetery": { slot: "hecates-cemetery", walk: "w3500 d900 w2000" },
  "the-grid": { slot: "the-grid", walk: "w3500 d900 w2000" },
  "garden": { slot: "garden", walk: "w3500 d900 w2000" },
  "haunted-house": { slot: "haunted-house", walk: "w2500 d600 w1500" },
  "sandbox": { slot: "sandbox", walk: "w2500 d600 w1500" },
}
const args = process.argv.slice(2)
const pAt = args.indexOf("--profile")
const kept = pAt >= 0 ? args.splice(pAt, 2)[1] : ""
const [base, ...only] = args
if (!base) { console.error("usage: bun coax/web/place-reload.ts BASE [realm ...] [--profile DIR]"); process.exit(2) }
const WAIT = Number(process.env.PLACE_WAIT || 12000)
const STEP = 400

type Place = { x: number; z: number; yaw: number; zoom: number; at: number }
const read = (t: string | null): Place | null => {
  if (!t) return null
  const n = (k: string) => { const m = t.match(new RegExp(`\\n${k}=(-?\\d+)`)); return m ? Number(m[1]) / 1000 : NaN }
  return { x: n("x"), z: n("z"), yaw: n("yaw"), zoom: n("zoom"), at: n("at") }
}
const far = (a: Place, b: Place) => Math.hypot(a.x - b.x, a.z - b.z)
const turned = (a: number, b: number) => { const d = Math.abs(a - b) % (2 * Math.PI); return Math.min(d, 2 * Math.PI - d) }

async function shot(send: (m: string, p?: any) => Promise<any>, out: string) {
  const s = await send("Page.captureScreenshot", { format: "png" })
  await Bun.write(out, Buffer.from(s.result.data, "base64"))
}

async function check(name: string): Promise<string> {
  const r = REALMS[name]
  const profile = kept ? join(kept, name.replace(/[^a-z-]/g, "_")) : mkdtempSync(join(tmpdir(), "coax-place-"))
  const proc = launchChrome(profile, ["--window-size=1280,800"])
  try {
    const portFile = join(profile, "DevToolsActivePort")
    for (let i = 0; i < 200 && !(existsSync(portFile) && readFileSync(portFile, "utf8").includes("\n")); i++) await Bun.sleep(50)
    const port = readFileSync(portFile, "utf8").split("\n")[0]
    const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
    const ws = new WebSocket(target.webSocketDebuggerUrl)
    await new Promise(ok => ws.addEventListener("open", ok, { once: true }))
    let id = 0
    const pending = new Map<number, (v: any) => void>()
    ws.addEventListener("message", e => { const m = JSON.parse(String(e.data)); pending.get(m.id)?.(m); pending.delete(m.id) })
    const send = (method: string, params: any = {}) => new Promise<any>(ok => { pending.set(++id, ok); ws.send(JSON.stringify({ id, method, params })) })
    const ev = async (e: string) => (await send("Runtime.evaluate", { expression: e, returnByValue: true }))?.result?.result?.value
    const key = async (k: string, ms: number) => {
      const p = { code: "Key" + k.toUpperCase(), key: k, windowsVirtualKeyCode: k.toUpperCase().charCodeAt(0) }
      await send("Input.dispatchKeyEvent", { type: "keyDown", ...p }); await Bun.sleep(ms); await send("Input.dispatchKeyEvent", { type: "keyUp", ...p })
    }
    const place = async () => read(await ev(`localStorage.getItem("coaxial.${r.slot}.place")`))
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false })
    await ev(`localStorage.removeItem("coaxial.${r.slot}.place")`)
    await send("Page.navigate", { url: `${base}/realms/${name}` })
    await Bun.sleep(WAIT)
    await ev(`localStorage.removeItem("coaxial.${r.slot}.place")`)   // a fresh start: the spawn
    if (r.click) {
      const [x, y] = r.click
      for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
      await Bun.sleep(1000)
    }
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 640, y: 400 })
    await key(r.step || "w", STEP); await Bun.sleep(1500)
    const spawn = await place()
    if (!spawn) { if (process.env.PLACE_SHOTS) await shot(send, `${process.env.PLACE_SHOTS}/${name}-0.png`); return `${name}: nothing kept after a step` }
    await key(r.step || "w", STEP); await Bun.sleep(1500)
    const second = await place()
    if (!second || second.at <= spawn.at) return `${name}: nothing kept after a second step`
    const step = far(second, spawn)
    for (const m of r.walk.split(" ")) await key(m[0], Number(m.slice(1)))
    await Bun.sleep(2000)
    const left = await place()
    if (!left || far(left, spawn) < 14) return `${name}: the walk didn't get away from the spawn (${left ? far(left, spawn).toFixed(1) : "none"} m)`
    if (r.hash && (await ev("location.hash"))) return `${name}: the hash stayed after walking off the place it named`
    await send("Page.reload", {})
    await Bun.sleep(WAIT)
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 640, y: 400 })
    // one step; when a wall is ahead of where you came back, another way
    let back: Place | null = null
    for (const k of [r.step || "w", "s", "a", "d"]) {
      await key(k, STEP); await Bun.sleep(1500)
      back = await place()
      if (back && back.at > left.at) break
    }
    if (!back || back.at <= left.at) { if (process.env.PLACE_SHOTS) await shot(send, `${process.env.PLACE_SHOTS}/${name}.png`); return `${name}: nothing kept after the reload's step` }
    const d = far(back, left), s = far(back, spawn)
    // a step is at most about 3 m (a run's 0.4 s); the camera may swing behind you as you step
    const ok = d < Math.max(step * 1.5, 4.5) && d < s / 3 && turned(back.yaw, left.yaw) < 0.6 && Math.abs(back.zoom - left.zoom) < 0.3
    console.log(`${ok ? "ok  " : "LOST"} ${name}: ${d.toFixed(2)} m from where you were (a step is ${step.toFixed(2)} m), ${s.toFixed(1)} m from the spawn, yaw ${left.yaw} -> ${back.yaw}, zoom ${left.zoom} -> ${back.zoom}`)
    ws.close()
    return ok ? "" : `${name}: the reload lost the place`
  } finally {
    proc.kill()
    if (!kept) rmSync(profile, { recursive: true, force: true })
  }
}

const names = only.length ? only : Object.keys(REALMS)
const failed = (await Promise.all(names.map(n => REALMS[n] ? check(n) : Promise.resolve(`${n}: not a realm this check knows`)))).filter(Boolean)
for (const f of failed) console.error("place-reload: " + f)
process.exit(failed.length ? 1 : 0)
