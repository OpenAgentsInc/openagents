// Firm reason: 1 (what it checks is the browser's: the GPU calls host.js makes, recorded in headless Chrome)
// The frame plan on every page (#13451, #13539): each page of realms.json opened in headless Chrome, where
// host.js draws every frame by walking a plan, the page's own or the engine's (frame-plan-host.js), and counts
// the frames it walked and those whose plan was refused, which it clears (coaxial.framePlan()). host.js has
// no order of its own since #13539; until then ?plan=check compared the two on the same frame, and every
// page but Nostr Lab's had 0 differing frames (#13451, and #13539 for the three v1 Everglade pages and
// /worlds/new). A page passes when it walked its plan on 30 frames and refused none.
//   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9624 --static
//   bun coax/web/plan-check.ts [http://127.0.0.1:9624] [--only a,b] [--seconds 8]
// A page drawn by its own render package (data-render, RenderHost) says "renderer": it walks no plan here.
// A page that became the app shell (data-shell) is read in the frame it holds itself in (PAGE_ENGINE).
// Exit 1 when a page refuses its plan on any frame (a refused frame is drawn as nothing), has no host or
// walks fewer than 30 frames in time; each line says which.
import { goldenPages, PAGE_ENGINE } from "./golden-pages"
import { startChrome } from "./chrome"

const args = process.argv.slice(2)
const flag = (name: string) => { const i = args.indexOf(name); return i >= 0 ? args.splice(i, 2)[1] : undefined }
const only = flag("--only")?.split(",")
const seconds = Number(flag("--seconds") ?? 8)
const base = (args[0] ?? "http://127.0.0.1:9624").replace(/\/$/, "")
const { pages } = goldenPages()
const names = Object.keys(pages).filter((n) => !only || only.includes(n)).filter((n) => !/nostr|^nip-/.test(n))

const c = await startChrome("coax-plan-check-", ["--window-size=1280,720"])
const target = await (await fetch(`http://127.0.0.1:${c.port}/json/new?about:blank`, { method: "PUT" })).json()
const ws = new WebSocket(target.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener("open", r, { once: true }))
let id = 0
const pending = new Map<number, (v: any) => void>()
const errors: string[] = []
ws.addEventListener("message", (e) => {
  const m = JSON.parse(String(e.data))
  if (m.id) { pending.get(m.id)?.(m); pending.delete(m.id); return }
  if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails?.exception?.description?.split("\n")[0] ?? m.params.exceptionDetails?.text)
})
const send = (method: string, params: any = {}) => new Promise<any>((r) => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
const evaluate = async (expression: string) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result?.result?.value
await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false })
await send("Emulation.setFocusEmulationEnabled", { enabled: true })
await send("Runtime.enable"); await send("Page.enable")
await send("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialGolden = true" })

let bad = 0
for (const name of names) {
  errors.length = 0
  const path = pages[name].path
  await send("Page.navigate", { url: `${base}${path}` })
  const end = Date.now() + seconds * 1000
  let s: any = null
  while (Date.now() < end) {
    await Bun.sleep(500)
    s = await evaluate(`(() => { const c = ${PAGE_ENGINE}; return c?.framePlan ? { ...c.framePlan(), renderer: !!c.renderStatus } : null })()`)
    if (s && (s.renderer || s.refused || s.frames >= 30)) break
  }
  const verdict = !s ? "no host" : s.renderer ? "renderer" : s.refused ? "REFUSED" : s.frames < 30 ? "not drawn" : "ok"
  if (verdict !== "ok" && verdict !== "renderer") bad++
  console.log(`${verdict.padEnd(9)} ${name.padEnd(28)} ${s ? `frames ${s.frames} refused ${s.refused} ops ${s.ops}` : ""}${errors.length ? `  errors: ${errors.slice(0, 2).join(" | ")}` : ""}`)
}
ws.close()
process.exit(bad ? 1 : 0)
