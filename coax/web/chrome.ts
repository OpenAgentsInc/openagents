// The headless Chrome a page check runs, on this machine: its binary and the
// flags that give it WebGPU (goldens.ts, perf.ts; the Coax tools use
// coax/lib/net/cdp.coax, which makes the same choices). Always headless,
// muted and on its own profile; it never opens a window or takes focus.
//   macOS  Google Chrome.app, WebGPU on Metal through ANGLE
//   Linux  the real Chromium (never a launcher that opens a window, such as
//          CoderOS's coder-browser), WebGPU on Vulkan: on PATH, in the Nix
//          profiles, behind coder-browser, or the newest in /nix/store
// CHROME names the binary on either.
//
// Start one with launch(profile, flags): every check starts its Chrome there,
// and every Chrome started there is killed, and its profile removed, when the
// process exits, whether it ends, throws, or is stopped (SIGINT, SIGTERM,
// SIGHUP). Only SIGKILL escapes that; release.sh kills a check's whole
// process group and reaps headless Chromes whose parent is gone (reap-chrome.sh).
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const MAC = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
const NAMES = ["chromium", "chromium-browser", "google-chrome-stable", "google-chrome"]

// A script that hands off to a windowed browser is not a headless Chrome.
const launcher = (path: string) => { try { return /coder-browser|has not granted a browser/.test(readFileSync(path).subarray(0, 8192).toString("latin1")) } catch { return true } }
const file = (p: string) => { try { return statSync(p).isFile() } catch { return false } }

// Where a Linux browser lives: PATH, then the Nix profiles a shell on NixOS or
// with Nix may not have on its PATH (a host's non-login shell, a cron job).
export function linuxDirs(env: Record<string, string | undefined> = process.env): string[] {
  const user = env.USER ?? env.LOGNAME ?? ""
  return (env.PATH ?? "").split(":").filter(Boolean).concat(
    "/run/current-system/sw/bin",
    ...(env.HOME ? [join(env.HOME, ".nix-profile/bin")] : []),
    ...(user ? [`/etc/profiles/per-user/${user}/bin`] : []),
    "/nix/var/nix/profiles/default/bin")
}

// A Chromium or Google Chrome package in the Nix store with nothing on any PATH
// or profile pointing at it (`nix build nixpkgs#chromium` left it there): the
// newest version by its store name. Never chromium-unwrapped (no wrapper flags).
export function nixStoreBrowser(names: string[], has: (p: string) => boolean = file): string {
  const re = /^[0-9a-z]{32}-(chromium|google-chrome)-(\d[\w.]*)$/
  const ver = (v: string) => v.split(".").map(n => parseInt(n, 10) || 0)
  const newer = (a: string, b: string) => { const x = ver(a), y = ver(b); for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0); return false }
  let best = "", bestVer = ""
  for (const n of names) {
    const m = n.match(re); if (!m) continue
    const bin = join("/nix/store", n, "bin", m[1] === "chromium" ? "chromium" : "google-chrome-stable")
    if (has(bin) && (!best || newer(m[2], bestVer))) { best = bin; bestVer = m[2] }
  }
  return best
}

// The places linux() looked, for the message when it finds nothing.
const LOOKED = "PATH, /run/current-system/sw/bin, ~/.nix-profile/bin, /etc/profiles/per-user/$USER/bin, /nix/var/nix/profiles/default/bin, a coder-browser launcher and /nix/store"

function linux(): string {
  const dirs = linuxDirs()
  for (const dir of dirs)
    for (const n of NAMES) { const p = join(dir, n); if (file(p) && !launcher(p)) return p }
  // CoderOS shadows every browser name with its launcher; the launcher names the real Chromium it runs.
  for (const dir of dirs) {
    const p = join(dir, "coder-browser"); if (!existsSync(p)) continue
    const text = readFileSync(p, "utf8")
    const bin = text.match(/\/nix\/store\/[^/:"]*chromium[^/:"]*\/bin/)
    if (bin && file(join(bin[0], "chromium"))) return join(bin[0], "chromium")
    const gc = text.match(/\/nix\/store\/[^/:"]*google-chrome[^/:"]*\/bin/)
    if (gc && file(join(gc[0], "google-chrome-stable"))) return join(gc[0], "google-chrome-stable")
  }
  let store: string[] = []
  try { store = readdirSync("/nix/store") } catch {}
  const nix = nixStoreBrowser(store)
  if (nix) return nix
  throw Error(`chrome.ts: no headless Chromium or Google Chrome on this machine (looked in ${LOOKED}). ` +
    "Install one (nix profile install nixpkgs#chromium, or apt install chromium) or set CHROME=/path/to/chromium")
}

export function chrome(): string {
  if (process.env.CHROME) {
    const named = process.env.CHROME.includes("/") ? process.env.CHROME : Bun.which(process.env.CHROME) ?? ""
    if (!file(named)) throw Error(`chrome.ts: CHROME=${process.env.CHROME} is not a file${named ? "" : " or a program on PATH"}`)
    return named
  }
  if (process.platform !== "darwin") return linux()
  if (!file(MAC)) throw Error(`chrome.ts: no Google Chrome at ${MAC}; install it or set CHROME=/path/to/chrome`)
  return MAC
}

// `bun coax/web/chrome.ts`: the browser a page check would run, or why there is none (exit 1).
// release.sh asks this before it builds, so a machine without one stops at the start, saying so.
if (import.meta.main) {
  try { console.log(chrome()) } catch (e) { console.error((e as Error).message); process.exit(1) }
}

// WebGPU in headless Chrome: Metal on a Mac, Vulkan elsewhere.
export function gpuFlags(): string[] {
  return process.platform === "darwin"
    ? ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal"]
    // no Vulkan surface: headless, the GPU process crashes making one (cdp.coax, #12678)
    : ["--enable-unsafe-webgpu", "--enable-features=Vulkan,VulkanFromANGLE", "--use-angle=vulkan", "--ignore-gpu-blocklist", "--disable-vulkan-surface"]
}

// Every Chrome launch() started in this process, with its profile.
const started = new Set<{ proc: ReturnType<typeof Bun.spawn>, profile: string }>()
let hooked = false
function reap() {
  for (const c of started) {
    try { if (c.proc.exitCode === null && !c.proc.killed) process.kill(c.proc.pid, "SIGKILL") } catch {}
    try { rmSync(c.profile, { recursive: true, force: true }) } catch {}
  }
  started.clear()
}
function hook() {
  if (hooked) return
  hooked = true
  process.on("exit", reap)
  for (const [signal, n] of [["SIGINT", 2], ["SIGTERM", 15], ["SIGHUP", 1]] as const)
    process.on(signal, () => { reap(); process.exit(128 + n) })
}

// Headless Chrome on profile (a directory the caller made), its DevTools on a
// port it writes to profile/DevToolsActivePort, at about:blank. flags come
// after the common ones (window size, fake devices...). Killed with its
// profile removed when this process exits, however it exits.
export function launch(profile: string, flags: string[] = []): ReturnType<typeof Bun.spawn> {
  hook()
  const proc = Bun.spawn([chrome(), "--headless=new", "--mute-audio", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars",
    ...gpuFlags(), ...flags, "--remote-debugging-port=0", "about:blank"], { stdout: "ignore", stderr: "ignore" })
  started.add({ proc, profile })
  return proc
}

// A check whose Chrome would not start could not run: it says so and exits
// with this code (EX_TEMPFAIL), and release.sh never rolls a release back for
// it (2026-10-31: a release rolled back because its Chrome didn't start).
export const CANNOT_RUN = 75

// On Linux the first WebGPU device a fresh headless Chrome makes is lost: its GPU process starts
// again (Dawn says "A valid external Instance reference no longer exists") and that first page
// finds no adapter, so it shows "no WebGPU" (coderos-4080, Chrome 152, Vulkan). warmGpu makes one
// device on a blank page of its own (a secure context) and draws one cleared frame, until a device
// stays (at most 4 tries), then closes the tab, so the check's first page starts on a GPU that holds.
export async function warmGpu(port: string, tries = 4): Promise<boolean> {
  const page = Bun.serve({ port: 0, hostname: "127.0.0.1", fetch: () => new Response("<!doctype html>", { headers: { "content-type": "text/html" } }) })
  let ws: WebSocket | null = null, target = ""
  try {
    const t = await (await fetch(`http://127.0.0.1:${port}/json/new?http://127.0.0.1:${page.port}/`, { method: "PUT", signal: AbortSignal.timeout(5000) })).json()
    target = t.id
    ws = new WebSocket(t.webSocketDebuggerUrl)
    await new Promise((r, j) => { ws!.addEventListener("open", r, { once: true }); ws!.addEventListener("error", j, { once: true }) })
    let id = 0
    const pending = new Map<number, (v: any) => void>()
    ws.addEventListener("message", (e) => { const m = JSON.parse(String(e.data)); if (m.id) { pending.get(m.id)?.(m); pending.delete(m.id) } })
    const evaluate = (expression: string) => new Promise<any>((r) => { pending.set(++id, r); ws!.send(JSON.stringify({ id, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } })) })
    const once = `(async () => {
      if (!isSecureContext) return "insecure"
      const a = await navigator.gpu?.requestAdapter(); if (!a) return "no adapter"
      const d = await a.requestDevice(), c = new OffscreenCanvas(4, 4).getContext("webgpu")
      c.configure({ device: d, format: "bgra8unorm" })
      const e = d.createCommandEncoder()
      e.beginRenderPass({ colorAttachments: [{ view: c.getCurrentTexture().createView(), loadOp: "clear", storeOp: "store" }] }).end()
      d.queue.submit([e.finish()]); await d.queue.onSubmittedWorkDone()
      const held = await Promise.race([d.lost.then(() => "lost"), new Promise((r) => setTimeout(() => r("ok"), 300))])
      d.destroy(); return held
    })().catch((e) => "threw " + e.message)`
    for (let k = 0; k < tries; k++) {
      await Bun.sleep(k ? 500 : 200)
      if ((await evaluate(once)).result?.result?.value === "ok") return true
    }
    return false
  } catch { return false } finally {
    try { ws?.close() } catch {}
    if (target) await fetch(`http://127.0.0.1:${port}/json/close/${target}`).catch(() => {})
    page.stop(true)
  }
}

// launch(), waited for: up to `tries` Chromes, each on a fresh profile, until
// one says its DevTools port and answers on it. One that doesn't is killed
// (only that one: its own pid) with its profile, and another started. When
// none starts, the check could not run: "could not run: ..." and CANNOT_RUN.
// CHROME_START_WAIT_MS bounds each wait (30 s).
export async function startChrome(prefix: string, flags: string[] = [], tries = 3): Promise<{ proc: ReturnType<typeof Bun.spawn>, profile: string, port: string }> {
  const wait = Number(process.env.CHROME_START_WAIT_MS ?? 30000)
  try { chrome() } catch (e) { console.error(`could not run: ${(e as Error).message}`); process.exit(CANNOT_RUN) }
  for (let attempt = 1; attempt <= tries; attempt++) {
    const profile = mkdtempSync(join(tmpdir(), prefix))
    const proc = launch(profile, flags)
    const portFile = join(profile, "DevToolsActivePort")
    const port = () => { try { return readFileSync(portFile, "utf8").split("\n")[0].trim() } catch { return "" } }
    const until = Date.now() + wait
    while (Date.now() < until && proc.exitCode === null) {
      if (/^\d+$/.test(port())) {
        let up = false
        try { up = (await fetch(`http://127.0.0.1:${port()}/json/version`, { signal: AbortSignal.timeout(2000) })).ok } catch {}
        if (up) { if (process.platform !== "darwin") await warmGpu(port()); return { proc, profile, port: port() } }
      }
      await Bun.sleep(50)
    }
    try { if (proc.exitCode === null) process.kill(proc.pid, "SIGKILL") } catch {}
    for (const c of started) if (c.proc === proc) started.delete(c)
    try { rmSync(profile, { recursive: true, force: true }) } catch {}
    console.error(`chrome.ts: headless Chrome didn't start (try ${attempt} of ${tries})`)
  }
  console.error(`could not run: headless Chrome did not start (${tries} tries)`)
  reap()
  process.exit(CANNOT_RUN)
}
