// A guest's own realm from the galaxy (pt-planets), on a live host in
// headless Chrome (no window opens): with no player key, MAKE YOUR REALM, a
// name and ENTER mint a key, keep it, set the realm up and open its world;
// the server lists it; after a reload YOUR REALM reopens it. Requests carry
// the automation mark galaxy-flow.
//   bun coax/web/api/galaxy-flow.ts ORIGIN [SHOTS_DIR]
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { chrome as chromePath, gpuFlags } from "../chrome"

const origin = process.argv[2]
const shots = process.argv[3] ?? ""
if (!origin) { console.error("usage: bun coax/web/api/galaxy-flow.ts ORIGIN [SHOTS_DIR]"); process.exit(2) }
const H = { "Content-Type": "application/json", "X-Verse-Automation": "galaxy-flow" }
const api = async (method: string, path: string, body?: any, key?: string) => {
  const r = await fetch(origin + "/api/playtest/" + path, { method, headers: key ? { ...H, "X-Verse-Player": key } : H,
    body: body === undefined ? undefined : JSON.stringify(body) })
  return { status: r.status, body: await r.json() }
}
let failed = 0
const check = (name: string, ok: boolean, detail?: any) => {
  console.log((ok ? "  ok    " : "  FAIL  ") + name + (ok ? "" : "  " + JSON.stringify(detail ?? "").slice(0, 400)))
  if (!ok) failed++
}

const W = 1440, Hh = 900
const chrome = chromePath()
const profile = mkdtempSync(join(tmpdir(), "galaxy-flow-"))
const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars",
  ...gpuFlags(), `--window-size=${W},${Hh}`,
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
  const key = async (k: string) => {
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: k, text: k })
    await Bun.sleep(80)
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: k })
    await Bun.sleep(250)
  }
  const click = async (x: number, y: number) => {
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
      await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
    await Bun.sleep(400)
  }
  const shot = async (name: string) => {
    if (!shots) return
    const s = await send("Page.captureScreenshot", { format: "png" })
    writeFileSync(join(shots, name + ".png"), Buffer.from(s.result.data, "base64"))
  }
  // Opens the realm as this player: the browser keeps the key as the landing does.
  const open = async (player: any, world: string) => {
    await send("Page.navigate", { url: origin + "/404.html" })
    await Bun.sleep(800)
    await js(`localStorage.setItem("verse.player", ${JSON.stringify(JSON.stringify(player))}); document.cookie = "verse-automation=galaxy-flow; path=/"; 1`)
    await send("Page.navigate", { url: origin + "/realms/haunted-house#" + world })
    await Bun.sleep(9000)
  }

  const type = async (text: string) => { await send("Input.insertText", { text }); await Bun.sleep(250) }
  const galaxy = async () => { await send("Page.navigate", { url: origin + "/verse/galaxy" }); await Bun.sleep(5000) }

  // A guest with no key: MAKE YOUR REALM, a name, ENTER.
  await send("Page.navigate", { url: origin + "/404.html" })
  await Bun.sleep(800)
  await js(`localStorage.removeItem("verse.player"); document.cookie = "verse-automation=galaxy-flow; path=/"; 1`)
  await galaxy()
  await shot("1-galaxy")
  await click(W / 2, 110)
  await type("Flow Den")
  await shot("2-setup")
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, text: "\r" })
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 })
  await Bun.sleep(300)
  await shot("2b-entered")
  await Bun.sleep(3700)
  const url = await js("location.href")
  const kept = JSON.parse((await js(`localStorage.getItem("verse.player")`)) || "null")
  const pid = String(url).split("#")[1] ?? ""
  check("a guest sets up a realm and its world opens", String(url).includes("/realms/haunted-house#planet-") && kept?.key, url)
  const mine = await api("GET", "planets/mine", undefined, kept?.key)
  check("the server keeps it as theirs, named as typed", mine.body.planet?.id === pid && mine.body.planet?.name === "Flow Den", mine.body)
  const list = await api("GET", "planets?after=" + Math.max(0, mine.body.planet.n - 1))
  check("the galaxy lists the new realm", list.body.planets?.some((p: any) => p.id === pid), list.body)
  await shot("3-world")

  // After a reload the galaxy shows YOUR REALM, and it reopens the same world.
  await galaxy()
  await shot("4-galaxy-again")
  await click(W / 2, 110)
  await Bun.sleep(3000)
  check("YOUR REALM reopens it after a reload", String(await js("location.href")).endsWith("#" + pid), await js("location.href"))
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(failed ? `galaxy flow: ${failed} failed` : "galaxy flow: passed")
process.exit(failed ? 1 : 0)
