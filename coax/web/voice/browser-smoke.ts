// The Haunted House's VOICE in headless Chrome, end to end: Chrome's fake
// microphone plays a recorded question, a real click turns VOICE on, and the
// page must hear it and play the house's reply. Nothing is heard on this
// Mac (--mute-audio) and no window opens.
// TALK=NAME: first turn to Mortimer, near where you arrive in the yard, and
// press E to talk to him; NAME must then answer, through the same character
// chat as a typed question, and the voice host must speak it as NAME.
//   [TALK=NAME] bun coax/web/voice/browser-smoke.ts PAGE_URL [FIXTURE.wav]
// PAGE_URL is the Haunted House, with ?voice=ws://127.0.0.1:8787 for a host on this Mac.
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

const [url, wav = "verse/voice-host/fixtures/hello-foreman.wav"] = process.argv.slice(2)
const W = 1440, H = 900
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const profile = mkdtempSync(join(tmpdir(), "coax-voice-"))
const proc = Bun.spawn([chrome, "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run",
  // The audio service's sandbox can't read the fixture: without this the fake microphone is silent.
  "--disable-features=AudioServiceSandbox,AudioServiceOutOfProcess",
  "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal", `--window-size=${W},${H}`,
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
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false })
  // One speaker id for every run: the host meters speakers per day (--max-speakers), and a fresh
  // profile's random id would take a visitor's place each time.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `try { localStorage.setItem("verse.voice.speaker", "web-5e0e0e0e0e0e") } catch {}` })
  await send("Page.navigate", { url })
  for (let i = 0; i < 60 && !(await value("!!globalThis.coaxial?.ready")); i++) await Bun.sleep(500)
  await Bun.sleep(3000)
  const talk = process.env.TALK ?? ""
  if (talk) {
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
  // VOICE, the hotbar's sixth slot of seven (realm.coax slot_box on a 1440x900 desk).
  const x = W - 16 - 4 - 1 * (76 + 12) - 38, y = H - 16 - 4 - 20
  for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 })
  if (process.env.SHOT) { await Bun.sleep(1500); const png = await send("Page.captureScreenshot", { format: "png" }); await Bun.write(process.env.SHOT, Buffer.from(png.result.data, "base64")) }
  const t0 = Date.now()
  let said: string[] = []
  while (Date.now() - t0 < 40000) {
    said = (await value("globalThis.coaxialVoice || []")) ?? []
    if (said.some((s) => s.startsWith("replying|")) && said.at(-1)?.startsWith("listening|")) break
    if (said.at(-1)?.startsWith("notice|")) break
    await Bun.sleep(250)
  }
  for (const s of said) console.log(`  ${s}`)
  const who = (await value("globalThis.coaxialVoiceWho || ''")) ?? ""
  if (who) console.log(`  spoken by ${who}`)
  ok = said.some((s) => (s.startsWith("heard|") || s.startsWith("thinking|")) && s.length > 9) && said.some((s) => s.startsWith("replying|") && s.length > 12) && (!talk || who === talk)
  ws.close()
} finally {
  proc.kill()
  rmSync(profile, { recursive: true, force: true })
}
console.log(ok ? "voice in the browser: ok" : "voice in the browser: FAILED")
process.exit(ok ? 0 : 1)
