// Build your own realm by voice, end to end in headless Chrome (byo-voice-check):
// Chrome's fake microphone plays a recorded prompt, the setup questions are
// tapped through (the first answer each), TALK is tapped, the prompt is
// spoken, TALK is tapped again, and the voice host's words must reach the
// planner (POST /api/playtest/realms/plan answers 200 with a board) and the
// page must draw it. MAKE IT is never tapped: nothing is created. One fixed
// speaker id, so the voice host's daily speaker cap isn't spent; the session
// is marked automation (cookie verse-automation=voicecheck). Nothing is heard
// on this Mac (--mute-audio) and no window opens.
//   bun coax/web/voice/builder-smoke.ts https://staging.openagents.com [FIXTURE.wav]
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { chrome as chromePath, gpuFlags } from "../chrome"

const [origin = "https://staging.openagents.com", wav = join(import.meta.dir, "../../../verse/voice-host/fixtures/build-red-house.wav")] = process.argv.slice(2)
const W = 1440, H = 900
const profile = mkdtempSync(join(tmpdir(), "coax-voice-"))
const proc = Bun.spawn([chromePath(), "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run",
  "--disable-features=AudioServiceSandbox,AudioServiceOutOfProcess", ...gpuFlags(), `--window-size=${W},${H}`,
  "--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", `--use-file-for-fake-audio-capture=${resolve(wav)}%noloop`,
  "--autoplay-policy=no-user-gesture-required", "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
let ok = false
try {
  const portFile = join(profile, "DevToolsActivePort")
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await Bun.sleep(50)
  const port = readFileSync(portFile, "utf8").split("\n")[0]
  const target = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find((t: any) => t.type === "page")
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise(r => ws.addEventListener("open", r, { once: true }))
  let id = 0
  const pending = new Map<number, (v: any) => void>()
  let plan = "", status = 0
  ws.addEventListener("message", e => {
    const m = JSON.parse(String(e.data))
    if (m.method === "Runtime.exceptionThrown") console.log("page exception:", m.params.exceptionDetails?.exception?.description ?? m.params.exceptionDetails?.text)
    if (m.method === "Network.responseReceived" && m.params.response.url.includes("/api/playtest/realms/plan")) { plan = m.params.requestId; status = m.params.response.status }
    pending.get(m.id)?.(m); pending.delete(m.id)
  })
  const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  const value = async (js: string) => (await send("Runtime.evaluate", { expression: js, returnByValue: true })).result?.result?.value
  const tap = async (x: number, y: number) => { for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 }) }
  await send("Runtime.enable"); await send("Page.enable"); await send("Network.enable")
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false })
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `try { localStorage.setItem("verse.voice.speaker", "web-5e0e0e0e0e0e") } catch {}` })
  await send("Network.setCookie", { name: "verse-automation", value: "voicecheck", url: origin })
  await send("Page.navigate", { url: `${origin}/realms/new` })
  for (let i = 0; i < 60 && !(await value("!!globalThis.coaxial?.ready")); i++) await Bun.sleep(500)
  await Bun.sleep(3000)
  // The five setup questions (builder.coax), the first answer each: its row starts at x 400 on a 1440x900 desk.
  for (let q = 0; q < 5; q++) { await tap(430, 421); await Bun.sleep(1200) }
  // TALK (the left of the two bottom buttons), the prompt, TALK again.
  await tap(603, 853)
  await Bun.sleep(7000)
  await tap(603, 853)
  const t0 = Date.now()
  while (Date.now() - t0 < 30000 && !plan) await Bun.sleep(250)
  await Bun.sleep(1500)
  const said: string[] = (await value("globalThis.coaxialVoice || []")) ?? []
  for (const s of said) console.log(`  ${s}`)
  const heard = said.find((s) => s.startsWith("heard|"))?.slice(6) ?? ""
  const body = plan ? (await send("Network.getResponseBody", { requestId: plan })).result?.body ?? "" : ""
  console.log(`  plan ${status}: ${body.split("\n").slice(0, 4).join(" / ").slice(0, 200)}`)
  if (process.env.SHOT) { const png = await send("Page.captureScreenshot", { format: "png" }); await Bun.write(process.env.SHOT, Buffer.from(png.result.data, "base64")) }
  ok = heard.length > 8 && status === 200 && body.trim().length > 0
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(ok ? "builder by voice: ok" : "builder by voice: FAILED")
process.exit(ok ? 0 : 1)
