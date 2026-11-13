// Firm reason: 1 (what it checks is the browser's own: computed animations and element identity in the page dom-host.js draws)
// The running glyphs turn in /chat (components/skin/mono.coax `turn`): the dashed ring beside "Working" under the
// transcript (k_work_now), the one on a running tool call (k_work_head) and the one on a running session's row in
// the side (k_row_busy). Headless Chrome opens the chat as it is built (apps/openagents-web/build/chat-web.wasm,
// drawn by coax/web/dom-host.js) with Coder's /local/ answers scripted: one session running a tool call that
// hasn't come back. For each glyph it checks, over two seconds of frames, that
//   - the page keeps the same element (a host or a flush that makes it again restarts its turn every frame),
//   - its computed animation is `turn`, running, and its time only moves forward,
//   - and it is drawn at more than one angle;
// then, with a new tool call answered and another started each second, that "Working" and the row keep turning.
// It failed before the glyph's rules moved from the stage's sheet into the page's (the chat has no stage, so the
// ring beside "Working" and on a tool call had no rule and stood still).
//   bun coax/web/spin-check.ts            (builds chat-web.wasm first; --no-build uses the one there)
import { existsSync, readFileSync, statSync } from "node:fs"
import { join } from "node:path"
import { resolve } from "./golden-serve"
import { startChrome } from "./chrome"

const repo = join(import.meta.dir, "../..")
const app = join(repo, "apps/openagents-web")
if (!process.argv.includes("--no-build")) {
  const b = Bun.spawnSync([join(import.meta.dir, "wasm.sh"), join(app, "chat-web.coax"), join(app, "chat-web-face.c"), join(app, "build/chat-web.wasm")], { stdout: "inherit", stderr: "inherit" })
  if (b.exitCode !== 0) { console.error("spin-check: chat-web.wasm didn't build"); process.exit(2) }
}
if (!existsSync(join(app, "build/chat-web.wasm"))) { console.error("spin-check: no apps/openagents-web/build/chat-web.wasm (build it, or drop --no-build)"); process.exit(2) }

// Coder's answers: one session running (s.0), a busy Claude Code conversation (c.0), and the session's log: the
// person's message and a tool call still running.
const SESSION = "20261113-050000-001"
const SESSIONS = JSON.stringify([{ id: SESSION, engine: "claude", state: "running", turns: 1, steps: 1, started_us: 1, cwd: "/w", task: "spin check" }])
const CLAUDE = JSON.stringify({ live: [{ id: "7fe255eb-ec54-4f77-b7b3-64252ee8d7ff", busy: true }], sessions: [{ session_id: "7fe255eb-ec54-4f77-b7b3-64252ee8d7ff", title: "Busy" }] })
let log = [{ kind: "session", id: SESSION, engine: "claude" }, { kind: "user", text: "wait a while" },
  { kind: "tool", label: "Bash", tool: "bash", name: "Bash", id: "t0", input: { command: "sleep 600" } }].map((l) => JSON.stringify(l)).join("\n") + "\n"
// Once `growing`, each read of the log past its end finds the running call answered and a new one running.
let growing = false, calls = 1
const local = (url: URL): string | null => {
  if (url.pathname === "/local/sessions") return SESSIONS
  if (url.pathname === "/local/claude") return CLAUDE
  if (url.pathname === "/local/agents") return '{"agents":[]}'
  if (url.pathname !== "/local/log") return null
  const after = Number(url.searchParams.get("after") ?? 0)
  if (growing && after === log.length) {
    log += JSON.stringify({ kind: "result", text: "ok", error: false, id: "t" + (calls - 1) }) + "\n"
      + JSON.stringify({ kind: "tool", label: "Bash", tool: "bash", name: "Bash", id: "t" + calls, input: { command: "sleep " + calls } }) + "\n"
    calls++
  }
  const from = after >= 0 && after <= log.length ? after : 0
  return `{"next":${log.length},"size":${log.length}}\n` + log.slice(from)
}
const types: Record<string, string> = { js: "text/javascript; charset=utf-8" }
const server = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch(req) {
  const url = new URL(req.url)
  const said = local(url)
  if (said !== null) return new Response(said, { headers: { "content-type": "text/plain; charset=utf-8" } })
  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/local/")) return new Response("", { status: 404 })
  if (url.pathname.startsWith("/coaxial/")) {
    const file = join(import.meta.dir, url.pathname.slice(9).replace(/\.\./g, ""))
    try { if (statSync(file).isFile()) return new Response(readFileSync(file), { headers: { "content-type": types[file.split(".").pop() ?? ""] ?? "application/octet-stream" } }) } catch {}
    return new Response("", { status: 404 })
  }
  const r = resolve(app, url.pathname)
  return r ? new Response(Bun.file(r.file), { headers: { "content-type": r.type } }) : new Response("", { status: 404 })
} })

const { port } = await startChrome("coax-spin-", ["--window-size=1280,900"])
const page = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((r, j) => { ws.addEventListener("open", r); ws.addEventListener("error", j) })
let next = 0
const waiting = new Map<number, (v: any) => void>()
ws.addEventListener("message", (e) => { const m = JSON.parse(String(e.data)); if (m.id && waiting.has(m.id)) { waiting.get(m.id)!(m); waiting.delete(m.id) } })
const call = (method: string, params: any = {}) => new Promise<any>((r) => { const id = ++next; waiting.set(id, r); ws.send(JSON.stringify({ id, method, params })) })
const evaluate = async (expression: string) => (await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.result?.value
const until = async (expression: string, ms: number) => { const end = Date.now() + ms; while (Date.now() < end) { if (await evaluate(expression)) return true; await Bun.sleep(100) } return false }
const done = (code: number) => { server.stop(true); process.exit(code) }

await call("Page.navigate", { url: `http://127.0.0.1:${server.port}/chat` })
if (!await until("!!document.getElementById('s.0.busy')", 15000)) { console.error("spin-check: the running session's row (s.0) never showed in the side"); done(1) }
await evaluate("document.getElementById('s.0').click()")
if (!await until("!!document.getElementById('l.work.g.ic') && !!document.querySelector('.wk-h .gl[data-state=run] .ic')", 15000)) {
  console.error("spin-check: the running session's transcript never showed \"Working\" and its running tool call"); done(1)
}
const sample = (which: string): Promise<string> => evaluate(`(async () => {
  const all = { work: () => document.getElementById('l.work.g.ic'), tool: () => document.querySelector('.wk-h .gl[data-state=run] .ic'), row: () => document.getElementById('s.0.busy') };
  const els = {}; for (const k of ${JSON.stringify(which)}.split(',')) els[k] = all[k]();
  const seen = {};
  for (const [k, e] of Object.entries(els)) { seen[k] = { times: [], angles: new Set(), names: new Set(), states: new Set(), same: true }; }
  for (let i = 0; i < 18; i++) {
    await new Promise((r) => setTimeout(r, 170));
    for (const [k, e] of Object.entries(els)) {
      const now = all[k]();
      const s = seen[k];
      if (now !== e || !e.isConnected) s.same = false;
      const cs = getComputedStyle(e), a = e.getAnimations()[0];
      s.names.add(cs.animationName); s.states.add(cs.animationPlayState); s.angles.add(cs.transform);
      s.times.push(a ? a.currentTime : -1);
    }
  }
  const out = [];
  for (const [k, s] of Object.entries(seen)) {
    const forward = s.times.every((t, i) => t >= 0 && (i === 0 || t > s.times[i - 1]));
    const bad = [];
    if (!s.same) bad.push('made again while it runs');
    if (s.names.size !== 1 || !s.names.has('turn')) bad.push('animation ' + [...s.names].join(','));
    if (!s.states.has('running') || s.states.size !== 1) bad.push('play state ' + [...s.states].join(','));
    if (!forward) bad.push('its time ' + s.times.map((t) => Math.round(t)).join(','));
    if (s.angles.size < 4) bad.push(s.angles.size + ' angles');
    out.push((bad.length ? 'FAIL ' : 'ok   ') + k + (bad.length ? ': ' + bad.join('; ') : ': the same element, turn running, ' + s.angles.size + ' angles'));
  }
  return out.join('\\n');
})()`)
// a tool call that is running: all three
const still = await sample('work,tool,row')
// tool calls coming in: "Working" and the row keep turning (each new call has a glyph of its own)
growing = true
const coming = await sample('work,row')
const report = still + '\n' + coming.split('\n').map((l) => l.replace(/^(ok   |FAIL )/, '$1(calls coming in) ')).join('\n')
console.log(report + `\n(${calls - 1} tool calls came in while it watched)`)
done(/FAIL/.test(report) ? 1 : 0)
