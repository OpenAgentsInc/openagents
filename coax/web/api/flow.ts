// The playtest's player flow, played from a page on a live host in headless
// Chrome (no window opens): what a Verse page does with /api/playtest/
// (player.c). A guest gets a key, makes a world and publishes it; a second
// browser opens it by its link; the guest joins with an email and saves;
// the second browser joins with the same email and loads that save. Every
// request carries X-Verse-Automation: flow-smoke, so analytics leave it out.
//   bun coax/web/api/flow.ts https://staging.openagents.com [PAGE_PATH]
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const origin = process.argv[2]
const page = process.argv[3] ?? "/realm/haunted-house"
if (!origin) { console.error("usage: bun coax/web/api/flow.ts ORIGIN [PAGE_PATH]"); process.exit(2) }

// Runs in the page: same origin, the browser's own fetch.
const flow = async () => {
  const H = { "Content-Type": "application/json", "X-Verse-Automation": "flow-smoke" }
  const api = async (method: string, path: string, body?: any, key?: string) => {
    const r = await fetch("/api/playtest/" + path, { method, headers: key ? { ...H, "X-Verse-Player": key } : H,
      body: body === undefined ? undefined : JSON.stringify(body) })
    return { status: r.status, body: await r.json() }
  }
  const crc32 = (b: Uint8Array) => {
    let c = ~0
    for (const x of b) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)) }
    return ~c >>> 0
  }
  const save = (text: string) => {
    const head = new Uint8Array([65, 69, 83, 86, 2, 2, ...new TextEncoder().encode(text)])
    const c = crc32(head), out = new Uint8Array(head.length + 4)
    out.set(head); out.set([c & 255, (c >>> 8) & 255, (c >>> 16) & 255, c >>> 24], head.length)
    return btoa(String.fromCharCode(...out))
  }
  // A document comes back as the same JSON value (Postgres jsonb keeps values, not key order).
  const same = (a: any, b: any): boolean => a && b && typeof a === "object" && typeof b === "object"
    ? Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(k => same(a[k], b[k])) : a === b
  const steps: [string, boolean, any][] = []
  const step = (name: string, ok: boolean, detail?: any) => steps.push([name, ok, detail])

  const guest = await api("POST", "player", {})
  step("a guest gets a player key", guest.status === 200 && guest.body.key?.length >= 20, guest)
  const key = guest.body.key
  const made = await api("POST", "worlds", {}, key)
  const wid = made.body.id
  step("the guest makes a world", made.status === 200 && /^w-/.test(wid), made)
  const doc = { kind: "haunt", objects: [{ id: "lamp", x: 1.5, z: -2 }], at: Date.now() }
  const pub = await api("POST", "world/" + wid, { doc, summary: "a lamp by the door", user: { name: "Flow" } }, key)
  step("and publishes it", pub.status === 200 && pub.body.version === 1, pub)
  const opened = await api("GET", "world/" + wid)
  step("another browser opens it by its link", opened.status === 200 && same(opened.body.doc, doc), opened)
  const own = await api("GET", "world/" + wid + "/owner")
  step("and is not its owner", own.body.claimed === true && own.body.you === false, own)
  const email = "flow-smoke+" + Date.now() + "@openagents.com"
  const join = await api("POST", "join", { email, player: "flow", sid: "flow-" + Date.now(), how: "email" }, key)
  step("the guest joins with an email", join.status === 200 && join.body.player.key === key && /^e-/.test(join.body.player.account), join)
  const bytes = save("the house, room 3, two haunts found")
  const kept = await api("POST", "progress/haunted-house/main", { bytes, at: Date.now() }, key)
  step("and saves", kept.status === 200 && kept.body.current.bytes === bytes, kept)
  const other = await api("POST", "player", {})
  const there = await api("POST", "join", { email, how: "email" }, other.body.key)
  const loaded = await api("GET", "progress/haunted-house/main", undefined, other.body.key)
  step("the same email in another browser loads that save", there.status === 200 && loaded.body.current?.bytes === bytes, loaded)
  const log = await api("GET", "log/" + wid)
  step("the world's log has the publish", log.body[0]?.summary === "a lamp by the door", log)
  return steps
}

const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "coax-flow-"))
const proc = Bun.spawn([chrome, "--headless=new", `--user-data-dir=${profile}`, "--no-first-run",
  "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
let failed = 1
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
  await send("Network.enable")
  await send("Network.setCookie", { name: "verse-automation", value: "flow-smoke", url: origin })
  await send("Page.navigate", { url: origin + page })
  await Bun.sleep(3000)
  const js = new Bun.Transpiler({ loader: "ts" }).transformSync("(" + flow.toString() + ")()")
  const r = await send("Runtime.evaluate", { expression: js, awaitPromise: true, returnByValue: true })
  const steps: [string, boolean, any][] = r.result?.result?.value ?? []
  if (!steps.length) console.log("  FAIL  the flow did not run: " + JSON.stringify(r.result?.exceptionDetails ?? r).slice(0, 400))
  for (const [name, ok, detail] of steps) console.log((ok ? "  ok    " : "  FAIL  ") + name + (ok ? "" : "  " + JSON.stringify(detail).slice(0, 300)))
  failed = steps.length && steps.every(s => s[1]) ? 0 : 1
  console.log(failed ? "flow: FAILED on " + origin : "flow: passed on " + origin + page)
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
process.exit(failed)
