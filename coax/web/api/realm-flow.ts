// A player's realm and the changes offered to it (pt-planets, pt-change-offers),
// played on a live host in headless Chrome (no window opens). A guest makes
// a realm; a second player opens it by its link, edits it and leaves edit
// mode, which offers the change; the owner opens the realm, sees the offer
// and presses ACCEPT, and the world shows the change after a reload; a second
// offer the owner presses DECLINE on changes nothing; the edit log lists every
// edit in order. Requests carry the automation mark realm-flow.
//   bun coax/web/api/realm-flow.ts ORIGIN [SHOTS_DIR]
import { mkdtempSync, readFileSync, rmSync, existsSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const origin = process.argv[2]
const shots = process.argv[3] ?? ""
if (!origin) { console.error("usage: bun coax/web/api/realm-flow.ts ORIGIN [SHOTS_DIR]"); process.exit(2) }
const H = { "Content-Type": "application/json", "X-Verse-Automation": "realm-flow" }
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
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "realm-flow-"))
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
    await js(`localStorage.setItem("verse.player", ${JSON.stringify(JSON.stringify(player))}); document.cookie = "verse-automation=realm-flow; path=/"; 1`)
    await send("Page.navigate", { url: origin + "/realm/haunted-house#" + world })
    await Bun.sleep(9000)
  }

  const owner = (await api("POST", "player", {})).body
  const visitor = (await api("POST", "player", {})).body
  const made = await api("POST", "planets", { user: { id: owner.id, name: "Flow owner" }, settings: { name: "Flow realm", kind: "cemetery" } }, owner.key)
  const pid = made.body.id
  check("a guest is awarded a realm and sets it up", made.status === 200 && pid?.startsWith("planet-"), made)
  const listed = await api("GET", "planets?after=" + (made.body.n - 1))
  check("the galaxy lists the new realm", listed.body.planets?.some((p: any) => p.id === pid), listed)

  // The second player edits the realm and leaves edit mode: an offer.
  await open(visitor, pid)
  await shot("1-visitor")
  await key("b"); await key("1"); await key("b")
  await Bun.sleep(2500)
  await shot("2-offered")
  let log = await api("GET", "log/" + pid)
  const first = log.body[0]
  check("a second session offers a change", log.status === 200 && first?.state === "waiting" && first?.by?.player === visitor.id, log.body)
  check("before the owner decides, the world is unchanged", (await api("GET", "world/" + pid)).body.version === 0)

  // The owner opens the realm: the offer, ACCEPT.
  await open(owner, pid)
  await shot("3-owner")
  const box = { x1: W - 20, x0: W - 20 - 340, y0: 20, y1: 138 }
  const bw = (340 - 44) * 0.5
  await click(box.x0 + 16 + bw / 2, box.y1 - 12 - 15)
  await Bun.sleep(2500)
  await shot("4-accepted")
  const world = await api("GET", "world/" + pid)
  log = await api("GET", "log/" + pid)
  check("the owner accepts it: the world is published with it", world.body.version === 1 && world.body.doc?.kind === "verse.haunt.save"
    && log.body[0]?.state === "accepted" && log.body[0]?.version === 1, { world: world.body.version, log: log.body })
  await open(owner, pid)
  await shot("5-reloaded")
  check("the world shows it after reload", (await api("GET", "world/" + pid)).body.version === 1)

  // A second offer, declined.
  await open(visitor, pid)
  await key("b"); await key("2"); await key("b")
  await Bun.sleep(2500)
  check("a second offer waits", (await api("GET", "log/" + pid)).body[0]?.state === "waiting")
  await open(owner, pid)
  await click(box.x0 + 16 + bw + 12 + bw / 2, box.y1 - 12 - 15)
  await Bun.sleep(2500)
  await shot("6-declined")
  const still = await api("GET", "world/" + pid)
  log = await api("GET", "log/" + pid)
  check("a declined offer changes nothing", still.body.version === 1 && log.body[0]?.state === "declined", log.body)
  check("the edit log lists every edit in order", log.body.map((e: any) => e.state).join(",") === "declined,accepted", log.body)
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(failed ? `realm flow: ${failed} failed` : "realm flow: passed")
process.exit(failed ? 1 : 0)
