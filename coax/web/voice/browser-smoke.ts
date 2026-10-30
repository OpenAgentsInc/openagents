// The Haunted House's VOICE in headless Chrome, end to end: Chrome's fake
// microphone plays a recorded question, a real click turns VOICE on, and the
// page must hear it and play the house's reply. Nothing is heard on this
// Mac (--mute-audio) and no window opens.
// TALK=NAME: first turn to Mortimer, near where you arrive in the yard, and
// press E to talk to him; NAME must then answer, through the same character
// chat as a typed question, and the voice host must speak it as NAME.
// PHONE=1: the same at 390x844 with touch: with TALK, a look to the left
// and a tap on Mortimer opens the talk pane (ptr-tap-talk), then the
// microphone under the minimap is tapped.
// OFF=1: VOICE is never tapped (with TALK, after a conversation starts):
// nothing may ask for the microphone or open a voice socket.
// Its sessions are marked automation (cookie verse-automation=voicecheck), so
// the analytics leave them out.
//   [TALK=NAME] [PHONE=1] [OFF=1] bun coax/web/voice/browser-smoke.ts PAGE_URL [FIXTURE.wav]
// PAGE_URL is the Haunted House, with ?voice=ws://127.0.0.1:8787 for a host on this Mac.
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { chrome as chromePath, gpuFlags } from "../chrome"

const [url, wav = "verse/voice-host/fixtures/hello-foreman.wav"] = process.argv.slice(2)
const PHONE = !!process.env.PHONE
const W = PHONE ? 390 : 1440, H = PHONE ? 844 : 900
const chrome = chromePath()
const profile = mkdtempSync(join(tmpdir(), "coax-voice-"))
const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run",
  // The audio service's sandbox can't read the fixture: without this the fake microphone is silent.
  "--disable-features=AudioServiceSandbox,AudioServiceOutOfProcess",
  ...gpuFlags(), `--window-size=${W},${H}`,
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
  ws.addEventListener("message", e => {
    const m = JSON.parse(String(e.data))
    if (m.method === "Runtime.exceptionThrown") console.log("page exception:", m.params.exceptionDetails?.exception?.description ?? m.params.exceptionDetails?.text)
    if (m.method === "Runtime.consoleAPICalled" && m.params.type !== "log") console.log(`page ${m.params.type}:`, m.params.args.map((a: any) => a.value ?? a.description).join(" "))
    pending.get(m.id)?.(m); pending.delete(m.id)
  })
  const send = (method: string, params: any = {}) => new Promise<any>(r => { pending.set(++id, r); ws.send(JSON.stringify({ id, method, params })) })
  const value = async (js: string) => (await send("Runtime.evaluate", { expression: js, returnByValue: true })).result?.result?.value
  await send("Runtime.enable")
  await send("Page.enable")   // without it the scripts below never run
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: PHONE ? 3 : 1, mobile: PHONE })
  if (PHONE) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 })
  const touch = async (points: { x: number, y: number }[]) => {
    await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ ...points[0], id: 1 }] })
    for (const p of points.slice(1)) { await send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ ...p, id: 1 }] }); await Bun.sleep(16) }
    await Bun.sleep(60)
    await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] })
  }
  // One speaker id for every run: the host meters speakers per day (--max-speakers), and a fresh
  // profile's random id would take a visitor's place each time.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `try { localStorage.setItem("verse.voice.speaker", "web-5e0e0e0e0e0e") } catch {}` })
  // What asks for the microphone or opens a voice socket, counted (OFF=1).
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `(() => { globalThis.micAsks = 0; const md = MediaDevices.prototype, g = md.getUserMedia; md.getUserMedia = function (c) { globalThis.micAsks++; return g.call(this, c) }; globalThis.voiceSockets = 0; const W = globalThis.WebSocket; globalThis.WebSocket = class extends W { constructor(u, p) { super(u, p); if (String(u).includes("voice")) globalThis.voiceSockets++ } } })()` })
  await send("Network.enable")
  await send("Network.setCookie", { name: "verse-automation", value: "voicecheck", url: new URL(url).origin })
  await send("Page.navigate", { url })
  for (let i = 0; i < 60 && !(await value("!!globalThis.coaxial?.ready")); i++) await Bun.sleep(500)
  await Bun.sleep(3000)
  const talk = process.env.TALK ?? ""
  if (talk && PHONE) {
    // Look left (a 100 px drag clear of the stick) until Mortimer stands at the screen's left edge, then tap him.
    await touch(Array.from({ length: 11 }, (_, i) => ({ x: 200 - 10 * i, y: 400 })))
    await Bun.sleep(600)
    await touch([{ x: 40, y: 370 }])
    await Bun.sleep(1500)
    if (process.env.SHOT) { const png = await send("Page.captureScreenshot", { format: "png" }); await Bun.write(process.env.SHOT.replace(/\.png$/, "-talk.png"), Buffer.from(png.result.data, "base64")) }
  } else if (talk) {
    // Look around to the right until Mortimer, by the path behind you, is near enough to talk to (800 px of drag at 1440x900).
    for (let k = 0; k < 4; k++) {
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x: 700, y: 400, button: "left", clickCount: 1 })
      for (let i = 1; i <= 10; i++) await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 700 + 20 * i, y: 400, button: "left", buttons: 1 })
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 900, y: 400, button: "left", clickCount: 1 })
      await Bun.sleep(700)
    }
    for (const type of ["keyDown", "keyUp"]) await send("Input.dispatchKeyEvent", { type, key: "e", code: "KeyE", text: type === "keyDown" ? "e" : undefined, windowsVirtualKeyCode: 69 })
    await Bun.sleep(800)
  }
  if (process.env.OFF) {
    await Bun.sleep(5000)
    const n = await value("[globalThis.micAsks, globalThis.voiceSockets]")
    console.log(`voice off: ${n?.[0]} microphone asks, ${n?.[1]} voice sockets`)
    ok = n?.[0] === 0 && n?.[1] === 0
    throw "done"
  }
  // VOICE, the microphone circle (realm.coax voice_circle): bottom right on a desk, under the minimap on a phone.
  if (PHONE) await touch([{ x: 347, y: 164 }])
  else { const x = W - 16 - 4 - 28, y = H - 16 - 4 - 28; for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 }) }
  if (process.env.SHOT) { await Bun.sleep(1500); const png = await send("Page.captureScreenshot", { format: "png" }); await Bun.write(process.env.SHOT, Buffer.from(png.result.data, "base64")) }
  const t0 = Date.now()
  let said: string[] = []
  while (Date.now() - t0 < 40000) {
    said = (await value("globalThis.coaxialVoice || []")) ?? []
    if (said.some((s) => s.startsWith("replying|")) && /^(listening|off)\|/.test(said.at(-1) ?? "")) break
    if (said.at(-1)?.startsWith("notice|")) break
    await Bun.sleep(250)
  }
  for (const s of said) console.log(`  ${s}`)
  const who = (await value("globalThis.coaxialVoiceWho || ''")) ?? ""
  if (who) console.log(`  spoken by ${who}`)
  ok = said.some((s) => (s.startsWith("heard|") || s.startsWith("thinking|")) && s.length > 9) && said.some((s) => s.startsWith("replying|") && s.length > 12) && (!talk || who === talk)
  ws.close()
} catch (e) {
  if (e !== "done") throw e
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(ok ? "voice in the browser: ok" : "voice in the browser: FAILED")
process.exit(ok ? 0 : 1)
