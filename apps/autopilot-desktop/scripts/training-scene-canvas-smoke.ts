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

const chromeCandidates = [
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

const resolveChromePath = (): string => {
  const chromePath = chromeCandidates.find(candidate => existsSync(candidate))
  if (chromePath !== undefined) return chromePath

  throw new Error(
    "Chrome executable not found. Set CHROME_PATH to a Chrome, Chromium, or Edge binary before running smoke:training-scene.",
  )
}

const wait = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

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
      server.close(error => {
        if (error !== undefined) reject(error)
        else resolve(port)
      })
    })
  })

type CdpMessage = {
  readonly id?: number
  readonly method?: string
  readonly params?: unknown
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
          const message = JSON.stringify({ id, method, params })
          return new Promise<Result>((messageResolve, messageReject) => {
            pending.set(id, {
              resolve: value => messageResolve(value as Result),
              reject: messageReject,
            })
            socket.send(message)
          })
        },
      })
    })

    socket.addEventListener("error", () => {
      reject(new Error(`failed to connect to Chrome CDP at ${webSocketUrl}`))
    })

    socket.addEventListener("message", event => {
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

const waitForPageWebSocket = async (
  debugPort: number,
): Promise<string> => {
  const deadline = Date.now() + 10_000
  let lastError: unknown = null
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`)
      if (response.ok) {
        const targets = await response.json() as Array<{
          readonly type?: string
          readonly webSocketDebuggerUrl?: string
        }>
        const page = targets.find(
          target =>
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

const smokeEntry = `
import {
  registerTrainingRunElement,
  trainingRunTagName,
} from "@openagentsinc/three-effect/foldkit"
import { trainingRunVisualizationOptionsFromSnapshot } from "@openagentsinc/three-effect/core"
import { withChatWorldPaymentLayer } from "../src/shared/chat-world-visualization"

registerTrainingRunElement()

const baseVisualization = trainingRunVisualizationOptionsFromSnapshot({
  activeWindowCount: 1,
  assignedContributorCount: 8,
  blockerRefCount: 2,
  closeoutSatisfied: false,
  deviceObserved: 5,
  deviceRequired: 6,
  externalStatus: "observed",
  finalValidationLoss: 3.08,
  freivaldsRefCount: 3,
  gradientCloseoutRefCount: 2,
  lifecycleCounts: {
    active: 3,
    qualified: 2,
    registered: 2,
    state_synced: 1,
    sync_reentry: 1,
    warmup: 2,
  },
  maxAllowedStaleSteps: 5,
  maxValidationLoss: 3.5,
  operatorSignals: [
    { detail: "planned", id: "plan", label: "plan", state: "success" },
    { detail: "active", id: "activate", label: "activate", state: "success" },
    { detail: "claimed", id: "lease", label: "lease", state: "success" },
    { detail: "granted", id: "bootstrap", label: "bootstrap", state: "success" },
    { detail: "admitted", id: "admit", label: "admit", state: "success" },
    { detail: "pending", id: "reconcile", label: "reconcile", state: "info" },
  ],
  pendingPayoutCount: 2,
  plannedWindowCount: 2,
  promiseSignals: [
    {
      blockerCount: 2,
      evidenceRefCount: 8,
      id: "training.model_ladder.v1",
      label: "model ladder",
      state: "yellow",
    },
    {
      blockerCount: 3,
      evidenceRefCount: 4,
      id: "training.marathon_operations.v1",
      label: "marathon ops",
      state: "red",
    },
    {
      blockerCount: 1,
      evidenceRefCount: 6,
      id: "training.public_distributed_training_run.v1",
      label: "public run",
      state: "yellow",
    },
    {
      blockerCount: 0,
      evidenceRefCount: 5,
      id: "training.verification_classes.v1",
      label: "verification",
      state: "green",
    },
  ],
  receiptRefCount: 7,
  reconciledWindowCount: 0,
  rejectedWorkCount: 1,
  runDetail: "run.cs336.a1.real_gradient.smoke",
  runLabel: "pylon.first_real_model_training_run.v1",
  runState: "active",
  sealInFlight: true,
  sealedWindowCount: 1,
  settledPayoutSats: 0,
  verifiedWorkCount: 4,
})

const visualization = withChatWorldPaymentLayer(
  baseVisualization,
  [{
    id: "smoke-payment-1",
    fromRef: "pylon.smoke.alpha",
    toRef: "agent.smoke.tassadar",
    amountSats: 21000,
    realBitcoinMoved: true,
    color: 0xf5b73a,
    size: 0.7,
    sourceRefs: ["receipt.smoke.real_bitcoin_moved.1"],
    ts: "2026-06-20T00:00:00.000Z",
    text: "smoke settlement",
  }],
  {
    connected: true,
    database: "openagents-world",
    worldUrl: "https://world.openagents.com",
    regionRef: "region.smoke",
    stations: [{
      pylonRef: "pylon.smoke.alpha",
      label: "Smoke Alpha Pylon",
      x: -2.5,
      y: 0.25,
      z: -1.25,
    }],
    agents: [{
      avatarRef: "avatar.smoke.tassadar",
      actorRef: "agent.smoke.tassadar",
      avatarKind: "tassadar",
      label: "Smoke Tassadar",
      color: "#f5b73a",
      x: 2.5,
      y: 0.75,
      z: 1.25,
      yaw: 0,
      movementMode: "walk",
      chatMessages: [],
      attentionRefs: [],
    }],
    proximityChatCount: 0,
  },
)

const host = document.createElement(trainingRunTagName)
host.id = "training-scene"
host.style.display = "block"
host.style.width = "960px"
host.style.height = "540px"
host.style.minHeight = "540px"
host.visualization = visualization
document.getElementById("scene")?.append(host)
`

const smokeHtml = `
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Autopilot Training Scene Smoke</title>
    <style>
      :root { color-scheme: dark; background: #050505; }
      body {
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        background: #050505;
      }
      #scene {
        width: 960px;
        height: 540px;
        background: #050505;
      }
    </style>
  </head>
  <body>
    <div id="scene"></div>
    <script type="module" src="/dist/training-scene-smoke-entry.js"></script>
  </body>
</html>
`

type PixelSmokeResult = {
  readonly ok: boolean
  readonly brightPixels: number
  readonly canvasHeight: number
  readonly canvasStableAfterEquivalentUpdate: boolean
  readonly canvasWidth: number
  readonly distinctLumaBuckets: number
  readonly message: string
  readonly sampledPixels: number
}

type CanvasProbeResult = {
  readonly canvasHeight: number
  readonly canvasStableAfterEquivalentUpdate: boolean
  readonly canvasWidth: number
  readonly message: string
  readonly ok: boolean
}

type PngImage = {
  readonly data: Uint8Array
  readonly height: number
  readonly width: number
}

const readUint32 = (buffer: Buffer, offset: number): number =>
  buffer.readUInt32BE(offset)

const paethPredictor = (left: number, up: number, upLeft: number): number => {
  const estimate = left + up - upLeft
  const leftDistance = Math.abs(estimate - left)
  const upDistance = Math.abs(estimate - up)
  const upLeftDistance = Math.abs(estimate - upLeft)
  if (leftDistance <= upDistance && leftDistance <= upLeftDistance) return left
  if (upDistance <= upLeftDistance) return up
  return upLeft
}

const decodePng = (png: Buffer): PngImage => {
  const signature = png.subarray(0, 8).toString("hex")
  if (signature !== "89504e470d0a1a0a") {
    throw new Error("Chrome screenshot was not a PNG")
  }

  let offset = 8
  let width = 0
  let height = 0
  let bitDepth = 0
  let colorType = 0
  const idatChunks: Buffer[] = []

  while (offset < png.length) {
    const length = readUint32(png, offset)
    const type = png.subarray(offset + 4, offset + 8).toString("ascii")
    const data = png.subarray(offset + 8, offset + 8 + length)
    offset += 12 + length

    if (type === "IHDR") {
      width = readUint32(data, 0)
      height = readUint32(data, 4)
      bitDepth = data[8] ?? 0
      colorType = data[9] ?? 0
      const interlace = data[12] ?? 0
      if (bitDepth !== 8) {
        throw new Error(`unsupported PNG bit depth ${bitDepth}`)
      }
      if (colorType !== 2 && colorType !== 6) {
        throw new Error(`unsupported PNG color type ${colorType}`)
      }
      if (interlace !== 0) {
        throw new Error("interlaced PNG screenshots are unsupported")
      }
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
        case 0:
          row[x] = raw
          break
        case 1:
          row[x] = (raw + left) & 0xff
          break
        case 2:
          row[x] = (raw + up) & 0xff
          break
        case 3:
          row[x] = (raw + Math.floor((left + up) / 2)) & 0xff
          break
        case 4:
          row[x] = (raw + paethPredictor(left, up, upLeft)) & 0xff
          break
        default:
          throw new Error(`unsupported PNG filter ${filter}`)
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

const screenshotPixelSmoke = (
  screenshotBase64: string,
  probe: CanvasProbeResult,
): PixelSmokeResult => {
  const image = decodePng(Buffer.from(screenshotBase64, "base64"))
  let brightPixels = 0
  const buckets = new Set<number>()
  for (let offset = 0; offset < image.data.length; offset += 4) {
    const luma =
      (image.data[offset] ?? 0) +
      (image.data[offset + 1] ?? 0) +
      (image.data[offset + 2] ?? 0)
    if (luma > 80) brightPixels += 1
    buckets.add(Math.floor(luma / 32))
  }
  const sampledPixels = image.width * image.height
  const ok =
    probe.ok &&
    probe.canvasWidth >= 480 &&
    probe.canvasHeight >= 270 &&
    probe.canvasStableAfterEquivalentUpdate &&
    brightPixels > 350 &&
    buckets.size >= 4

  return {
    ok,
    message: ok
      ? "training scene screenshot has nonblank WebGL pixels"
      : `training scene screenshot appears blank or undersized: ${probe.message}`,
    canvasWidth: probe.canvasWidth,
    canvasHeight: probe.canvasHeight,
    canvasStableAfterEquivalentUpdate: probe.canvasStableAfterEquivalentUpdate,
    sampledPixels,
    brightPixels,
    distinctLumaBuckets: buckets.size,
  }
}

const canvasProbeExpression = `
(async () => {
  await customElements.whenDefined("oa-training-run")
  const host = document.querySelector("oa-training-run")
  await new Promise(resolve => requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      setTimeout(resolve, 350)
    })
  }))
  const canvas = host?.shadowRoot?.querySelector("canvas")
  if (!(canvas instanceof HTMLCanvasElement)) {
    return {
      ok: false,
      message: "missing oa-training-run shadow canvas",
      canvasWidth: 0,
      canvasHeight: 0,
      canvasStableAfterEquivalentUpdate: false,
    }
  }
  host.visualization = structuredClone(host.visualization)
  await new Promise(resolve => requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      setTimeout(resolve, 120)
    })
  }))
  const canvasAfterEquivalentUpdate = host.shadowRoot?.querySelector("canvas")
  const canvasStableAfterEquivalentUpdate = canvasAfterEquivalentUpdate === canvas
  return {
    ok:
      canvas.width >= 480 &&
      canvas.height >= 270 &&
      canvasStableAfterEquivalentUpdate,
    message: canvasStableAfterEquivalentUpdate
      ? "oa-training-run shadow canvas mounted and stable after equivalent update"
      : "oa-training-run shadow canvas remounted after equivalent update",
    canvasWidth: canvas.width,
    canvasHeight: canvas.height,
    canvasStableAfterEquivalentUpdate,
  }
})()
`

const main = async (): Promise<void> => {
  const tmpRoot = mkdtempSync(
    join(process.cwd(), ".training-scene-smoke-"),
  )
  const userDataDir = join(tmpRoot, "chrome-profile")
  const entryPath = join(tmpRoot, "training-scene-smoke-entry.ts")
  const distDir = join(tmpRoot, "dist")
  const htmlPath = join(tmpRoot, "index.html")
  mkdirSync(distDir, { recursive: true })
  mkdirSync(userDataDir, { recursive: true })
  writeFileSync(entryPath, smokeEntry)
  writeFileSync(htmlPath, smokeHtml)

  let server: ReturnType<typeof Bun.serve> | null = null
  let chrome: Bun.Subprocess | null = null
  let cdp: CdpClient | null = null

  try {
    const build = Bun.spawnSync({
      cmd: [
        process.execPath,
        "build",
        entryPath,
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
      const stdout = new TextDecoder().decode(build.stdout)
      const stderr = new TextDecoder().decode(build.stderr)
      if (stdout.trim().length > 0) console.error(stdout)
      if (stderr.trim().length > 0) console.error(stderr)
      throw new Error("failed to build training scene smoke bundle")
    }

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
        if (url.pathname === "/dist/training-scene-smoke-entry.js") {
          return new Response(
            readFileSync(join(distDir, "training-scene-smoke-entry.js")),
            { headers: { "content-type": "text/javascript; charset=utf-8" } },
          )
        }
        return new Response("not found", { status: 404 })
      },
    })

    const debugPort = await getFreePort()
    const smokeUrl = `http://127.0.0.1:${server.port}/`
    chrome = Bun.spawn({
      cmd: [
        resolveChromePath(),
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
        `--remote-debugging-port=${debugPort}`,
        `--user-data-dir=${userDataDir}`,
        smokeUrl,
      ],
      stdout: "pipe",
      stderr: "pipe",
    })

    const pageWebSocket = await waitForPageWebSocket(debugPort)
    cdp = await connectCdp(pageWebSocket)
    await cdp.send("Runtime.enable")
    await cdp.send("Page.enable")
    await cdp.send("Page.navigate", { url: smokeUrl })
    await wait(900)
    const evaluation = await cdp.send<{
      readonly result?: { readonly value?: CanvasProbeResult }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: canvasProbeExpression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (evaluation.exceptionDetails !== undefined) {
      throw new Error(
        `training scene smoke threw in Chrome: ${JSON.stringify(evaluation.exceptionDetails)}`,
      )
    }
    const probe = evaluation.result?.value
    if (probe === undefined) {
      throw new Error("training scene smoke returned no canvas probe")
    }
    const screenshot = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      {
        format: "png",
        fromSurface: true,
      },
    )
    const result = screenshotPixelSmoke(screenshot.data, probe)
    console.log(JSON.stringify(result, null, 2))
    if (!result.ok) {
      throw new Error(result.message)
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

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
