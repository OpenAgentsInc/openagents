// The headless Chrome a page check runs, on this machine: its binary and the
// flags that give it WebGPU (goldens.ts, perf.ts; the Coax tools use
// coax/lib/net/cdp.coax, which makes the same choices). Always headless,
// muted and on its own profile; it never opens a window or takes focus.
//   macOS  Google Chrome.app, WebGPU on Metal through ANGLE
//   Linux  the real Chromium (never a launcher that opens a window, such as
//          CoderOS's coder-browser), WebGPU on Vulkan
// CHROME names the binary on either.
import { existsSync, readFileSync } from "node:fs"
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
