// The Haunted House's saves, played in two headless Chrome browsers (no
// window opens) on a live host (pt-saves): browser A joins with an email,
// opens the house, places a jack-o'-lantern and leaves; its save is in its
// IndexedDB and on the server. Browser B joins with the same email in its
// own profile and opens the house: the server's save opens there and B
// keeps it as its own current save. Requests carry the verse-automation
// cookie, so analytics leave them out.
//   bun coax/web/api/saves.ts https://staging.openagents.com
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const origin = process.argv[2]
if (!origin) { console.error("usage: bun coax/web/api/saves.ts ORIGIN"); process.exit(2) }
const page = origin + "/realm/haunted-house"
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const email = `saves-smoke+${Date.now()}@openagents.com`

async function browser() {
  const profile = mkdtempSync(join(tmpdir(), "coax-saves-"))
  const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run", "--enable-unsafe-webgpu",
    "--enable-features=Vulkan", "--use-angle=metal", "--window-size=1280,800", "--remote-debugging-port=0", "about:blank"],
    { stdout: "ignore", stderr: "ignore" })
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
  const run = async (js: string) => (await send("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })).result?.result?.value
  const key = async (text: string) => {
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: text, text, code: "Key" + text.toUpperCase() })
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: text, code: "Key" + text.toUpperCase() })
  }
  await send("Network.enable")
  await send("Network.setCookie", { name: "verse-automation", value: "saves-smoke", url: origin })
  await send("Page.navigate", { url: page })
  await Bun.sleep(1500)
  // join with the email, as the landing does, and keep the player where the host reads it
  await run(`(async () => {
    const r = await fetch("/api/playtest/join", { method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: ${JSON.stringify(email)}, how: "email", player: "saves-smoke", sid: "saves-smoke" }) })
    localStorage.setItem("verse.player", JSON.stringify((await r.json()).player)); return true })()`)
  return { send, run, key, close: () => { ws.close(); proc.kill(); rmSync(profile, { recursive: true, force: true }) } }
}
const record = `new Promise((ok) => { const r = indexedDB.open("coaxial", 1); r.onsuccess = () => {
  const q = r.result.transaction("kept").objectStore("kept").get("realm"); q.onsuccess = () => ok(q.result || ""); q.onerror = () => ok(""); };
  r.onerror = () => ok(""); })`
const server = `fetch("/api/playtest/progress/haunted-house/main", { headers: { "X-Verse-Player": JSON.parse(localStorage.getItem("verse.player")).key } }).then(r => r.json())`

let failed = 0
const check = (name: string, ok: boolean, detail?: any) => { console.log((ok ? "  ok    " : "  FAIL  ") + name + (ok ? "" : "  " + JSON.stringify(detail).slice(0, 300))); failed += ok ? 0 : 1 }
const a = await browser(), b = await browser()
try {
  await a.send("Page.reload"); await Bun.sleep(9000)          // the house opens with the player's facts
  await a.key("b"); await Bun.sleep(400); await a.key("1"); await Bun.sleep(3500)   // edit mode, a jack-o'-lantern
  const kept: string = await a.run(record)
  const lines = kept.split("\n")
  check("A keeps the house in IndexedDB, as an AESV save", lines[0] === "verse-save 1" && atob(lines[2] || "").startsWith("AESV"), kept.slice(0, 80))
  const there = await a.run(server)
  check("A's save is on the server", !!lines[2] && there?.current?.bytes === lines[2], there)
  await b.send("Page.reload"); await Bun.sleep(9000)
  const keptB: string = await b.run(record)
  check("B, another browser with the same email, opens A's save and keeps it", !!lines[2] && keptB.split("\n")[2] === lines[2], keptB.slice(0, 80))
} finally { a.close(); b.close() }
console.log(failed ? "saves: FAILED on " + origin : "saves: passed on " + origin)
process.exit(failed ? 1 : 0)
