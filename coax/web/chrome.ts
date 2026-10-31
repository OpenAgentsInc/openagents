// The headless Chrome a page check runs, on this machine: its binary and the
// flags that give it WebGPU (goldens.ts, perf.ts; the Coax tools use
// coax/lib/net/cdp.coax, which makes the same choices). Always headless,
// muted and on its own profile; it never opens a window or takes focus.
//   macOS  Google Chrome.app, WebGPU on Metal through ANGLE
//   Linux  the real Chromium (never a launcher that opens a window, such as
//          CoderOS's coder-browser), WebGPU on Vulkan
// CHROME names the binary on either.
//
// Start one with launch(profile, flags): every check starts its Chrome there,
// and every Chrome started there is killed, and its profile removed, when the
// process exits, whether it ends, throws, or is stopped (SIGINT, SIGTERM,
// SIGHUP). Only SIGKILL escapes that; release.sh kills a check's whole
// process group and reaps headless Chromes whose parent is gone (reap-chrome.sh).
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const MAC = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

// A script that hands off to a windowed browser is not a headless Chrome.
const launcher = (path: string) => { try { return /coder-browser|has not granted a browser/.test(readFileSync(path).subarray(0, 8192).toString("latin1")) } catch { return true } }

function linux(): string {
  const names = ["chromium", "chromium-browser", "google-chrome-stable", "google-chrome"]
  for (const dir of (process.env.PATH ?? "").split(":").concat("/run/current-system/sw/bin"))
    for (const n of names) { const p = join(dir, n); if (existsSync(p) && !launcher(p)) return p }
  // CoderOS shadows every browser name with its launcher; the launcher names the real Chromium it runs.
  for (const dir of (process.env.PATH ?? "").split(":").concat("/run/current-system/sw/bin")) {
    const p = join(dir, "coder-browser"); if (!existsSync(p)) continue
    const bin = readFileSync(p, "utf8").match(/\/nix\/store\/[^/:"]*chromium[^/:"]*\/bin/)
    if (bin && existsSync(join(bin[0], "chromium"))) return join(bin[0], "chromium")
  }
  throw Error("chrome.ts: no headless Chromium on this machine; set CHROME")
}

export function chrome(): string {
  if (process.env.CHROME) return process.env.CHROME
  return process.platform === "darwin" ? MAC : linux()
}

// WebGPU in headless Chrome: Metal on a Mac, Vulkan elsewhere.
export function gpuFlags(): string[] {
  return process.platform === "darwin"
    ? ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=metal"]
    : ["--enable-unsafe-webgpu", "--enable-features=Vulkan,VulkanFromANGLE", "--use-angle=vulkan", "--ignore-gpu-blocklist"]
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

// launch(), waited for: up to `tries` Chromes, each on a fresh profile, until
// one says its DevTools port and answers on it. One that doesn't is killed
// (only that one: its own pid) with its profile, and another started. When
// none starts, the check could not run: "could not run: ..." and CANNOT_RUN.
// CHROME_START_WAIT_MS bounds each wait (30 s).
export async function startChrome(prefix: string, flags: string[] = [], tries = 3): Promise<{ proc: ReturnType<typeof Bun.spawn>, profile: string, port: string }> {
  const wait = Number(process.env.CHROME_START_WAIT_MS ?? 30000)
  for (let attempt = 1; attempt <= tries; attempt++) {
    const profile = mkdtempSync(join(tmpdir(), prefix))
    const proc = launch(profile, flags)
    const portFile = join(profile, "DevToolsActivePort")
    const port = () => { try { return readFileSync(portFile, "utf8").split("\n")[0].trim() } catch { return "" } }
    const until = Date.now() + wait
    while (Date.now() < until && proc.exitCode === null) {
      if (/^\d+$/.test(port())) {
        try { if ((await fetch(`http://127.0.0.1:${port()}/json/version`, { signal: AbortSignal.timeout(2000) })).ok) return { proc, profile, port: port() } } catch {}
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
