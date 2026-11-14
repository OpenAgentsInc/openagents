// Firm reason: 1 (what it checks is the browser's: the GPU calls host.js makes, recorded in headless Chrome)
// The frame plan's proof on every page (#13451): each page of realms.json opened with ?plan=check, where
// host.js records the built-in order and the plan's walk on the same frame and counts the frames whose GPU
// calls differ (coaxial.framePlan()). A page passes when its plan was walked and no frame differed.
//   coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9624 --static
//   bun coax/web/plan-check.ts [http://127.0.0.1:9624] [--only a,b] [--seconds 8]
// A page drawn by its own render package (data-render, RenderHost) says "renderer": it has no built-in order.
// A page that became the app shell (data-shell) is read in the frame it holds itself in (PAGE_ENGINE).
// Exit 1 when a page differs, refuses its plan on any frame (with no built-in order to fall back on, a refused
// frame is drawn as nothing), has no host or draws no frame in time; each line says which and the first
// differing call.
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
await send("Page.addScriptToEvaluateOnNewDocument", { source: "globalThis.coaxialGolden = true; globalThis.coaxialPlanCheck = true" })

let bad = 0
for (const name of names) {
  errors.length = 0
  const path = pages[name].path
  await send("Page.navigate", { url: `${base}${path}${path.includes("?") ? "&" : "?"}plan=check` })
  const end = Date.now() + seconds * 1000
  let s: any = null
  while (Date.now() < end) {
    await Bun.sleep(500)
    s = await evaluate(`(() => { const c = ${PAGE_ENGINE}; return c?.framePlan ? { ...c.framePlan(), renderer: !!c.renderStatus } : null })()`)
    if (s && s.checked >= 30) break
  }
  // a page drawn by its render package (RenderHost) has no built-in order to compare
  const verdict = !s ? "no host" : s.renderer ? "renderer" : s.differing ? "DIFFERS" : s.refused ? "REFUSED" : !s.checked ? "not drawn" : "ok"
  if (verdict !== "ok" && verdict !== "renderer") bad++
  console.log(`${verdict.padEnd(9)} ${name.padEnd(28)} ${s ? `frames ${s.frames} checked ${s.checked} differing ${s.differing} refused ${s.refused} ops ${s.ops}` : ""}${errors.length ? `  errors: ${errors.slice(0, 2).join(" | ")}` : ""}`)
  if (s?.first) console.log(`          first at ${JSON.stringify(s.first)}`)
}
ws.close()
process.exit(bad ? 1 : 0)
