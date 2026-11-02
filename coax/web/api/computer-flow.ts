// YOUR COMPUTER, played in headless Chrome (no window opens) against a local relay and a real
// `coder host` (openagents#12642; docs/verse/realm-edit.md "Your computer"). The page is a made realm
// from staging, served here with the module built from this checkout and the realm's spec saying we
// own it; the page reaches the local relay through ?relay=ws://localhost:PORT (the wire takes only
// that form). The owner opens EDIT > COMPUTER, pastes the host's key and pairs; the person at the
// computer allows it (`coder host allow`); the pane shows the computer answers, a request runs as a
// job whose steps come in, a reload keeps the pairing, a phone-width page shows the pane, and FORGET
// takes the browser off the computer.
//
//   coax/relay/build.sh /tmp/relay && /tmp/relay --listen 127.0.0.1 --port 47777 --url ws://localhost:47777
//   OPENAGENTS_NIP_HOST_RELAY=ws://localhost:47777 CODER_HOME=/tmp/host-home coder/tui/coder.sh host --engine echo
//   COAXIAL_RUN_STEPS=250000000 coax/web/wasm.sh apps/openagents-web/myrealm.coax apps/openagents-web/myrealm-face.c \
//     /tmp/myrealm.wasm verse/edit/computer-wire.coax coax/lib/crypto/entropy.coax
//   CODER_HOME=/tmp/host-home bun coax/web/api/computer-flow.ts ws://localhost:47777 /tmp/myrealm.wasm [SHOTS_DIR]
import { readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { startChrome } from "../chrome"

const relay = process.argv[2], wasm = process.argv[3], shots = process.argv[4] ?? ""
if (!relay?.startsWith("ws://localhost:") || !wasm) { console.error("usage: bun coax/web/api/computer-flow.ts ws://localhost:PORT MYREALM.wasm [SHOTS_DIR]"); process.exit(2) }
const STAGING = "https://staging.openagents.com", SLUG = "pine-forest"
const repo = join(import.meta.dir, "../../..")
const coder = (...args: string[]) => Bun.spawnSync([join(repo, "coder/tui/coder.sh"), "host", ...args], { env: { ...process.env, BROWSER: "/usr/bin/true" } })
let failed = 0
const check = (name: string, ok: boolean, detail?: any) => {
  console.log((ok ? "  ok    " : "  FAIL  ") + name + (ok ? "" : "  " + JSON.stringify(detail ?? "").slice(0, 400)))
  if (!ok) failed++
}

// The page: staging's bytes, this checkout's module, our ownership of the realm; nothing sent anywhere.
const module = readFileSync(wasm)
const server = Bun.serve({ port: 0, async fetch(req) {
  const url = new URL(req.url)
  if (/^\/build\/myrealm\.[0-9a-f]+\.wasm$/.test(url.pathname)) return new Response(module, { headers: { "Content-Type": "application/wasm" } })
  if (req.method !== "GET") return new Response(null, { status: 204 })
  const r = await fetch(STAGING + url.pathname + url.search)
  if (url.pathname === "/api/playtest/realms/" + SLUG && r.ok) return new Response((await r.text()).replace(/\n?$/, "\n") + "role owner\nowner 1\n")
  return new Response(r.body, { status: r.status, headers: { "Content-Type": r.headers.get("Content-Type") ?? "application/octet-stream" } })
} })
const origin = `http://golden.localhost:${server.port}`
const page = `${origin}/realms/${SLUG}?relay=${relay}`

const hostKey = coder("key").stdout.toString().trim().split(/\s+/)
const npub = hostKey.find(w => w.startsWith("npub1")) ?? ""
check("coder host key gives the npub to paste", npub.length === 63, hostKey)

const { proc, port } = await startChrome("computer-flow-")
try {
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise(r => ws.addEventListener("open", r, { once: true }))
  let id = 0
  const pending = new Map<number, (v: any) => void>()
  ws.addEventListener("message", e => { const m = JSON.parse(String(e.data)); pending.get(m.id)?.(m); pending.delete(m.id) })
  const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  const js = async (expression: string) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.result?.value
  const probe = async (f: number) => Number(await js(`coaxial.guardProbe(${2400 + f})`) ?? -1)
  const view = () => probe(97)
  const box = async (b: number) => [await probe(4 * b), await probe(4 * b + 1), await probe(4 * b + 2), await probe(4 * b + 3)]
  const tap = async (b: number) => {
    const [x0, y0, x1, y1] = await box(b)
    if (x0 < 0) return false
    const x = (x0 + x1) / 2, y = (y0 + y1) / 2
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
      await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
    await Bun.sleep(300)
    return true
  }
  const typeText = async (text: string) => { await send("Input.insertText", { text }); await Bun.sleep(200) }
  const enter = async () => {
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 })
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 })
    await Bun.sleep(300)
  }
  const until = async (ok: () => Promise<boolean>, ms: number) => { for (let t = 0; t < ms; t += 250) { if (await ok()) return true; await Bun.sleep(250) } return false }
  const shot = async (name: string) => {
    if (!shots) return
    const s = await send("Page.captureScreenshot", { format: "png" })
    writeFileSync(join(shots, name + ".png"), Buffer.from(s.result.data, "base64"))
  }
  const size = (w: number, h: number, mobile: boolean) => send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile })
  // Opens the realm and its COMPUTER pane.
  const open = async () => {
    await send("Page.navigate", { url: page })
    await until(async () => (await probe(0)) >= 0, 30000)   // EDIT is up once the spec says we own it
    await tap(0)                                              // EDIT
    await until(async () => (await probe(4 * 12)) >= 0, 5000)
    await tap(12)                                             // COMPUTER
    return until(async () => (await probe(96)) === 6 && (await view()) >= 1, 10000)
  }

  await size(1440, 900, false)
  check("the owner opens EDIT > COMPUTER", await open())
  check("unpaired, it asks for the computer's key", await view() === 1)
  await shot("1-unpaired")
  await tap(16)                                               // the field
  await typeText(npub)
  await enter()
  check("PAIR asks the computer: ALLOW THIS BROWSER", await until(async () => await view() === 2, 5000), await view())
  let waiting: any[] = []
  await until(async () => { waiting = JSON.parse(coder("list", "--json").stdout.toString() || "{}").waiting ?? []; return waiting.length > 0 }, 20000)
  check("the computer has the ask, with this browser's name", waiting.length === 1 && /on Mac|on Linux/.test(waiting[0]?.label ?? ""), waiting)
  await shot("2-asking")
  const ask = String(waiting[0]?.request ?? waiting[0]?.id ?? "")
  const allowed = coder("allow", ask.slice(0, 8))
  check("coder host allow allows it", allowed.exitCode === 0, allowed.stderr.toString())
  check("the pane says allowed, and that the computer answers", await until(async () => await view() === 3, 15000), await view())
  await shot("3-allowed")

  await tap(13)                                               // TYPE A REQUEST
  await typeText("add a red barn by the pines")
  await tap(13)                                               // SEND
  const ran = await until(async () => [7, 8, 9, 10].includes(await view()), 8000)
  check("SEND makes it a job", ran, await view())
  const ended = await until(async () => [8, 9, 10].includes(await view()), 60000)
  const last = await view()
  check("its steps come in and the job ends", ended && (last === 10 || last === 9), last)
  await shot("4-job")

  await open()
  check("a reload keeps the pairing", await until(async () => await view() === 3, 15000), await view())

  await size(390, 844, true)
  check("on a phone width the pane opens", await open() && await until(async () => await view() === 3, 15000))
  const [cx0, , cx1] = await box(14)
  check("its controls fit the phone's width", cx0 >= 0 && cx1 <= 390, [cx0, cx1])
  await shot("5-phone")

  await tap(14); await tap(14)                                // FORGET THIS COMPUTER, SURE?
  check("FORGET takes it back to unpaired", await until(async () => await view() === 1, 3000), await view())
  const gone = await until(async () => (JSON.parse(coder("list", "--json").stdout.toString() || "{}").browsers ?? [1]).length === 0, 15000)
  check("the computer no longer lists this browser", gone)
  ws.close()
} finally {
  proc.kill()
  server.stop(true)
}
console.log(failed ? `computer flow: ${failed} failed` : "computer flow: passed")
process.exit(failed ? 1 : 0)
