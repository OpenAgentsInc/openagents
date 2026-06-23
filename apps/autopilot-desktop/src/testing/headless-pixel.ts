// Headless deterministic pixel verification (first-class render testing).
//
// WHY: model-shape tests ("the beam is in the visualization") pass while the
// scene renders NOTHING. A beam can be in the model yet invisible: the renderer
// gates it by `motionAllowedByPolicy` (evidence:required), resolves positions
// from entity ids, and only advances on frame ticks. None of that is observable
// from the model. This helper RUNS the real three-effect element and OBSERVES
// real pixels — the same "execute + observe" model as the inference
// acceptance-runner, generalized for the desktop render path.
//
// DETERMINISM: the three-effect element builds its loop with
// createManagedFrameClock({ mode: "always" }), which calls the GLOBAL
// requestAnimationFrame + performance.now. Before the element mounts we replace
// those globals with a DRIVER-CONTROLLED fake clock, then advance EXACTLY N
// frames of EXACTLY `frameDeltaMs` each from a fixed start time. No real rAF, no
// wall-clock — identical pixels every run. We do this WITHOUT modifying
// three-effect, because the element already reads the injectable globals.
//
// CONSTRAINTS: Electrobun's native bridge is not faithfully headless, so we
// drive the element directly in headless Chromium (WebGL under SwiftShader, no
// GPU). #5026 (bun load-hang) is irrelevant here — this is a `bun run` script /
// per-file test, not the bare full-suite `bun test`.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { join } from "node:path"
import { inflateSync } from "node:zlib"
import net from "node:net"

const CHROME_CANDIDATES = [
  process.env.CHROME_PATH,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/opt/google/chrome/chrome",
].filter((candidate): candidate is string => candidate !== undefined)

// Resolve a Chromium-family binary, or null when none is installed (so callers
// can skip cleanly in environments without a browser).
export const resolveChromePathOrNull = (): string | null =>
  CHROME_CANDIDATES.find((candidate) => existsSync(candidate)) ?? null

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

const getFreePort = (): Promise<number> =>
  new Promise((resolve, reject) => {
    const server = net.createServer()
    server.unref()
    server.on("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      if (address === null || typeof address === "string") {
        server.close(() => reject(new Error("failed to allocate port")))
        return
      }
      const { port } = address
      server.close((error) => {
        if (error !== undefined) reject(error)
        else resolve(port)
      })
    })
  })

type CdpMessage = {
  readonly id?: number
  readonly result?: unknown
  readonly error?: { readonly message?: string }
}

type CdpClient = {
  readonly close: () => void
  readonly send: <Result>(
    method: string,
    params?: Record<string, unknown>,
  ) => Promise<Result>
}

const connectCdp = (webSocketUrl: string): Promise<CdpClient> =>
  new Promise((resolve, reject) => {
    const socket = new WebSocket(webSocketUrl)
    let nextId = 1
    const pending = new Map<
      number,
      {
        readonly resolve: (value: unknown) => void
        readonly reject: (error: Error) => void
      }
    >()
    socket.addEventListener("open", () => {
      resolve({
        close: () => socket.close(),
        send: <Result>(
          method: string,
          params: Record<string, unknown> = {},
        ): Promise<Result> => {
          const id = nextId++
          return new Promise<Result>((messageResolve, messageReject) => {
            pending.set(id, {
              resolve: (value) => messageResolve(value as Result),
              reject: messageReject,
            })
            socket.send(JSON.stringify({ id, method, params }))
          })
        },
      })
    })
    socket.addEventListener("error", () => {
      reject(new Error(`failed to connect to Chrome CDP at ${webSocketUrl}`))
    })
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data)) as CdpMessage
      if (message.id === undefined) return
      const handler = pending.get(message.id)
      if (handler === undefined) return
      pending.delete(message.id)
      if (message.error !== undefined) {
        handler.reject(new Error(message.error.message ?? "CDP command failed"))
        return
      }
      handler.resolve(message.result)
    })
  })

const waitForPageWebSocket = async (debugPort: number): Promise<string> => {
  const deadline = Date.now() + 10_000
  let lastError: unknown = null
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`)
      if (response.ok) {
        const targets = (await response.json()) as Array<{
          readonly type?: string
          readonly webSocketDebuggerUrl?: string
        }>
        const page = targets.find(
          (target) =>
            target.type === "page" &&
            typeof target.webSocketDebuggerUrl === "string",
        )
        if (page?.webSocketDebuggerUrl !== undefined) {
          return page.webSocketDebuggerUrl
        }
      }
    } catch (error) {
      lastError = error
    }
    await wait(100)
  }
  throw new Error(`Chrome CDP page target unavailable: ${String(lastError)}`)
}

// ── PNG decode (no image deps) ────────────────────────────────────────────────

export type PngImage = {
  readonly data: Uint8Array
  readonly height: number
  readonly width: number
}

const paethPredictor = (left: number, up: number, upLeft: number): number => {
  const estimate = left + up - upLeft
  const leftDistance = Math.abs(estimate - left)
  const upDistance = Math.abs(estimate - up)
  const upLeftDistance = Math.abs(estimate - upLeft)
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) return left
  if (upDistance <= upLeftDistance) return up
  return upLeft
}

export const decodePng = (png: Buffer): PngImage => {
  if (png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
    throw new Error("Chrome screenshot was not a PNG")
  }
  let offset = 8
  let width = 0
  let height = 0
  let colorType = 0
  const idatChunks: Buffer[] = []
  while (offset < png.length) {
    const length = png.readUInt32BE(offset)
    const type = png.subarray(offset + 4, offset + 8).toString("ascii")
    const data = png.subarray(offset + 8, offset + 8 + length)
    offset += 12 + length
    if (type === "IHDR") {
      width = data.readUInt32BE(0)
      height = data.readUInt32BE(4)
      const bitDepth = data[8] ?? 0
      colorType = data[9] ?? 0
      if (bitDepth !== 8) throw new Error(`unsupported PNG bit depth ${bitDepth}`)
      if (colorType !== 2 && colorType !== 6) {
        throw new Error(`unsupported PNG color type ${colorType}`)
      }
      if ((data[12] ?? 0) !== 0) throw new Error("interlaced PNG unsupported")
    } else if (type === "IDAT") {
      idatChunks.push(Buffer.from(data))
    } else if (type === "IEND") {
      break
    }
  }
  if (width <= 0 || height <= 0 || idatChunks.length === 0) {
    throw new Error("invalid PNG screenshot")
  }
  const bytesPerPixel = colorType === 6 ? 4 : 3
  const stride = width * bytesPerPixel
  const inflated = inflateSync(Buffer.concat(idatChunks))
  const output = new Uint8Array(width * height * 4)
  let inputOffset = 0
  let previous = new Uint8Array(stride)
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[inputOffset]
    inputOffset += 1
    const row = new Uint8Array(stride)
    for (let x = 0; x < stride; x += 1) {
      const raw = inflated[inputOffset + x] ?? 0
      const left = x >= bytesPerPixel ? row[x - bytesPerPixel] ?? 0 : 0
      const up = previous[x] ?? 0
      const upLeft = x >= bytesPerPixel ? previous[x - bytesPerPixel] ?? 0 : 0
      switch (filter) {
        case 0: row[x] = raw; break
        case 1: row[x] = (raw + left) & 0xff; break
        case 2: row[x] = (raw + up) & 0xff; break
        case 3: row[x] = (raw + Math.floor((left + up) / 2)) & 0xff; break
        case 4: row[x] = (raw + paethPredictor(left, up, upLeft)) & 0xff; break
        default: throw new Error(`unsupported PNG filter ${filter}`)
      }
    }
    inputOffset += stride
    for (let x = 0; x < width; x += 1) {
      const source = x * bytesPerPixel
      const target = (y * width + x) * 4
      output[target] = row[source] ?? 0
      output[target + 1] = row[source + 1] ?? 0
      output[target + 2] = row[source + 2] ?? 0
      output[target + 3] = colorType === 6 ? row[source + 3] ?? 255 : 255
    }
    previous = row
  }
  return { data: output, height, width }
}

// A fractional region of the frame to score, in [0,1]. Defaults to the whole
// frame. Use a sub-region to assert a SPECIFIC effect lit up where it should.
export type PixelRegion = Readonly<{ x0: number; y0: number; x1: number; y1: number }>

const FULL_FRAME: PixelRegion = { x0: 0, y0: 0, x1: 1, y1: 1 }

export type RegionScore = Readonly<{
  brightPixels: number
  distinctLumaBuckets: number
  sampledPixels: number
}>

export const scoreRegion = (
  image: PngImage,
  region: PixelRegion,
): RegionScore => {
  const px0 = Math.max(0, Math.floor(region.x0 * image.width))
  const py0 = Math.max(0, Math.floor(region.y0 * image.height))
  const px1 = Math.min(image.width, Math.ceil(region.x1 * image.width))
  const py1 = Math.min(image.height, Math.ceil(region.y1 * image.height))
  let brightPixels = 0
  let sampledPixels = 0
  const buckets = new Set<number>()
  for (let y = py0; y < py1; y += 1) {
    for (let x = px0; x < px1; x += 1) {
      const offset = (y * image.width + x) * 4
      const luma =
        (image.data[offset] ?? 0) +
        (image.data[offset + 1] ?? 0) +
        (image.data[offset + 2] ?? 0)
      if (luma > 80) brightPixels += 1
      buckets.add(Math.floor(luma / 32))
      sampledPixels += 1
    }
  }
  return { brightPixels, distinctLumaBuckets: buckets.size, sampledPixels }
}

// Count BRIGHT, CYAN/BLUE-DOMINANT pixels in a region — the crackling-arc strands
// are bright near-white/cyan (≈ #93c5fd / #f8fafc) on a near-black world, so this
// isolates the arc from the dim grey street geometry and the warm pylon/board
// tones. Used by the real-scene render regression (app-replica.test.ts) to assert
// the spawned arc actually lit up where the avatar is looking — the exact check
// the false #6044 "isolated primitive" proof never ran against the full scene.
export const scoreCracklingArcRegion = (
  image: PngImage,
  region: PixelRegion = FULL_FRAME,
): number => {
  const px0 = Math.max(0, Math.floor(region.x0 * image.width))
  const py0 = Math.max(0, Math.floor(region.y0 * image.height))
  const px1 = Math.min(image.width, Math.ceil(region.x1 * image.width))
  const py1 = Math.min(image.height, Math.ceil(region.y1 * image.height))
  let arcPixels = 0
  for (let y = py0; y < py1; y += 1) {
    for (let x = px0; x < px1; x += 1) {
      const offset = (y * image.width + x) * 4
      const r = image.data[offset] ?? 0
      const g = image.data[offset + 1] ?? 0
      const b = image.data[offset + 2] ?? 0
      // Bright AND blue-leaning (b dominates red; green present for the near-white
      // strands) — the arc's cyan/white palette, never the warm/grey world.
      if (b > 150 && b >= r && g > 110) arcPixels += 1
    }
  }
  return arcPixels
}

export type HeadlessRenderOptions = Readonly<{
  // Absolute path to a TS module that, when imported, mounts an `oa-training-run`
  // element into `#scene` and sets its `.visualization`. (See the regression
  // script for an example.) It runs AFTER the deterministic clock is installed.
  entryModulePath: string
  width?: number
  height?: number
  // Number of FIXED deterministic frames to advance. No rAF / wall-clock.
  frameSteps?: number
  // Milliseconds advanced per fixed frame.
  frameDeltaMs?: number
  // Optional query string (no leading "?") appended to the page URL so the entry
  // module can branch on `location.search` (e.g. a deliberately-broken variant).
  pageQuery?: string
}>

export type HeadlessRenderResult = Readonly<{
  canvasWidth: number
  canvasHeight: number
  framesAdvanced: number
  image: PngImage
  screenshotBase64: string
  // Score a fractional region of the captured frame.
  score: (region?: PixelRegion) => RegionScore
}>

// The page bootstrap. It installs a deterministic fake clock OVER the global
// requestAnimationFrame + performance.now BEFORE the entry module imports (and
// thus before three-effect's "always" frame clock captures them), exposes a
// `__stepFrames(n, deltaMs)` driver, then imports the entry module.
const pageBootstrap = (): string => `
let __fakeNow = 0
const __rafQueue = []
performance.now = () => __fakeNow
globalThis.requestAnimationFrame = (cb) => {
  const id = __rafQueue.length + 1
  __rafQueue.push({ id, cb })
  return id
}
globalThis.cancelAnimationFrame = (id) => {
  const idx = __rafQueue.findIndex((entry) => entry.id === id)
  if (idx >= 0) __rafQueue.splice(idx, 1)
}
// Drive EXACTLY n frames; each advances the fake clock by a fixed delta and
// flushes the rAF queue that was pending at the start of that frame (the loop
// re-registers itself for the next frame, matching how the managed clock works).
globalThis.__stepFrames = (n, deltaMs) => {
  for (let i = 0; i < n; i += 1) {
    __fakeNow += deltaMs
    const pending = __rafQueue.splice(0, __rafQueue.length)
    for (const entry of pending) entry.cb(__fakeNow)
  }
}
globalThis.__pendingFrameCount = () => __rafQueue.length
`

const pageHtml = (width: number, height: number): string => `
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Headless render harness</title>
    <style>
      :root { color-scheme: dark; background: #050505; }
      body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #050505; }
      #scene { width: ${width}px; height: ${height}px; background: #050505; }
    </style>
    <script>${pageBootstrap()}</script>
  </head>
  <body>
    <div id="scene"></div>
    <script type="module" src="/dist/headless-render-entry.js"></script>
  </body>
</html>
`

// Mount the entry module's visualization in headless Chromium, advance a fixed
// number of deterministic frames, screenshot, and return the decoded image plus
// a region scorer. Throws if the canvas never mounts.
export const renderVisualizationAndProbe = async (
  options: HeadlessRenderOptions,
): Promise<HeadlessRenderResult> => {
  const chromePath = resolveChromePathOrNull()
  if (chromePath === null) {
    throw new Error(
      "Chrome/Chromium not found. Set CHROME_PATH to a Chromium-family binary.",
    )
  }
  const width = options.width ?? 960
  const height = options.height ?? 540
  const frameSteps = options.frameSteps ?? 90
  const frameDeltaMs = options.frameDeltaMs ?? 16

  const tmpRoot = mkdtempSync(join(process.cwd(), ".headless-render-"))
  const userDataDir = join(tmpRoot, "chrome-profile")
  const distDir = join(tmpRoot, "dist")
  const htmlPath = join(tmpRoot, "index.html")
  mkdirSync(distDir, { recursive: true })
  mkdirSync(userDataDir, { recursive: true })
  writeFileSync(htmlPath, pageHtml(width, height))

  let server: ReturnType<typeof Bun.serve> | null = null
  let chrome: Bun.Subprocess | null = null
  let cdp: CdpClient | null = null
  try {
    const build = Bun.spawnSync({
      cmd: [
        process.execPath,
        "build",
        options.entryModulePath,
        "--outdir",
        distDir,
        "--target",
        "browser",
        "--format",
        "esm",
      ],
      stderr: "pipe",
      stdout: "pipe",
    })
    if (!build.success) {
      const stderr = new TextDecoder().decode(build.stderr)
      if (stderr.trim().length > 0) console.error(stderr)
      throw new Error("failed to build headless render entry bundle")
    }
    // The bundle is emitted under the entry's basename; serve it as the fixed
    // path the HTML references.
    const builtName = options.entryModulePath
      .split("/")
      .pop()!
      .replace(/\.ts$/, ".js")

    server = Bun.serve({
      hostname: "127.0.0.1",
      port: 0,
      fetch(request) {
        const url = new URL(request.url)
        if (url.pathname === "/") {
          return new Response(readFileSync(htmlPath), {
            headers: { "content-type": "text/html; charset=utf-8" },
          })
        }
        if (url.pathname === "/dist/headless-render-entry.js") {
          return new Response(readFileSync(join(distDir, builtName)), {
            headers: { "content-type": "text/javascript; charset=utf-8" },
          })
        }
        return new Response("not found", { status: 404 })
      },
    })

    const debugPort = await getFreePort()
    const query =
      options.pageQuery === undefined || options.pageQuery.length === 0
        ? ""
        : `?${options.pageQuery}`
    const url = `http://127.0.0.1:${server.port}/${query}`
    chrome = Bun.spawn({
      cmd: [
        chromePath,
        "--headless=new",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-background-networking",
        "--disable-extensions",
        "--disable-sync",
        "--enable-webgl",
        "--ignore-gpu-blocklist",
        "--use-angle=swiftshader",
        "--enable-unsafe-swiftshader",
        `--window-size=${width},${height}`,
        `--remote-debugging-port=${debugPort}`,
        `--user-data-dir=${userDataDir}`,
        url,
      ],
      stdout: "pipe",
      stderr: "pipe",
    })

    const pageWebSocket = await waitForPageWebSocket(debugPort)
    cdp = await connectCdp(pageWebSocket)
    await cdp.send("Runtime.enable")
    await cdp.send("Page.enable")
    await cdp.send("Page.navigate", { url })

    // Wait for the custom element to define + mount its canvas (NOT for an
    // animation — frames are driven below). Polling, not rAF, so this is robust.
    const mountProbe = await cdp.send<{
      readonly result?: { readonly value?: { ok: boolean; w: number; h: number } }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: `(async () => {
        await customElements.whenDefined("oa-training-run")
        const host = document.querySelector("oa-training-run")
        // Give synchronous mount a moment; the canvas mounts inside connectedCallback.
        for (let i = 0; i < 200; i += 1) {
          const c = host?.shadowRoot?.querySelector("canvas")
          if (c instanceof HTMLCanvasElement && c.width > 0) {
            return { ok: true, w: c.width, h: c.height }
          }
          await new Promise((r) => setTimeout(r, 10))
        }
        return { ok: false, w: 0, h: 0 }
      })()`,
      awaitPromise: true,
      returnByValue: true,
    })
    if (mountProbe.exceptionDetails !== undefined) {
      throw new Error(
        `headless render threw at mount: ${JSON.stringify(mountProbe.exceptionDetails)}`,
      )
    }
    const mount = mountProbe.result?.value
    if (mount === undefined || !mount.ok) {
      throw new Error("oa-training-run canvas never mounted")
    }

    // DETERMINISTIC FRAME STEPPING: advance exactly `frameSteps` fixed frames.
    await cdp.send("Runtime.evaluate", {
      expression: `__stepFrames(${frameSteps}, ${frameDeltaMs})`,
      awaitPromise: false,
      returnByValue: true,
    })

    const screenshot = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const image = decodePng(Buffer.from(screenshot.data, "base64"))
    return {
      canvasWidth: mount.w,
      canvasHeight: mount.h,
      framesAdvanced: frameSteps,
      image,
      screenshotBase64: screenshot.data,
      score: (region: PixelRegion = FULL_FRAME) => scoreRegion(image, region),
    }
  } finally {
    cdp?.close()
    if (chrome !== null) {
      chrome.kill()
      await chrome.exited.catch(() => null)
    }
    server?.stop(true)
    rmSync(tmpRoot, { force: true, recursive: true })
  }
}
