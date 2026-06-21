import { createHash } from "node:crypto"
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { join, normalize, relative } from "node:path"
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

const viewport = { width: 1280, height: 800 }
const tassadarBulletinWorldItemId = "verse:bulletin:tassadar-run"

const resolveChromePath = (): string => {
  const chromePath = chromeCandidates.find(candidate => existsSync(candidate))
  if (chromePath !== undefined) return chromePath

  throw new Error(
    "Chrome executable not found. Set CHROME_PATH to a Chrome, Chromium, or Edge binary before running smoke:verse-launch.",
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

const waitForPageWebSocket = async (debugPort: number): Promise<string> => {
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

type Rect = {
  readonly bottom: number
  readonly height: number
  readonly left: number
  readonly right: number
  readonly top: number
  readonly width: number
}

type DomProbe = {
  readonly ok: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly checks: {
    readonly appShellVerse: boolean
    readonly chatPaneWorld: boolean
    readonly noVerseHotbar: boolean
    readonly noVerseChatbar: boolean
    readonly noCommandPaletteAffordance: boolean
    readonly noPylonBaseStatus: boolean
    readonly noCharacterCreationCard: boolean
    readonly noPersistentChatThread: boolean
    readonly trainingCanvas: boolean
    readonly noVerseTitle: boolean
    readonly noAdvancedButton: boolean
    readonly noAdvancedChrome: boolean
    readonly noPanelOverlap: boolean
  }
  readonly canvas: {
    readonly height: number
    readonly width: number
    readonly rect: Rect | null
  }
  readonly forbiddenTextHits: ReadonlyArray<string>
  readonly overlapPairs: ReadonlyArray<string>
  readonly visibleTextSample: string
}

type PngImage = {
  readonly data: Uint8Array
  readonly height: number
  readonly width: number
}

type PixelSmoke = {
  readonly ok: boolean
  readonly brightPixels: number
  readonly distinctLumaBuckets: number
  readonly height: number
  readonly sampledPixels: number
  readonly sha256: string
  readonly width: number
}

type MovementSmoke = {
  readonly ok: boolean
  readonly changedPixels: number
  readonly changedPixelRatio: number
  readonly afterSha256: string
  readonly path: string
}

type VerseSceneDiagnostic = {
  readonly at: string
  readonly event: string
  readonly detail: Record<string, unknown>
}

type ContinuousMovementSmoke = {
  readonly ok: boolean
  readonly sampledFrames: number
  readonly nonblankFrames: number
  readonly minBrightPixels: number
  readonly activeHostRemounts: ReadonlyArray<VerseSceneDiagnostic>
  readonly observedTassadarBoardProximity: boolean
  readonly worldItemProximityEvents: ReadonlyArray<VerseSceneDiagnostic>
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type VerseLaunchReceipt = {
  readonly ok: boolean
  readonly message: string
  readonly generatedAt: string
  readonly appBundle: string
  readonly sourceRefs: ReadonlyArray<string>
  readonly packagedFiles: {
    readonly html: string
    readonly css: string
    readonly main: string
    readonly pylonNode: string
  }
  readonly packagedSource: {
    readonly includesFirstRenderBulletinBoard: boolean
    readonly includesVisibleBulletinPrimitive: boolean
  }
  readonly viewport: typeof viewport
  readonly dom: DomProbe
  readonly screenshot: PixelSmoke & {
    readonly path: string
  }
  readonly movement: MovementSmoke
  readonly continuousMovement: ContinuousMovementSmoke
  readonly typecheckProof: string
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
      if (bitDepth !== 8) throw new Error(`unsupported PNG bit depth ${bitDepth}`)
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

const pixelSmoke = (png: Buffer): PixelSmoke => {
  const image = decodePng(png)
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
    image.width >= viewport.width &&
    image.height >= viewport.height &&
    brightPixels > 2_000 &&
    buckets.size >= 6

  return {
    ok,
    width: image.width,
    height: image.height,
    sampledPixels,
    brightPixels,
    distinctLumaBuckets: buckets.size,
    sha256: createHash("sha256").update(png).digest("hex"),
  }
}

const movementSmoke = (
  before: Buffer,
  after: Buffer,
  afterPath: string,
): MovementSmoke => {
  const beforeImage = decodePng(before)
  const afterImage = decodePng(after)
  if (
    beforeImage.width !== afterImage.width ||
    beforeImage.height !== afterImage.height ||
    beforeImage.data.length !== afterImage.data.length
  ) {
    throw new Error("Verse movement smoke screenshots have mismatched sizes")
  }

  let changedPixels = 0
  for (let offset = 0; offset < beforeImage.data.length; offset += 4) {
    const dr = Math.abs(
      (beforeImage.data[offset] ?? 0) - (afterImage.data[offset] ?? 0),
    )
    const dg = Math.abs(
      (beforeImage.data[offset + 1] ?? 0) -
        (afterImage.data[offset + 1] ?? 0),
    )
    const db = Math.abs(
      (beforeImage.data[offset + 2] ?? 0) -
        (afterImage.data[offset + 2] ?? 0),
    )
    if (dr + dg + db > 18) changedPixels += 1
  }
  const sampledPixels = beforeImage.width * beforeImage.height
  const changedPixelRatio = sampledPixels === 0 ? 0 : changedPixels / sampledPixels
  const afterSha256 = createHash("sha256").update(after).digest("hex")
  return {
    ok: changedPixels >= 250 && changedPixelRatio >= 0.0002,
    changedPixels,
    changedPixelRatio,
    afterSha256,
    path: relativePath(afterPath),
  }
}

const keyDownW = async (cdp: CdpClient): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "w",
    code: "KeyW",
    text: "w",
    unmodifiedText: "w",
    windowsVirtualKeyCode: 87,
    nativeVirtualKeyCode: 87,
  })
}

const keyDownD = async (cdp: CdpClient): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: "d",
    code: "KeyD",
    text: "d",
    unmodifiedText: "d",
    windowsVirtualKeyCode: 68,
    nativeVirtualKeyCode: 68,
  })
}

const keyUpW = async (cdp: CdpClient): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "w",
    code: "KeyW",
    windowsVirtualKeyCode: 87,
    nativeVirtualKeyCode: 87,
  })
  await wait(160)
}

const keyUpD = async (cdp: CdpClient): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: "d",
    code: "KeyD",
    windowsVirtualKeyCode: 68,
    nativeVirtualKeyCode: 68,
  })
}

const readVerseSceneDiagnostics = async (
  cdp: CdpClient,
): Promise<ReadonlyArray<VerseSceneDiagnostic>> => {
  const response = await cdp.send<{
    readonly result?: { readonly value?: ReadonlyArray<VerseSceneDiagnostic> }
  }>("Runtime.evaluate", {
    expression:
      "globalThis.__OA_DUMP_VERSE_SCENE_LOGS?.() ?? globalThis.__OA_VERSE_SCENE_LOGS ?? []",
    returnByValue: true,
  })
  return response.result?.value ?? []
}

const contentTypeFor = (path: string): string => {
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8"
  if (path.endsWith(".css")) return "text/css; charset=utf-8"
  if (path.endsWith(".html")) return "text/html; charset=utf-8"
  if (path.endsWith(".glb")) return "model/gltf-binary"
  if (path.endsWith(".blob")) return "application/octet-stream"
  if (path.endsWith(".jpg") || path.endsWith(".jpeg")) return "image/jpeg"
  if (path.endsWith(".png")) return "image/png"
  return "application/octet-stream"
}

const safeAssetPath = (viewDir: string, requestPath: string): string | null => {
  const normalized = normalize(requestPath.replace(/^\/packaged\//, ""))
  if (normalized.startsWith("..") || normalized.includes("/../")) return null
  return join(viewDir, normalized)
}

const patchedPackagedHtml = (html: string): string =>
  html
    .replace(
      "views://autopilot-desktop/styles.css",
      "/packaged/styles.css",
    )
    .replace(
      '<script type="module" src="views://autopilot-desktop/main.js"></script>',
      [
        "<script>",
        "window.__electrobun = window.__electrobun || {};",
        "window.__electrobunBunBridge = window.__electrobunBunBridge || { postMessage() {} };",
        "</script>",
        '<script type="module" src="/packaged/main.js"></script>',
      ].join(""),
    )

const relativePath = (path: string): string =>
  relative(process.cwd(), path) || "."

const domProbeExpression = `
(async () => {
  const wait = (ms) => new Promise(resolve => setTimeout(resolve, ms))
  const waitFor = async (predicate, timeout = 10000) => {
    const deadline = Date.now() + timeout
    while (Date.now() < deadline) {
      if (predicate()) return true
      await wait(100)
    }
    return false
  }
  await waitFor(() => document.querySelector(".app-shell-verse") !== null)
  await customElements.whenDefined("oa-training-run").catch(() => null)
  await waitFor(() => {
    const host = document.querySelector(".three-effect-chat-scene")
    return host?.shadowRoot?.querySelector("canvas") instanceof HTMLCanvasElement
  })
  await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
  await wait(900)

  const rect = (selector) => {
    const element = document.querySelector(selector)
    if (!(element instanceof HTMLElement)) return null
    const r = element.getBoundingClientRect()
    return {
      top: r.top,
      right: r.right,
      bottom: r.bottom,
      left: r.left,
      width: r.width,
      height: r.height,
    }
  }
  const overlapArea = (a, b) => {
    if (a === null || b === null) return 0
    const x = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left))
    const y = Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top))
    return x * y
  }
  const overlapPairs = [
    [".chat-content-overlay", ".character-creation-overlay", "chat/character-creation"],
    [".chat-content-overlay", ".pylon-base-status", "chat/pylon-base"],
    [".chat-content-overlay", ".hotbar", "chat/hotbar"],
    [".pylon-base-status", ".hotbar", "pylon-base/hotbar"],
  ].flatMap(([a, b, label]) => overlapArea(rect(a), rect(b)) > 1 ? [label] : [])

  const text = document.body.innerText || ""
  const forbiddenText = [
    "Claude Code",
    "Codex",
    "Spawn a session",
    "Sessions",
    "Swarm",
    "Deploy",
    "The Verse",
    "Advanced",
  ]
  const forbiddenTextHits = forbiddenText.filter(value => text.includes(value))
  const host = document.querySelector(".three-effect-chat-scene")
  const canvas = host?.shadowRoot?.querySelector("canvas")
  const canvasRect = canvas instanceof HTMLCanvasElement
    ? (() => {
      const r = canvas.getBoundingClientRect()
      return {
        top: r.top,
        right: r.right,
        bottom: r.bottom,
        left: r.left,
        width: r.width,
        height: r.height,
      }
    })()
    : null
  const checks = {
    appShellVerse: document.querySelector(".app-shell-verse") !== null,
    chatPaneWorld: document.querySelector(".chat-pane-world") !== null,
    noVerseHotbar: document.querySelector(".app-shell-verse .hotbar") === null,
    noVerseChatbar:
      document.querySelector(".app-shell-verse .chat-composer-verse") === null &&
      document.querySelector(".app-shell-verse .chat-input") === null,
    noCommandPaletteAffordance:
      document.querySelector(".app-shell-verse .command-palette") === null &&
      text.includes("Command palette") === false &&
      text.includes("⌘K") === false,
    noPylonBaseStatus: document.querySelector(".pylon-base-status") === null,
    noCharacterCreationCard:
      document.querySelector(".character-creation-overlay") === null,
    noPersistentChatThread:
      document.querySelector(".chat-thread-shell") === null &&
      document.querySelector(".chat-message-list") === null,
    trainingCanvas:
      canvas instanceof HTMLCanvasElement &&
      canvas.width >= 480 &&
      canvas.height >= 270 &&
      canvasRect !== null &&
      canvasRect.width >= 640 &&
      canvasRect.height >= 400,
    noVerseTitle: text.includes("The Verse") === false,
    noAdvancedButton:
      document.querySelector(".verse-advanced") === null &&
      text.includes("Advanced") === false,
    noAdvancedChrome:
      document.querySelector(".sidebar") === null &&
      document.querySelector(".shell-target-tabs") === null &&
      document.querySelector(".status-hud-overlay") === null &&
      forbiddenTextHits.length === 0,
    noPanelOverlap: overlapPairs.length === 0,
  }
  const blockerRefs = Object.entries(checks)
    .flatMap(([key, passed]) => passed ? [] : ["verse.launch." + key])
  const ok = blockerRefs.length === 0

  return {
    ok,
    blockerRefs,
    checks,
    canvas: {
      width: canvas instanceof HTMLCanvasElement ? canvas.width : 0,
      height: canvas instanceof HTMLCanvasElement ? canvas.height : 0,
      rect: canvasRect,
    },
    forbiddenTextHits,
    overlapPairs,
    visibleTextSample: text.replace(/\\s+/g, " ").trim().slice(0, 320),
  }
})()
`

const main = async (): Promise<void> => {
  const appBundle = process.env.AUTOPILOT_DESKTOP_APP_BUNDLE ??
    join(process.cwd(), "build/dev-macos-arm64/Autopilot-dev.app")
  const resourcesDir = join(appBundle, "Contents/Resources/app")
  const viewDir = join(resourcesDir, "views/autopilot-desktop")
  const htmlPath = join(viewDir, "index.html")
  const cssPath = join(viewDir, "styles.css")
  const mainPath = join(viewDir, "main.js")
  const pylonNodePath = join(resourcesDir, "pylon-node/index.js")
  const proofDir = process.env.AUTOPILOT_DESKTOP_VERSE_SMOKE_PROOF_DIR ??
    join(process.cwd(), "build/verse-launch-smoke")
  const screenshotPath = join(proofDir, "verse-launch-smoke.png")
  const movementScreenshotPath = join(proofDir, "verse-launch-smoke-after-w.png")
  const receiptPath = join(proofDir, "verse-launch-smoke.json")

  for (const [label, path] of [
    ["packaged view HTML", htmlPath],
    ["packaged view CSS", cssPath],
    ["packaged view JS", mainPath],
    ["packaged Pylon node", pylonNodePath],
  ] as const) {
    if (!existsSync(path)) {
      throw new Error(`${label} missing at ${path}; run bun run build first`)
    }
  }

  const tmpRoot = join(process.cwd(), ".verse-launch-smoke-profile")
  rmSync(tmpRoot, { force: true, recursive: true })
  mkdirSync(tmpRoot, { recursive: true })
  mkdirSync(proofDir, { recursive: true })

  const html = patchedPackagedHtml(readFileSync(htmlPath, "utf8"))
  const packagedMain = readFileSync(mainPath, "utf8")
  const packagedSource = {
    includesFirstRenderBulletinBoard:
      packagedMain.includes("Tassadar Board") &&
      packagedMain.includes("Loading Tassadar run"),
    includesVisibleBulletinPrimitive:
      packagedMain.includes("width = 3.1") &&
      packagedMain.includes("postHeight = 2.18"),
  }
  if (!packagedSource.includesFirstRenderBulletinBoard) {
    throw new Error(
      "Packaged Verse bundle is missing the first-render Tassadar bulletin board copy; run bun run build after changing the board.",
    )
  }
  if (!packagedSource.includesVisibleBulletinPrimitive) {
    throw new Error(
      "Packaged Verse bundle is missing the visible three-effect bulletin board primitive; repin/build @openagentsinc/three-effect.",
    )
  }
  let server: ReturnType<typeof Bun.serve> | null = null
  let chrome: Bun.Subprocess | null = null
  let cdp: CdpClient | null = null

  try {
    server = Bun.serve({
      hostname: "127.0.0.1",
      port: 0,
      fetch(request) {
        const url = new URL(request.url)
        if (url.pathname === "/") {
          return new Response(html, {
            headers: { "content-type": "text/html; charset=utf-8" },
          })
        }
        if (url.pathname === "/packaged/main.js") {
          return new Response(readFileSync(mainPath), {
            headers: { "content-type": "text/javascript; charset=utf-8" },
          })
        }
        if (url.pathname === "/packaged/styles.css") {
          return new Response(readFileSync(cssPath), {
            headers: { "content-type": "text/css; charset=utf-8" },
          })
        }
        if (url.pathname.startsWith("/packaged/assets/")) {
          const assetPath = safeAssetPath(viewDir, url.pathname)
          if (assetPath !== null && existsSync(assetPath)) {
            return new Response(readFileSync(assetPath), {
              headers: { "content-type": contentTypeFor(assetPath) },
            })
          }
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
        `--user-data-dir=${tmpRoot}`,
        smokeUrl,
      ],
      stdout: "pipe",
      stderr: "pipe",
    })

    const pageWebSocket = await waitForPageWebSocket(debugPort)
    cdp = await connectCdp(pageWebSocket)
    await cdp.send("Runtime.enable")
    await cdp.send("Page.enable")
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: 1,
      mobile: false,
    })
    await cdp.send("Page.navigate", { url: smokeUrl })

    const evaluation = await cdp.send<{
      readonly result?: { readonly value?: DomProbe }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: domProbeExpression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (evaluation.exceptionDetails !== undefined) {
      throw new Error(
        `Verse launch smoke threw in Chrome: ${JSON.stringify(evaluation.exceptionDetails)}`,
      )
    }
    const dom = evaluation.result?.value
    if (dom === undefined) {
      throw new Error("Verse launch smoke returned no DOM probe")
    }

    const screenshot = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const screenshotBytes = Buffer.from(screenshot.data, "base64")
    const pixels = pixelSmoke(screenshotBytes)
    writeFileSync(screenshotPath, screenshotBytes)

    const diagnosticsBeforeMovement = await readVerseSceneDiagnostics(cdp)
    await keyDownW(cdp)
    await keyDownD(cdp)
    await wait(120)
    const movementFrameBytes: Buffer[] = []
    for (let i = 0; i < 5; i += 1) {
      const frame = await cdp.send<{ readonly data: string }>(
        "Page.captureScreenshot",
        { format: "png", fromSurface: true },
      )
      movementFrameBytes.push(Buffer.from(frame.data, "base64"))
      await wait(140)
    }
    await keyUpD(cdp)
    await keyUpW(cdp)
    const movementScreenshotBytes =
      movementFrameBytes[movementFrameBytes.length - 1]
    if (movementScreenshotBytes === undefined) {
      throw new Error("Verse movement smoke captured no movement frames")
    }
    writeFileSync(movementScreenshotPath, movementScreenshotBytes)
    const movement = movementSmoke(
      screenshotBytes,
      movementScreenshotBytes,
      movementScreenshotPath,
    )
    const diagnosticsAfterMovement = await readVerseSceneDiagnostics(cdp)
    const movementDiagnostics = diagnosticsAfterMovement.slice(
      diagnosticsBeforeMovement.length,
    )
    const movementFramePixels = movementFrameBytes.map(pixelSmoke)
    const activeHostRemounts = movementDiagnostics.filter(entry =>
      entry.event === "verse-host.remount.mounted" ||
      entry.event === "verse-host.remount.swapped",
    )
    const worldItemProximityEvents = movementDiagnostics.filter(entry =>
      entry.event === "verse-host.world-item-proximity",
    )
    const observedTassadarBoardProximity = worldItemProximityEvents.some(entry =>
      entry.detail["itemId"] === tassadarBulletinWorldItemId,
    )
    const continuousMovement: ContinuousMovementSmoke = {
      ok:
        movementFramePixels.every(frame => frame.ok) &&
        activeHostRemounts.length === 0 &&
        observedTassadarBoardProximity,
      sampledFrames: movementFramePixels.length,
      nonblankFrames: movementFramePixels.filter(frame => frame.ok).length,
      minBrightPixels: Math.min(
        ...movementFramePixels.map(frame => frame.brightPixels),
      ),
      activeHostRemounts,
      observedTassadarBoardProximity,
      worldItemProximityEvents,
      diagnosticsSample: movementDiagnostics.slice(-20),
    }

    const ok = dom.ok && pixels.ok && movement.ok && continuousMovement.ok
    const receipt: VerseLaunchReceipt = {
      ok,
      message: ok
        ? "packaged Desktop Verse launch is nonblank and first-paint checks passed"
        : "packaged Desktop Verse launch failed smoke checks",
      generatedAt: new Date().toISOString(),
      appBundle: relativePath(appBundle),
      sourceRefs: [
        "github:OpenAgentsInc/openagents#5827",
        "github:OpenAgentsInc/openagents#5910",
        "script:apps/autopilot-desktop/scripts/verse-launch-smoke.ts",
      ],
      packagedFiles: {
        html: relativePath(htmlPath),
        css: relativePath(cssPath),
        main: relativePath(mainPath),
        pylonNode: relativePath(pylonNodePath),
      },
      packagedSource,
      viewport,
      dom,
      screenshot: {
        ...pixels,
        path: relativePath(screenshotPath),
      },
      movement,
      continuousMovement,
      typecheckProof:
        "Desktop typecheck is enforced by apps/autopilot-desktop `bun run typecheck` and runs first in `bun run verify:deploy`.",
    }
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`)
    console.log(JSON.stringify(receipt, null, 2))

    if (!receipt.ok) {
      throw new Error(
        [
          receipt.message,
          ...dom.blockerRefs,
          pixels.ok ? "" : "verse.launch.screenshotPixels",
          movement.ok ? "" : "verse.launch.wasdMovementPixels",
          continuousMovement.ok
            ? ""
            : "verse.launch.continuousMovementNoBlackFramesRemountsOrBoardProximityMiss",
        ].filter(Boolean).join("; "),
      )
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
