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

import { projectVerseWebglDiagnostics } from "../src/shared/verse-progress-diagnostics-model"

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
    readonly verseHotbar: boolean
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
    readonly sceneCursorNotText: boolean
    readonly scenePointerTarget: boolean
    readonly verseInitialFocus: boolean
    readonly hotbarNumberKeysConsumed: boolean
  }
  readonly canvas: {
    readonly height: number
    readonly width: number
    readonly rect: Rect | null
  }
  readonly pointerHitTest: {
    readonly canvasCursor: string
    readonly topElements: ReadonlyArray<string>
  }
  readonly focus: {
    readonly activeElement: string | null
    readonly shadowActiveElement: string | null
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

type MouseLookDragSmoke = {
  readonly ok: boolean
  readonly observedDragControl: boolean
  readonly observedWheelControl: boolean
  readonly cursorAfterDrag: string
  readonly scenePointerTarget: boolean
  readonly selectedText: string
  readonly topElements: ReadonlyArray<string>
  readonly cameraControlEvents: ReadonlyArray<VerseSceneDiagnostic>
  readonly frame: PixelSmoke & {
    readonly path: string
  }
  readonly movement: MovementSmoke
  readonly activeHostRemounts: ReadonlyArray<VerseSceneDiagnostic>
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type RetainedLiveUpdateSmoke = {
  readonly ok: boolean
  readonly boardHydrated: boolean
  readonly retainedUpdateCount: number
  readonly remounts: ReadonlyArray<VerseSceneDiagnostic>
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type ScenePoint = {
  readonly x: number
  readonly y: number
}

type VerseCodingOverlayProbe = {
  readonly ok: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly checks: {
    readonly smokeHook: boolean
    readonly enteredCodeMode: boolean
    readonly dockVisible: boolean
    readonly accountInventoryVisible: boolean
    readonly selectedAccountVisible: boolean
    readonly composerPaneOpen: boolean
    readonly sessionsPaneOpen: boolean
    readonly decisionsPaneOpen: boolean
    readonly diffPaneOpen: boolean
    readonly terminalPaneOpen: boolean
    readonly diagnosticsPaneOpen: boolean
    readonly streamRowsVisible: boolean
    readonly approvalVisible: boolean
    readonly diffVisible: boolean
    readonly terminalVisible: boolean
    readonly diagnosticsExportVisible: boolean
    readonly diagnosticsExportPublicSafe: boolean
    readonly scenePointAvailable: boolean
    readonly focusedTextarea: boolean
  }
  readonly paneKinds: ReadonlyArray<string>
  readonly openResults: Readonly<Record<string, boolean>>
  readonly scenePoint: ScenePoint | null
  readonly visibleTextSample: string
  readonly diagnosticsExportSample: string
}

type FocusedTypingProbe = {
  readonly ok: boolean
  readonly activeTag: string
  readonly valueTail: string
}

type VerseCodingOverlaySmoke = {
  readonly ok: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly probe: VerseCodingOverlayProbe
  readonly focusedTyping: FocusedTypingProbe
  readonly screenshot: PixelSmoke & {
    readonly path: string
  }
  readonly sceneInput: {
    readonly ok: boolean
    readonly observedDragControl: boolean
    readonly observedWheelControl: boolean
    readonly activeHostRemounts: ReadonlyArray<VerseSceneDiagnostic>
    readonly blackFrameEvents: ReadonlyArray<VerseSceneDiagnostic>
    readonly cameraControlEvents: ReadonlyArray<VerseSceneDiagnostic>
    readonly frame: PixelSmoke & {
      readonly path: string
    }
    readonly movement: MovementSmoke
    readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
  }
}

type KeybindingUiProbe = {
  readonly ok: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly activeLabel: string
  readonly storedCodes: ReadonlyArray<string>
  readonly visibleTextSample: string
}

type LocalPoseSample = {
  readonly animation: string | null
  readonly capturedAtMs: number | null
  readonly x: number
  readonly y: number
  readonly z: number
}

type KeybindingMovementProbe = {
  readonly ok: boolean
  readonly expected: "movement" | "stationary"
  readonly keyCode: string
  readonly poseBefore: LocalPoseSample | null
  readonly poseAfter: LocalPoseSample | null
  readonly poseDelta: number | null
  readonly frame: PixelSmoke & {
    readonly path: string
  }
  readonly visualDiff: MovementSmoke
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type KeybindingFocusedTypingProbe = {
  readonly ok: boolean
  readonly target: "composer" | "terminal"
  readonly activeTag: string
  readonly valueTail: string
  readonly poseBefore: LocalPoseSample | null
  readonly poseAfter: LocalPoseSample | null
  readonly poseDelta: number | null
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type CustomKeybindingSmoke = {
  readonly ok: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly captureProbe: KeybindingUiProbe
  readonly persistedAfterCapture: KeybindingUiProbe
  readonly wSuppressed: KeybindingMovementProbe
  readonly iMovement: KeybindingMovementProbe
  readonly mouseLookAfterRebind: MouseLookDragSmoke
  readonly codeOverlayProbe: VerseCodingOverlayProbe
  readonly composerFocusedTyping: KeybindingFocusedTypingProbe
  readonly terminalFocusedTyping: KeybindingFocusedTypingProbe
  readonly persistenceAfterReload: KeybindingUiProbe
  readonly iMovementAfterReload: KeybindingMovementProbe
  readonly resetProbe: KeybindingUiProbe
  readonly wMovementAfterReset: KeybindingMovementProbe
  readonly activeHostRemounts: ReadonlyArray<VerseSceneDiagnostic>
  readonly blackFrameEvents: ReadonlyArray<VerseSceneDiagnostic>
  readonly diagnosticsSample: ReadonlyArray<VerseSceneDiagnostic>
}

type VerseLaunchReceipt = {
  readonly ok: boolean
  readonly message: string
  readonly generatedAt: string
  readonly target: "packaged" | "dev"
  readonly appBundle: string | null
  readonly appUrl: string
  readonly sourceRefs: ReadonlyArray<string>
  readonly packagedFiles: {
    readonly html: string
    readonly css: string
    readonly main: string
    readonly pylonNode: string
  } | null
  readonly packagedSource: {
    readonly includesFirstRenderBulletinBoard: boolean
    readonly includesVisibleBulletinPrimitive: boolean
  } | null
  readonly viewport: typeof viewport
  readonly dom: DomProbe
  readonly screenshot: PixelSmoke & {
    readonly path: string
  }
  readonly movement: MovementSmoke
  readonly continuousMovement: ContinuousMovementSmoke
  readonly mouseLookDrag: MouseLookDragSmoke
  readonly retainedLiveUpdate: RetainedLiveUpdateSmoke
  readonly customKeybindings: CustomKeybindingSmoke
  readonly codingOverlay: VerseCodingOverlaySmoke
  readonly webglDiagnostics: ReturnType<typeof projectVerseWebglDiagnostics>
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

type KeyboardSpec = {
  readonly code: string
  readonly key: string
  readonly nativeVirtualKeyCode: number
  readonly text: string
  readonly windowsVirtualKeyCode: number
}

const keySpecW: KeyboardSpec = {
  code: "KeyW",
  key: "w",
  nativeVirtualKeyCode: 87,
  text: "w",
  windowsVirtualKeyCode: 87,
}

const keySpecD: KeyboardSpec = {
  code: "KeyD",
  key: "d",
  nativeVirtualKeyCode: 68,
  text: "d",
  windowsVirtualKeyCode: 68,
}

const keySpecI: KeyboardSpec = {
  code: "KeyI",
  key: "i",
  nativeVirtualKeyCode: 73,
  text: "i",
  windowsVirtualKeyCode: 73,
}

const keyDown = async (
  cdp: CdpClient,
  spec: KeyboardSpec,
): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: spec.key,
    code: spec.code,
    text: spec.text,
    unmodifiedText: spec.text,
    windowsVirtualKeyCode: spec.windowsVirtualKeyCode,
    nativeVirtualKeyCode: spec.nativeVirtualKeyCode,
  })
}

const keyUp = async (
  cdp: CdpClient,
  spec: KeyboardSpec,
): Promise<void> => {
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: spec.key,
    code: spec.code,
    windowsVirtualKeyCode: spec.windowsVirtualKeyCode,
    nativeVirtualKeyCode: spec.nativeVirtualKeyCode,
  })
}

const holdKey = async (
  cdp: CdpClient,
  spec: KeyboardSpec,
  ms: number,
): Promise<void> => {
  await keyDown(cdp, spec)
  await wait(ms)
  await keyUp(cdp, spec)
  await wait(220)
}

const tapKey = async (
  cdp: CdpClient,
  spec: KeyboardSpec,
): Promise<void> => {
  await keyDown(cdp, spec)
  await wait(40)
  await keyUp(cdp, spec)
  await wait(180)
}

const latestLocalPose = (
  diagnostics: ReadonlyArray<VerseSceneDiagnostic>,
): LocalPoseSample | null => {
  for (let i = diagnostics.length - 1; i >= 0; i -= 1) {
    const entry = diagnostics[i]
    if (entry?.event !== "local-pose.cached") continue
    const x = entry.detail["x"]
    const y = entry.detail["y"]
    const z = entry.detail["z"]
    if (
      typeof x === "number" &&
      typeof y === "number" &&
      typeof z === "number" &&
      Number.isFinite(x) &&
      Number.isFinite(y) &&
      Number.isFinite(z)
    ) {
      const capturedAtMs = entry.detail["capturedAtMs"]
      const animation = entry.detail["animation"]
      return {
        x,
        y,
        z,
        capturedAtMs:
          typeof capturedAtMs === "number" && Number.isFinite(capturedAtMs)
            ? capturedAtMs
            : null,
        animation: typeof animation === "string" ? animation : null,
      }
    }
  }
  return null
}

const poseDelta = (
  before: LocalPoseSample | null,
  after: LocalPoseSample | null,
): number | null => {
  if (before === null || after === null) return null
  return Math.hypot(after.x - before.x, after.z - before.z)
}

const remountDiagnostics = (
  diagnostics: ReadonlyArray<VerseSceneDiagnostic>,
): ReadonlyArray<VerseSceneDiagnostic> =>
  diagnostics.filter(entry =>
    entry.event === "verse-host.remount.mounted" ||
    entry.event === "verse-host.remount.swapped" ||
    entry.event === "verse-host.remount.scheduled",
  )

const activeRemountDiagnostics = (
  diagnostics: ReadonlyArray<VerseSceneDiagnostic>,
): ReadonlyArray<VerseSceneDiagnostic> =>
  remountDiagnostics(diagnostics).filter(entry => {
    if (entry.event !== "verse-host.remount.mounted") return true
    return !(
      entry.detail["hadPrevious"] === false &&
      entry.detail["reason"] === "direct"
    )
  })

const blackFrameDiagnostics = (
  diagnostics: ReadonlyArray<VerseSceneDiagnostic>,
): ReadonlyArray<VerseSceneDiagnostic> =>
  diagnostics.filter(entry => entry.event.includes("black-frame"))

const keyDownW = async (cdp: CdpClient): Promise<void> => {
  await keyDown(cdp, keySpecW)
}

const keyDownD = async (cdp: CdpClient): Promise<void> => {
  await keyDown(cdp, keySpecD)
}

const keyUpW = async (cdp: CdpClient): Promise<void> => {
  await keyUp(cdp, keySpecW)
  await wait(160)
}

const keyUpD = async (cdp: CdpClient): Promise<void> => {
  await keyUp(cdp, keySpecD)
}

type MouseDragPoint = {
  readonly endX: number
  readonly endY: number
  readonly startX: number
  readonly startY: number
}

const dispatchMouse = async (
  cdp: CdpClient,
  {
    button = "none",
    buttons = 0,
    clickCount = 0,
    type,
    x,
    y,
  }: {
    readonly button?: "none" | "left"
    readonly buttons?: number
    readonly clickCount?: number
    readonly type: "mouseMoved" | "mousePressed" | "mouseReleased"
    readonly x: number
    readonly y: number
  },
): Promise<void> => {
  await cdp.send("Input.dispatchMouseEvent", {
    button,
    buttons,
    clickCount,
    pointerType: "mouse",
    type,
    x,
    y,
  })
}

const dragSceneMouse = async (
  cdp: CdpClient,
  rect: Rect,
): Promise<MouseDragPoint> => {
  const startX = Math.floor(rect.left + rect.width * 0.58)
  const startY = Math.floor(rect.top + rect.height * 0.46)
  const midX = startX + 92
  const midY = startY + 24
  const endX = startX + 184
  const endY = startY + 52

  await dispatchMouse(cdp, { type: "mouseMoved", x: startX, y: startY })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    clickCount: 1,
    type: "mousePressed",
    x: startX,
    y: startY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    type: "mouseMoved",
    x: midX,
    y: midY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    type: "mouseMoved",
    x: endX,
    y: endY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    clickCount: 1,
    type: "mouseReleased",
    x: endX,
    y: endY,
  })
  await wait(180)

  return { endX, endY, startX, startY }
}

const dragSceneMouseFromPoint = async (
  cdp: CdpClient,
  point: ScenePoint,
): Promise<MouseDragPoint> => {
  const startX = Math.floor(point.x)
  const startY = Math.floor(point.y)
  const midX = startX + 72
  const midY = startY + 20
  const endX = startX + 132
  const endY = startY + 40

  await dispatchMouse(cdp, { type: "mouseMoved", x: startX, y: startY })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    clickCount: 1,
    type: "mousePressed",
    x: startX,
    y: startY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    type: "mouseMoved",
    x: midX,
    y: midY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    buttons: 1,
    type: "mouseMoved",
    x: endX,
    y: endY,
  })
  await wait(40)
  await dispatchMouse(cdp, {
    button: "left",
    clickCount: 1,
    type: "mouseReleased",
    x: endX,
    y: endY,
  })
  await wait(180)

  return { endX, endY, startX, startY }
}

const wheelSceneMouse = async (
  cdp: CdpClient,
  point: Pick<MouseDragPoint, "endX" | "endY">,
): Promise<void> => {
  await cdp.send("Input.dispatchMouseEvent", {
    button: "none",
    deltaX: 0,
    deltaY: -420,
    pointerType: "mouse",
    type: "mouseWheel",
    x: point.endX,
    y: point.endY,
  })
  await wait(180)
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

const readVerseFrameTimes = async (
  cdp: CdpClient,
): Promise<ReadonlyArray<number>> =>
  evaluateValue<ReadonlyArray<number>>(
    cdp,
    "verse frame timing probe",
    `new Promise(resolve => {
      const frames = []
      let previous = performance.now()
      let remaining = 30
      const step = (now) => {
        frames.push(Math.max(0, now - previous))
        previous = now
        remaining -= 1
        if (remaining <= 0) resolve(frames)
        else requestAnimationFrame(step)
      }
      requestAnimationFrame(step)
    })`,
  )

const evaluateValue = async <T>(
  cdp: CdpClient,
  label: string,
  expression: string,
): Promise<T> => {
  const response = await cdp.send<{
    readonly result?: { readonly value?: T }
    readonly exceptionDetails?: unknown
  }>("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  })
  if (response.exceptionDetails !== undefined) {
    throw new Error(
      `${label} threw in Chrome: ${JSON.stringify(response.exceptionDetails)}`,
    )
  }
  const value = response.result?.value
  if (value === undefined) throw new Error(`${label} returned no probe`)
  return value
}

const runStage = async <T>(
  label: string,
  task: () => Promise<T>,
  timeoutMs = 45_000,
): Promise<T> => {
  console.error(`[verse-smoke] ${label}...`)
  let timeout: ReturnType<typeof setTimeout> | null = null
  try {
    const result = await Promise.race([
      task(),
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(
          () => reject(new Error(`${label} timed out after ${timeoutMs}ms`)),
          timeoutMs,
        )
      }),
    ])
    console.error(`[verse-smoke] ${label}: ok`)
    return result
  } finally {
    if (timeout !== null) clearTimeout(timeout)
  }
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

const withSmokeHookParam = (urlText: string): string => {
  const url = new URL(urlText)
  url.searchParams.set("__oa_desktop_smoke", "1")
  return url.toString()
}

const controlledCodingSessionRef = "session.pylon.codex.verse_smoke"
const controlledCodingAccountHash =
  "account.pylon.codex.smoke.abcdef0123456789abcdef0123456789"

const controlledCodingNodeState = {
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [
    {
      sessionRef: controlledCodingSessionRef,
      adapter: "codex",
      state: "running",
      objectiveRef: "objective.verse_coding_overlay_smoke",
      workspaceRef: "workspace.github.OpenAgentsInc.openagents.main",
      accountRefHash: controlledCodingAccountHash,
      latestActivity: "check: bun test tests/verse-code-dock.test.ts exit 0",
      updatedAt: "2026-06-21T23:50:00.000Z",
    },
  ],
  events: {
    [controlledCodingSessionRef]: [
      {
        eventIndex: 0,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T23:49:55.000Z",
        detail: "thinking: plan the controlled Verse coding overlay smoke",
      },
      {
        eventIndex: 1,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T23:49:57.000Z",
        detail: "tool: inspect apps/autopilot-desktop/src/ui/view.ts",
      },
      {
        eventIndex: 2,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T23:49:59.000Z",
        detail: "edited apps/autopilot-desktop/scripts/verse-launch-smoke.ts (+120 -0)",
      },
      {
        eventIndex: 3,
        phase: "progress",
        state: "running",
        observedAt: "2026-06-21T23:50:01.000Z",
        detail: "check: bun test tests/verse-code-dock.test.ts exit 0",
      },
      {
        eventIndex: 4,
        phase: "decision_requested",
        state: "running",
        observedAt: "2026-06-21T23:50:03.000Z",
        detail: "permission requested: run controlled verification command",
      },
    ],
  },
  approvals: [
    {
      approvalRef: "approval.codex.verse_smoke.exec",
      kind: "exec",
      prompt: "Run controlled verification command?",
      createdAt: "2026-06-21T23:50:03.000Z",
    },
  ],
  accounts: [
    {
      provider: "codex",
      homeState: "present",
      ready: true,
      accountRef: "smoke",
      accountRefHash: controlledCodingAccountHash,
      selector: "registry_ref",
      blockerRefs: [],
      priority: 1,
    },
  ],
  artifacts: {
    [controlledCodingSessionRef]: {
      kind: "proof",
      outcome: "completed",
      editedFileCount: 1,
      commandCount: 1,
      totalTokens: 120,
      detail: {
        schema: "schema.pylon.proof.v1",
        objectiveDigestRef: "digest.objective.verse_smoke",
        verifyRef: "verify.verse_smoke",
        responseDigestRef: "digest.response.verse_smoke",
        externalSessionRef: "session.external.verse_smoke",
        executionPathRef: "control_session.composer",
        executionMode: "local_bounded",
        sandboxMode: "workspace-write",
        permissionMode: "on-request",
        devCheckState: "passed",
        redactionState: "clean",
        errorClass: null,
        errorDigestRef: null,
      },
    },
  },
} as const

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
  const verseFocusRoot = document.querySelector("[data-verse-focus-root='true']")
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
  const pointerX = canvasRect === null ? Math.floor(window.innerWidth / 2) : Math.floor(canvasRect.left + canvasRect.width / 2)
  const pointerY = canvasRect === null ? Math.floor(window.innerHeight / 2) : Math.floor(canvasRect.top + canvasRect.height / 2)
  const topElements = document.elementsFromPoint(pointerX, pointerY)
    .slice(0, 8)
    .map(element => {
      const className = element instanceof HTMLElement ? element.className : ""
      const normalizedClass = typeof className === "string" ? className : ""
      return [
        element.tagName.toLowerCase(),
        normalizedClass.split(/\\s+/).filter(Boolean).slice(0, 3).join("."),
      ].filter(Boolean).join(".")
    })
  const pointerTop = document.elementFromPoint(pointerX, pointerY)
  const canvasCursor = canvas instanceof HTMLCanvasElement
    ? getComputedStyle(canvas).cursor
    : "missing"
  const scenePointerTarget =
    pointerTop === host ||
    pointerTop === canvas ||
    (pointerTop instanceof HTMLElement &&
      pointerTop.closest(".three-effect-chat-scene") === host)
  const activeElement = document.activeElement
  const shadowActiveElement = host?.shadowRoot?.activeElement ?? null
  const verseInitialFocus =
    activeElement === verseFocusRoot ||
    activeElement === host ||
    shadowActiveElement === canvas
  const hotbarDigit2Event = new KeyboardEvent("keydown", {
    key: "2",
    code: "Digit2",
    bubbles: true,
    cancelable: true,
  })
  window.dispatchEvent(hotbarDigit2Event)
  const hotbarNumberKeysConsumed = hotbarDigit2Event.defaultPrevented === true
  const checks = {
    appShellVerse: document.querySelector(".app-shell-verse") !== null,
    chatPaneWorld: document.querySelector(".chat-pane-world") !== null,
    verseHotbar:
      document.querySelector(".app-shell-verse .hotbar") !== null &&
      document.querySelector(
        ".app-shell-verse .hotbar [data-hotbar-action='action_bar.slot_1'].hotbar-slot-filled",
      ) !== null,
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
    sceneCursorNotText: canvasCursor !== "text",
    scenePointerTarget,
    verseInitialFocus,
    hotbarNumberKeysConsumed,
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
    pointerHitTest: {
      canvasCursor,
      topElements,
    },
    focus: {
      activeElement:
        activeElement instanceof HTMLElement
          ? [
            activeElement.tagName.toLowerCase(),
            typeof activeElement.className === "string"
              ? activeElement.className.split(/\\s+/).filter(Boolean).slice(0, 3).join(".")
              : "",
          ].filter(Boolean).join(".")
          : null,
      shadowActiveElement:
        shadowActiveElement instanceof HTMLElement
          ? shadowActiveElement.tagName.toLowerCase()
          : null,
    },
    forbiddenTextHits,
    overlapPairs,
    visibleTextSample: text.replace(/\\s+/g, " ").trim().slice(0, 320),
  }
})()
`

const retainedLiveUpdateExpression = `
(async () => {
  const wait = (ms) => new Promise(resolve => setTimeout(resolve, ms))
  const afterFrames = () => new Promise(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))
  )
  const logs = () => globalThis.__OA_DUMP_VERSE_SCENE_LOGS?.() ?? globalThis.__OA_VERSE_SCENE_LOGS ?? []
  await customElements.whenDefined("oa-training-run").catch(() => null)
  const host = document.querySelector(".three-effect-chat-scene")
  if (host === null) {
    return {
      ok: false,
      boardHydrated: false,
      retainedUpdateCount: 0,
      remounts: [],
      diagnosticsSample: [],
    }
  }
  const beforeCount = logs().length
  const previous = host.visualization ?? {}
  const previousWorldItems = Array.isArray(previous.worldItems)
    ? previous.worldItems
    : []
  const hydratedBoard = {
    detail: "Tassadar is active with public training windows and verified work.",
    id: "${tassadarBulletinWorldItemId}",
    interactionRadius: 3.8,
    kind: "bulletin_board",
    label: "Tassadar Run Board",
    lines: ["Status: active", "5 pylons, 2 active", "2,100 sats paid"],
    position: [-0.95, 1.78, 0.04],
    sourceRefs: ["route:/api/public/tassadar-run-summary"],
    status: "active",
    title: "Tassadar Run Board",
    yaw: -0.04,
  }
  const nextWorldItems = previousWorldItems.some(item => item?.id === hydratedBoard.id)
    ? previousWorldItems.map(item => item?.id === hydratedBoard.id ? hydratedBoard : item)
    : [...previousWorldItems, hydratedBoard]
  host.visualization = {
    ...previous,
    worldItems: nextWorldItems,
  }
  const boardAfterSetter = (host.visualization?.worldItems ?? [])
    .find(item => item?.id === hydratedBoard.id)
  const boardHydratedAfterSetter = Array.isArray(boardAfterSetter?.lines) &&
    boardAfterSetter.lines.includes("Status: active") &&
    boardAfterSetter.lines.includes("5 pylons, 2 active")
  await afterFrames()
  await wait(180)
  const newLogs = logs().slice(beforeCount)
  const retained = newLogs.filter(entry => entry.event === "verse-host.visualization.retained")
  const remounts = newLogs.filter(entry =>
    entry.event === "verse-host.remount.mounted" ||
    entry.event === "verse-host.remount.swapped" ||
    entry.event === "verse-host.remount.scheduled"
  )
  return {
    ok: boardHydratedAfterSetter && retained.length > 0 && remounts.length === 0,
    boardHydrated: boardHydratedAfterSetter,
    retainedUpdateCount: retained.length,
    remounts,
    diagnosticsSample: newLogs.slice(-20),
  }
})()
`

const mouseLookDragProbeExpression = (x: number, y: number): string => `
(() => {
  const host = document.querySelector(".three-effect-chat-scene")
  const canvas = host?.shadowRoot?.querySelector("canvas")
  const topElements = document.elementsFromPoint(${x}, ${y})
    .slice(0, 8)
    .map(element => {
      const className = element instanceof HTMLElement ? element.className : ""
      const normalizedClass = typeof className === "string" ? className : ""
      return [
        element.tagName.toLowerCase(),
        normalizedClass.split(/\\s+/).filter(Boolean).slice(0, 3).join("."),
      ].filter(Boolean).join(".")
    })
  const pointerTop = document.elementFromPoint(${x}, ${y})
  const scenePointerTarget =
    pointerTop === host ||
    pointerTop === canvas ||
    (pointerTop instanceof HTMLElement &&
      pointerTop.closest(".three-effect-chat-scene") === host)
  return {
    cursorAfterDrag: canvas instanceof HTMLCanvasElement
      ? getComputedStyle(canvas).cursor
      : "missing",
    scenePointerTarget,
    selectedText: String(globalThis.getSelection?.()?.toString() ?? ""),
    topElements,
  }
})()
`

const codingOverlayProbeExpression = `
(async () => {
  const wait = (ms) => new Promise(resolve => setTimeout(resolve, ms))
  const afterFrames = () => new Promise(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))
  )
  const waitFor = async (predicate, timeout = 10000) => {
    const deadline = Date.now() + timeout
    while (Date.now() < deadline) {
      if (predicate()) return true
      await wait(100)
    }
    return false
  }
  const smokeHookReady = await waitFor(() => globalThis.__OA_DESKTOP_SMOKE__ !== undefined)
  const smoke = globalThis.__OA_DESKTOP_SMOKE__
  if (!smokeHookReady || smoke === undefined) {
    return {
      ok: false,
      blockerRefs: ["verse.coding.smokeHook"],
      checks: {
        smokeHook: false,
        enteredCodeMode: false,
        dockVisible: false,
        accountInventoryVisible: false,
        selectedAccountVisible: false,
        composerPaneOpen: false,
        sessionsPaneOpen: false,
        decisionsPaneOpen: false,
        diffPaneOpen: false,
        terminalPaneOpen: false,
        diagnosticsPaneOpen: false,
        streamRowsVisible: false,
        approvalVisible: false,
        diffVisible: false,
        terminalVisible: false,
        diagnosticsExportVisible: false,
        diagnosticsExportPublicSafe: false,
        scenePointAvailable: false,
        focusedTextarea: false,
      },
      paneKinds: [],
      openResults: {},
      scenePoint: null,
      visibleTextSample: "",
      diagnosticsExportSample: "",
    }
  }

  const node = ${JSON.stringify(controlledCodingNodeState)}
  smoke.setNodeLaunchStatus("online")
  smoke.pushNodeState(node)
  smoke.enterCodeMode()
  smoke.selectComposerAccount("smoke")
  smoke.setComposerObjective("Exercise the controlled Verse coding overlay smoke.")
  await afterFrames()
  await wait(450)
  smoke.enterCodeMode()
  smoke.selectComposerAccount("smoke")
  smoke.pushNodeState(node)
  smoke.setComposerSession("${controlledCodingSessionRef}")
  const paneKindsNow = () => [...document.querySelectorAll(".pane-window")]
    .map(element => element.getAttribute("data-pane-kind") ?? "")
    .filter(Boolean)
  const openResults = {}
  for (const pane of [
    "composer",
    "sessions",
    "decisions",
    "diff-artifacts",
    "terminal-log",
    "diagnostics",
  ]) {
    openResults[pane] = smoke.openPane(pane)
    await afterFrames()
    await waitFor(() => paneKindsNow().includes(pane), 2500)
  }
  await afterFrames()
  await wait(700)

  const paneKinds = [...document.querySelectorAll(".pane-window")]
    .map(element => element.getAttribute("data-pane-kind") ?? "")
    .filter(Boolean)
  const hasPane = (kind) => paneKinds.includes(kind)
  const text = document.body.innerText || ""
  const diagnosticsExport = document.querySelector("[data-autopilot-host-diagnostics-export]")?.textContent ?? ""
  const rawLeaks = [
    "${controlledCodingAccountHash}",
    "/Users/",
    "sk-secret",
    "ghp_secret",
    "provider payload",
  ].filter(value => diagnosticsExport.includes(value))

  const scenePoint = (() => {
    const host = document.querySelector(".three-effect-chat-scene")
    const canvas = host?.shadowRoot?.querySelector("canvas")
    const canvasRect = canvas instanceof HTMLCanvasElement ? canvas.getBoundingClientRect() : null
    if (host === null || canvasRect === null) return null
    const candidates = [
      [0.82, 0.72],
      [0.86, 0.44],
      [0.72, 0.82],
      [0.18, 0.76],
      [0.52, 0.80],
      [0.94, 0.60],
    ]
    for (const [px, py] of candidates) {
      const x = Math.floor(canvasRect.left + canvasRect.width * px)
      const y = Math.floor(canvasRect.top + canvasRect.height * py)
      const top = document.elementFromPoint(x, y)
      const sceneTarget =
        top === host ||
        top === canvas ||
        (top instanceof HTMLElement && top.closest(".three-effect-chat-scene") === host)
      if (sceneTarget) return { x, y }
    }
    return null
  })()

  const textarea =
    document.querySelector(".pane-window[data-pane-kind='composer'] textarea.text-area") ??
    document.querySelector(".verse-code-dock-textarea")
  if (textarea instanceof HTMLTextAreaElement) {
    textarea.focus()
    textarea.setSelectionRange(textarea.value.length, textarea.value.length)
  }

  const checks = {
    smokeHook: true,
    enteredCodeMode: document.querySelector("[data-verse-mode='code']") !== null,
    dockVisible: document.querySelector("[data-verse-code-dock='codex']") !== null,
    accountInventoryVisible: document.querySelector("[data-verse-code-account-inventory]") !== null,
    selectedAccountVisible:
      document.querySelector("[data-verse-code-account-ref='smoke']") !== null ||
      text.includes("smoke"),
    composerPaneOpen: hasPane("composer"),
    sessionsPaneOpen: hasPane("sessions"),
    decisionsPaneOpen: hasPane("decisions"),
    diffPaneOpen: hasPane("diff-artifacts"),
    terminalPaneOpen: hasPane("terminal-log"),
    diagnosticsPaneOpen: hasPane("diagnostics"),
    streamRowsVisible: document.querySelectorAll(".agent-stream-row").length >= 4,
    approvalVisible:
      document.querySelector("[data-autopilot-approval-ref]") !== null ||
      document.querySelector("[data-verse-code-dock-permissions='1']") !== null,
    diffVisible: document.querySelector("[data-autopilot-diff-artifacts-panel]") !== null,
    terminalVisible: document.querySelector("[data-autopilot-terminal-log-session]") !== null,
    diagnosticsExportVisible: diagnosticsExport.includes("openagents.autopilot_desktop.host_diagnostics.v1"),
    diagnosticsExportPublicSafe: diagnosticsExport.length > 0 && rawLeaks.length === 0,
    scenePointAvailable: scenePoint !== null,
    focusedTextarea: document.activeElement instanceof HTMLTextAreaElement,
  }
  const blockerRefs = Object.entries(checks)
    .flatMap(([key, passed]) => passed ? [] : ["verse.coding." + key])
  return {
    ok: blockerRefs.length === 0,
    blockerRefs,
    checks,
    paneKinds,
    openResults,
    scenePoint,
    visibleTextSample: text.replace(/\\s+/g, " ").trim().slice(0, 420),
    diagnosticsExportSample: diagnosticsExport.replace(/\\s+/g, " ").trim().slice(0, 420),
  }
})()
`

const focusedTypingProbeExpression = `
(() => {
  const active = document.activeElement
  const activeTag = active instanceof HTMLElement ? active.tagName.toLowerCase() : "none"
  const value = active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement
    ? active.value
    : ""
  return {
    ok: active instanceof HTMLTextAreaElement && value.endsWith("w"),
    activeTag,
    valueTail: value.slice(-80),
  }
})()
`

const customKeybindingStorageKey = "autopilot-desktop.input-bindings.v1"

const keybindingUiProbeHelpers = `
  const wait = (ms) => new Promise(resolve => setTimeout(resolve, ms))
  const afterFrames = () => new Promise(resolve =>
    requestAnimationFrame(() => requestAnimationFrame(resolve))
  )
  const waitFor = async (predicate, timeout = 10000) => {
    const deadline = Date.now() + timeout
    while (Date.now() < deadline) {
      if (predicate()) return true
      await wait(100)
    }
    return false
  }
  const storedForwardCodes = () => {
    try {
      const raw = localStorage.getItem("${customKeybindingStorageKey}")
      const profile = raw === null ? null : JSON.parse(raw)
      const bindings = profile?.bindings?.["movement.forward"]
      return Array.isArray(bindings)
        ? bindings.flatMap(binding => typeof binding?.code === "string" ? [binding.code] : [])
        : []
    } catch {
      return []
    }
  }
  const keybindingVisibleText = () =>
    String(document.body.innerText ?? "").replace(/\\s+/g, " ").trim().slice(0, 420)
  const movementForwardPrimaryButton = () =>
    document.querySelector("[data-keybinding-action='movement.forward'] [data-keybinding-slot='0']")
  const uiProbe = (extraBlockers = []) => {
    const button = movementForwardPrimaryButton()
    const storedCodes = storedForwardCodes()
    const activeLabel = button instanceof HTMLElement
      ? String(button.textContent ?? "").trim()
      : ""
    const blockerRefs = [
      ...(document.querySelector("[data-keybindings-settings='ready']") === null
        ? ["verse.keybindings.settingsMissing"]
        : []),
      ...(button instanceof HTMLElement ? [] : ["verse.keybindings.forwardPrimaryMissing"]),
      ...extraBlockers,
    ]
    return {
      ok: blockerRefs.length === 0,
      blockerRefs,
      activeLabel,
      storedCodes,
      visibleTextSample: keybindingVisibleText(),
    }
  }
`

const openKeybindingsAndCaptureForwardExpression = `
(async () => {
${keybindingUiProbeHelpers}
  const smokeReady = await waitFor(() => globalThis.__OA_DESKTOP_SMOKE__ !== undefined)
  const smoke = globalThis.__OA_DESKTOP_SMOKE__
  if (!smokeReady || smoke === undefined || typeof smoke.navigateTo !== "function") {
    return uiProbe(["verse.keybindings.smokeHook"])
  }
  smoke.exitCodeMode()
  smoke.navigateTo("settings")
  await waitFor(() => document.querySelector("[data-keybindings-settings='ready']") !== null)
  await afterFrames()
  await wait(300)
  const button = movementForwardPrimaryButton()
  if (!(button instanceof HTMLElement)) return uiProbe(["verse.keybindings.forwardPrimaryMissing"])
  button.scrollIntoView({ block: "center", inline: "center" })
  button.click()
  button.focus()
  await afterFrames()
  await wait(160)
  const active = document.activeElement
  return uiProbe(active === button ? [] : ["verse.keybindings.captureFocus"])
})()
`

const persistedForwardKeyIProbeExpression = `
(async () => {
${keybindingUiProbeHelpers}
  await waitFor(() => storedForwardCodes().includes("KeyI"))
  await afterFrames()
  await wait(180)
  const storedCodes = storedForwardCodes()
  const blockers = [
    ...(storedCodes[0] === "KeyI" ? [] : ["verse.keybindings.persistedPrimaryNotI"]),
    ...(storedCodes.includes("KeyW") ? ["verse.keybindings.persistedStillHasW"] : []),
  ]
  return uiProbe(blockers)
})()
`

const openVerseExploreExpression = `
(async () => {
${keybindingUiProbeHelpers}
  const smokeReady = await waitFor(() => globalThis.__OA_DESKTOP_SMOKE__ !== undefined)
  const smoke = globalThis.__OA_DESKTOP_SMOKE__
  if (smokeReady && smoke !== undefined) {
    smoke.exitCodeMode()
    smoke.navigateTo?.("chat")
  }
  await waitFor(() => document.querySelector(".app-shell-verse") !== null)
  await customElements.whenDefined("oa-training-run").catch(() => null)
  await waitFor(() => {
    const host = document.querySelector(".three-effect-chat-scene")
    return host?.shadowRoot?.querySelector("canvas") instanceof HTMLCanvasElement
  })
  const active = document.activeElement
  if (active instanceof HTMLElement) active.blur()
  await afterFrames()
  await wait(300)
  const host = document.querySelector(".three-effect-chat-scene")
  const canvas = host?.shadowRoot?.querySelector("canvas")
  const canvasRect = canvas instanceof HTMLCanvasElement
    ? (() => {
      const r = canvas.getBoundingClientRect()
      return { top: r.top, right: r.right, bottom: r.bottom, left: r.left, width: r.width, height: r.height }
    })()
    : null
  return {
    ok: canvasRect !== null,
    canvasRect,
    visibleTextSample: keybindingVisibleText(),
  }
})()
`

const customTerminalFocusExpression = `
(async () => {
${keybindingUiProbeHelpers}
  await waitFor(() => document.querySelector("[data-autopilot-terminal-text-selection]") !== null)
  await afterFrames()
  await wait(250)
  const textarea = document.querySelector("[data-autopilot-terminal-text-selection]")
  if (textarea instanceof HTMLTextAreaElement) {
    textarea.focus()
    textarea.setSelectionRange(textarea.value.length, textarea.value.length)
  }
  const active = document.activeElement
  return {
    ok: active instanceof HTMLTextAreaElement,
    activeTag: active instanceof HTMLElement ? active.tagName.toLowerCase() : "none",
    valueTail: active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement
      ? active.value.slice(-80)
      : "",
    blockerRefs: active instanceof HTMLTextAreaElement ? [] : ["verse.keybindings.terminalFocus"],
  }
})()
`

const currentFocusedTextareaExpression = `
(() => {
  const active = document.activeElement
  return {
    ok: active instanceof HTMLTextAreaElement,
    activeTag: active instanceof HTMLElement ? active.tagName.toLowerCase() : "none",
    valueTail: active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement
      ? active.value.slice(-80)
      : "",
    blockerRefs: active instanceof HTMLTextAreaElement ? [] : ["verse.keybindings.textareaFocus"],
  }
})()
`

const focusedTypingValueProbeExpression = (expectedTail: string): string => `
(() => {
  const active = document.activeElement
  const activeTag = active instanceof HTMLElement ? active.tagName.toLowerCase() : "none"
  const value = active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement
    ? active.value
    : ""
  return {
    ok: active instanceof HTMLTextAreaElement && value.endsWith(${JSON.stringify(expectedTail)}),
    activeTag,
    valueTail: value.slice(-80),
  }
})()
`

const persistenceAfterReloadProbeExpression = `
(async () => {
${keybindingUiProbeHelpers}
  const smokeReady = await waitFor(() => globalThis.__OA_DESKTOP_SMOKE__ !== undefined)
  const smoke = globalThis.__OA_DESKTOP_SMOKE__
  await waitFor(() => document.querySelector(".app-shell-verse") !== null)
  await customElements.whenDefined("oa-training-run").catch(() => null)
  await waitFor(() => {
    const host = document.querySelector(".three-effect-chat-scene")
    return host?.shadowRoot?.querySelector("canvas") instanceof HTMLCanvasElement
  })
  if (smokeReady && smoke !== undefined) {
    smoke.exitCodeMode()
    smoke.navigateTo?.("settings")
  }
  await waitFor(() => document.querySelector("[data-keybindings-settings='ready']") !== null)
  await afterFrames()
  await wait(500)
  const storedCodes = storedForwardCodes()
  const blockers = [
    ...(storedCodes[0] === "KeyI" ? [] : ["verse.keybindings.reloadPrimaryNotI"]),
    ...(storedCodes.includes("KeyW") ? ["verse.keybindings.reloadStillHasW"] : []),
  ]
  return uiProbe(blockers)
})()
`

const resetDefaultsProbeExpression = `
(async () => {
${keybindingUiProbeHelpers}
  const smokeReady = await waitFor(() => globalThis.__OA_DESKTOP_SMOKE__ !== undefined)
  const smoke = globalThis.__OA_DESKTOP_SMOKE__
  if (!smokeReady || smoke === undefined) {
    return uiProbe(["verse.keybindings.smokeHook"])
  }
  smoke.exitCodeMode()
  smoke.navigateTo?.("settings")
  await waitFor(() => document.querySelector("[data-keybindings-settings='ready']") !== null)
  await afterFrames()
  await wait(240)
  const reset = document.querySelector("[data-keybinding-reset-all='true']")
  if (!(reset instanceof HTMLElement)) return uiProbe(["verse.keybindings.resetAllMissing"])
  reset.click()
  await waitFor(() => storedForwardCodes().includes("KeyW"))
  await afterFrames()
  await wait(240)
  const storedCodes = storedForwardCodes()
  const blockers = [
    ...(storedCodes[0] === "KeyW" ? [] : ["verse.keybindings.resetPrimaryNotW"]),
    ...(storedCodes.includes("KeyI") ? ["verse.keybindings.resetStillHasI"] : []),
  ]
  return uiProbe(blockers)
})()
`

// Headless WebGL renders at a sparse frame rate (~2.5fps observed). Capturing a
// screenshot in that gap can land on an incomplete/black frame, which would
// falsely trip the rendering-health checks (pixelSmoke / black-frame) even when
// the avatar pose behaved correctly. Settling on actually-rendered frames before
// each capture keeps the rendering-health checks honest without weakening the
// authoritative pose-delta behavioral assertion.
const settleRenderedFrames = async (
  cdp: CdpClient,
  frames = 4,
): Promise<void> => {
  await cdp.send("Runtime.evaluate", {
    expression: `new Promise(resolve => {
      let remaining = ${Math.max(1, Math.floor(frames))}
      const step = () => {
        remaining -= 1
        if (remaining <= 0) resolve(true)
        else requestAnimationFrame(step)
      }
      requestAnimationFrame(step)
    })`,
    awaitPromise: true,
    returnByValue: true,
  })
}

// Capture a screenshot that is actually a rendered, non-degenerate frame. Under
// sparse headless fps a single capture can return a black/incomplete frame; we
// settle rendered frames and retry a bounded number of times until pixelSmoke
// reports a healthy frame (or we exhaust the bounded retries and return the last
// frame, so the secondary rendering-health check still has data to report).
const captureRenderedScreenshot = async (
  cdp: CdpClient,
  attempts = 4,
): Promise<{ readonly bytes: Buffer; readonly pixels: PixelSmoke }> => {
  let last: { readonly bytes: Buffer; readonly pixels: PixelSmoke } | null = null
  for (let attempt = 0; attempt < Math.max(1, attempts); attempt += 1) {
    await settleRenderedFrames(cdp)
    const shot = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const bytes = Buffer.from(shot.data, "base64")
    const pixels = pixelSmoke(bytes)
    last = { bytes, pixels }
    if (pixels.ok) return last
    await wait(120)
  }
  return last!
}

const runKeybindingMovementProbe = async (
  cdp: CdpClient,
  spec: KeyboardSpec,
  expected: "movement" | "stationary",
  screenshotPath: string,
  holdMs = 1_350,
): Promise<KeybindingMovementProbe> => {
  const beforeCapture = await captureRenderedScreenshot(cdp)
  const beforeBytes = beforeCapture.bytes
  const diagnosticsBefore = await readVerseSceneDiagnostics(cdp)
  const poseBefore = latestLocalPose(diagnosticsBefore)
  await holdKey(cdp, spec, holdMs)
  const afterCapture = await captureRenderedScreenshot(cdp)
  const afterBytes = afterCapture.bytes
  writeFileSync(screenshotPath, afterBytes)
  const frame = afterCapture.pixels
  const visualDiff = movementSmoke(beforeBytes, afterBytes, screenshotPath)
  const diagnosticsAfter = await readVerseSceneDiagnostics(cdp)
  const diagnostics = diagnosticsAfter.slice(diagnosticsBefore.length)
  const poseAfter = latestLocalPose(diagnosticsAfter)
  const delta = poseDelta(poseBefore, poseAfter)
  const remounts = remountDiagnostics(diagnostics)
  const blackFrames = blackFrameDiagnostics(diagnostics)

  // Authoritative behavioral signal: did the avatar's scene pose move? This stays
  // strict in both directions and is what the keybinding test actually asserts.
  const poseMovementOk = delta !== null && delta > 0.35
  const poseStationaryOk = delta === null || delta < 0.08

  // Rendering-health is a SECONDARY, headless-tolerant signal. The renderer must
  // produce a real frame (no remount churn, no black-frame diagnostics, a
  // non-degenerate pixelSmoke frame). We only fold the pixel-diff portion in when
  // the captured frame actually rendered, so ambient bloom/particle animation
  // straddling sparse frames cannot cause a false stationary failure, and a
  // sparse-fps black frame cannot mask correct movement.
  const renderHealthOk =
    frame.ok && remounts.length === 0 && blackFrames.length === 0
  // Headless-tolerant ceiling for "no avatar motion" via pixels. Ambient scene
  // animation (bloom/particles) can change a few percent of pixels between sparse
  // frames even when the avatar is stationary; a moving avatar changes far more
  // (~20-28% observed). The gap is wide, so a tolerant ceiling stays meaningful.
  const stationaryPixelHealthy =
    !frame.ok || visualDiff.changedPixelRatio < 0.12
  const movementPixelHealthy = !frame.ok || visualDiff.ok

  const movementOk = poseMovementOk && renderHealthOk && movementPixelHealthy
  const stationaryOk =
    poseStationaryOk && renderHealthOk && stationaryPixelHealthy

  return {
    ok: expected === "movement" ? movementOk : stationaryOk,
    expected,
    keyCode: spec.code,
    poseBefore,
    poseAfter,
    poseDelta: delta,
    frame: {
      ...frame,
      path: relativePath(screenshotPath),
    },
    visualDiff,
    diagnosticsSample: diagnostics.slice(-20),
  }
}

const runFocusedTypingProbe = async (
  cdp: CdpClient,
  target: "composer" | "terminal",
  focusExpression: string,
  expectTextInsertion: boolean,
): Promise<KeybindingFocusedTypingProbe> => {
  const focusProbe = await evaluateValue<{
    readonly activeTag: string
    readonly blockerRefs?: ReadonlyArray<string>
    readonly ok: boolean
    readonly valueTail: string
  }>(cdp, `Verse keybinding ${target} focus`, focusExpression)
  const diagnosticsBefore = await readVerseSceneDiagnostics(cdp)
  const poseBefore = latestLocalPose(diagnosticsBefore)
  await tapKey(cdp, keySpecI)
  const valueProbe = await evaluateValue<{
    readonly activeTag: string
    readonly ok: boolean
    readonly valueTail: string
  }>(cdp, `Verse keybinding ${target} typing`, focusedTypingValueProbeExpression("i"))
  const diagnosticsAfter = await readVerseSceneDiagnostics(cdp)
  const diagnostics = diagnosticsAfter.slice(diagnosticsBefore.length)
  const poseAfter = latestLocalPose(diagnosticsAfter)
  const delta = poseDelta(poseBefore, poseAfter)
  const remounts = remountDiagnostics(diagnostics)
  const blackFrames = blackFrameDiagnostics(diagnostics)
  return {
    ok:
      focusProbe.ok &&
      (expectTextInsertion ? valueProbe.ok : valueProbe.activeTag === "textarea") &&
      (delta === null || delta < 0.08) &&
      remounts.length === 0 &&
      blackFrames.length === 0,
    target,
    activeTag: valueProbe.activeTag || focusProbe.activeTag,
    valueTail: valueProbe.valueTail || focusProbe.valueTail,
    poseBefore,
    poseAfter,
    poseDelta: delta,
    diagnosticsSample: diagnostics.slice(-20),
  }
}

const runMouseLookDragSmoke = async (
  cdp: CdpClient,
  rect: Rect,
  screenshotPath: string,
): Promise<MouseLookDragSmoke> => {
  const mouseLookBefore = await cdp.send<{ readonly data: string }>(
    "Page.captureScreenshot",
    { format: "png", fromSurface: true },
  )
  const mouseLookBeforeBytes = Buffer.from(mouseLookBefore.data, "base64")
  const diagnosticsBeforeMouseLook = await readVerseSceneDiagnostics(cdp)
  const dragPoint = await dragSceneMouse(cdp, rect)
  await wheelSceneMouse(cdp, dragPoint)
  const mouseLookAfter = await cdp.send<{ readonly data: string }>(
    "Page.captureScreenshot",
    { format: "png", fromSurface: true },
  )
  const mouseLookAfterBytes = Buffer.from(mouseLookAfter.data, "base64")
  writeFileSync(screenshotPath, mouseLookAfterBytes)
  const mouseLookPixels = pixelSmoke(mouseLookAfterBytes)
  const mouseLookMovement = movementSmoke(
    mouseLookBeforeBytes,
    mouseLookAfterBytes,
    screenshotPath,
  )
  const diagnosticsAfterMouseLook = await readVerseSceneDiagnostics(cdp)
  const mouseLookDiagnostics = diagnosticsAfterMouseLook.slice(
    diagnosticsBeforeMouseLook.length,
  )
  const mouseLookProbe = await evaluateValue<{
    readonly cursorAfterDrag?: string
    readonly scenePointerTarget?: boolean
    readonly selectedText?: string
    readonly topElements?: ReadonlyArray<string>
  }>(
    cdp,
    "Verse keybinding mouselook drag probe",
    mouseLookDragProbeExpression(dragPoint.endX, dragPoint.endY),
  )
  const mouseLookRemounts = remountDiagnostics(mouseLookDiagnostics)
  const cameraControlEvents = mouseLookDiagnostics.filter(
    entry => entry.event === "verse-host.camera-control",
  )
  const observedDragControl = cameraControlEvents.some(
    entry => entry.detail["type"] === "drag",
  )
  const observedWheelControl = cameraControlEvents.some(
    entry => entry.detail["type"] === "wheel",
  )
  return {
    ok:
      mouseLookPixels.ok &&
      mouseLookMovement.ok &&
      mouseLookRemounts.length === 0 &&
      blackFrameDiagnostics(mouseLookDiagnostics).length === 0 &&
      observedDragControl &&
      observedWheelControl &&
      mouseLookProbe.cursorAfterDrag !== "text" &&
      mouseLookProbe.scenePointerTarget === true &&
      (mouseLookProbe.selectedText ?? "").trim().length === 0,
    observedDragControl,
    observedWheelControl,
    cursorAfterDrag: mouseLookProbe.cursorAfterDrag ?? "missing",
    scenePointerTarget: mouseLookProbe.scenePointerTarget === true,
    selectedText: mouseLookProbe.selectedText ?? "",
    topElements: mouseLookProbe.topElements ?? [],
    cameraControlEvents,
    frame: {
      ...mouseLookPixels,
      path: relativePath(screenshotPath),
    },
    movement: mouseLookMovement,
    activeHostRemounts: mouseLookRemounts,
    diagnosticsSample: mouseLookDiagnostics.slice(-20),
  }
}

const main = async (): Promise<void> => {
  const devSmokeUrl = process.env.AUTOPILOT_DESKTOP_SMOKE_URL?.trim()
  const target: "packaged" | "dev" =
    devSmokeUrl === undefined || devSmokeUrl === "" ? "packaged" : "dev"
  const appBundle = target === "packaged"
    ? (process.env.AUTOPILOT_DESKTOP_APP_BUNDLE ??
      join(process.cwd(), "build/dev-macos-arm64/Autopilot-dev.app"))
    : null
  const resourcesDir = appBundle === null ? null : join(appBundle, "Contents/Resources/app")
  const viewDir = resourcesDir === null ? null : join(resourcesDir, "views/autopilot-desktop")
  const htmlPath = viewDir === null ? null : join(viewDir, "index.html")
  const cssPath = viewDir === null ? null : join(viewDir, "styles.css")
  const mainPath = viewDir === null ? null : join(viewDir, "main.js")
  const pylonNodePath = resourcesDir === null ? null : join(resourcesDir, "pylon-node/index.js")
  const proofDir = process.env.AUTOPILOT_DESKTOP_VERSE_SMOKE_PROOF_DIR ??
    join(process.cwd(), "build/verse-launch-smoke")
  const screenshotPath = join(proofDir, "verse-launch-smoke.png")
  const movementScreenshotPath = join(proofDir, "verse-launch-smoke-after-w.png")
  const mouseLookScreenshotPath = join(
    proofDir,
    "verse-launch-smoke-after-mouselook.png",
  )
  const customKeybindingWPath = join(
    proofDir,
    "verse-keybinding-after-w-suppressed.png",
  )
  const customKeybindingIPath = join(
    proofDir,
    "verse-keybinding-after-i-movement.png",
  )
  const customKeybindingMouseLookPath = join(
    proofDir,
    "verse-keybinding-after-mouselook.png",
  )
  const customKeybindingReloadIPath = join(
    proofDir,
    "verse-keybinding-after-reload-i-movement.png",
  )
  const customKeybindingResetWPath = join(
    proofDir,
    "verse-keybinding-after-reset-w-movement.png",
  )
  const codingOverlayScreenshotPath = join(
    proofDir,
    "verse-coding-overlay-smoke.png",
  )
  const codingOverlaySceneInputScreenshotPath = join(
    proofDir,
    "verse-coding-overlay-after-scene-input.png",
  )
  const receiptPath = join(proofDir, "verse-launch-smoke.json")

  if (target === "packaged") {
    for (const [label, path] of [
      ["packaged view HTML", htmlPath],
      ["packaged view CSS", cssPath],
      ["packaged view JS", mainPath],
      ["packaged Pylon node", pylonNodePath],
    ] as const) {
      if (path === null || !existsSync(path)) {
        throw new Error(`${label} missing at ${path ?? "unknown"}; run bun run build first`)
      }
    }
  }

  const tmpRoot = join(process.cwd(), ".verse-launch-smoke-profile")
  rmSync(tmpRoot, { force: true, recursive: true })
  mkdirSync(tmpRoot, { recursive: true })
  mkdirSync(proofDir, { recursive: true })

  const html = target === "packaged" && htmlPath !== null
    ? patchedPackagedHtml(readFileSync(htmlPath, "utf8"))
    : null
  const packagedMain = target === "packaged" && mainPath !== null
    ? readFileSync(mainPath, "utf8")
    : null
  const packagedSource = packagedMain === null
    ? null
    : {
        includesFirstRenderBulletinBoard:
          packagedMain.includes("Tassadar Board") &&
          packagedMain.includes("Loading Tassadar run"),
        includesVisibleBulletinPrimitive:
          packagedMain.includes("width = 3.1") &&
          packagedMain.includes("postHeight = 2.18"),
      }
  if (packagedSource !== null && !packagedSource.includesFirstRenderBulletinBoard) {
    throw new Error(
      "Packaged Verse bundle is missing the first-render Tassadar bulletin board copy; run bun run build after changing the board.",
    )
  }
  if (packagedSource !== null && !packagedSource.includesVisibleBulletinPrimitive) {
    throw new Error(
      "Packaged Verse bundle is missing the visible three-effect bulletin board primitive; repin/build @openagentsinc/three-effect.",
    )
  }
  let server: ReturnType<typeof Bun.serve> | null = null
  let chrome: Bun.Subprocess | null = null
  let cdp: CdpClient | null = null

  try {
    if (target === "packaged") {
      if (
        html === null ||
        viewDir === null ||
        mainPath === null ||
        cssPath === null
      ) {
        throw new Error("Packaged smoke target was not initialized")
      }
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
    }

    const debugPort = await getFreePort()
    const smokeUrl = withSmokeHookParam(
      target === "packaged"
        ? `http://127.0.0.1:${server?.port ?? 0}/`
        : devSmokeUrl!,
    )
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

    const retainedLiveUpdateEvaluation = await cdp.send<{
      readonly result?: { readonly value?: RetainedLiveUpdateSmoke }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: retainedLiveUpdateExpression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (retainedLiveUpdateEvaluation.exceptionDetails !== undefined) {
      throw new Error(
        `Verse retained live-update smoke threw in Chrome: ${JSON.stringify(retainedLiveUpdateEvaluation.exceptionDetails)}`,
      )
    }
    const retainedLiveUpdate = retainedLiveUpdateEvaluation.result?.value
    if (retainedLiveUpdate === undefined) {
      throw new Error("Verse retained live-update smoke returned no probe")
    }

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

    if (dom.canvas.rect === null) {
      throw new Error("Verse mouselook smoke cannot run without a canvas rect")
    }
    const mouseLookBefore = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const mouseLookBeforeBytes = Buffer.from(mouseLookBefore.data, "base64")
    const diagnosticsBeforeMouseLook = await readVerseSceneDiagnostics(cdp)
    const dragPoint = await dragSceneMouse(cdp, dom.canvas.rect)
    await wheelSceneMouse(cdp, dragPoint)
    const mouseLookAfter = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const mouseLookAfterBytes = Buffer.from(mouseLookAfter.data, "base64")
    writeFileSync(mouseLookScreenshotPath, mouseLookAfterBytes)
    const mouseLookPixels = pixelSmoke(mouseLookAfterBytes)
    const mouseLookMovement = movementSmoke(
      mouseLookBeforeBytes,
      mouseLookAfterBytes,
      mouseLookScreenshotPath,
    )
    const diagnosticsAfterMouseLook = await readVerseSceneDiagnostics(cdp)
    const mouseLookDiagnostics = diagnosticsAfterMouseLook.slice(
      diagnosticsBeforeMouseLook.length,
    )
    const mouseLookProbeEvaluation = await cdp.send<{
      readonly result?: {
        readonly value?: {
          readonly cursorAfterDrag?: string
          readonly scenePointerTarget?: boolean
          readonly selectedText?: string
          readonly topElements?: ReadonlyArray<string>
        }
      }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: mouseLookDragProbeExpression(dragPoint.endX, dragPoint.endY),
      returnByValue: true,
    })
    if (mouseLookProbeEvaluation.exceptionDetails !== undefined) {
      throw new Error(
        `Verse mouselook drag probe threw in Chrome: ${JSON.stringify(mouseLookProbeEvaluation.exceptionDetails)}`,
      )
    }
    const mouseLookProbe = mouseLookProbeEvaluation.result?.value
    if (mouseLookProbe === undefined) {
      throw new Error("Verse mouselook drag probe returned no result")
    }
    const mouseLookRemounts = mouseLookDiagnostics.filter(entry =>
      entry.event === "verse-host.remount.mounted" ||
      entry.event === "verse-host.remount.swapped" ||
      entry.event === "verse-host.remount.scheduled",
    )
    const cameraControlEvents = mouseLookDiagnostics.filter(
      entry => entry.event === "verse-host.camera-control",
    )
    const observedDragControl = cameraControlEvents.some(
      entry => entry.detail["type"] === "drag",
    )
    const observedWheelControl = cameraControlEvents.some(
      entry => entry.detail["type"] === "wheel",
    )
    const mouseLookDrag: MouseLookDragSmoke = {
      ok:
        mouseLookPixels.ok &&
        mouseLookMovement.ok &&
        mouseLookRemounts.length === 0 &&
        observedDragControl &&
        observedWheelControl &&
        mouseLookProbe.cursorAfterDrag !== "text" &&
        mouseLookProbe.scenePointerTarget === true &&
        (mouseLookProbe.selectedText ?? "").trim().length === 0,
      observedDragControl,
      observedWheelControl,
      cursorAfterDrag: mouseLookProbe.cursorAfterDrag ?? "missing",
      scenePointerTarget: mouseLookProbe.scenePointerTarget === true,
      selectedText: mouseLookProbe.selectedText ?? "",
      topElements: mouseLookProbe.topElements ?? [],
      cameraControlEvents,
      frame: {
        ...mouseLookPixels,
        path: relativePath(mouseLookScreenshotPath),
      },
      movement: mouseLookMovement,
      activeHostRemounts: mouseLookRemounts,
      diagnosticsSample: mouseLookDiagnostics.slice(-20),
    }

    const diagnosticsBeforeCustomKeybindings = await readVerseSceneDiagnostics(cdp)
    const captureProbe = await runStage(
      "custom keybindings: open Settings and start forward capture",
      () =>
        evaluateValue<KeybindingUiProbe>(
          cdp,
          "Verse keybinding capture setup",
          openKeybindingsAndCaptureForwardExpression,
        ),
    )
    await runStage("custom keybindings: press I for capture", () =>
      tapKey(cdp, keySpecI),
    )
    const persistedAfterCapture = await runStage(
      "custom keybindings: verify captured profile",
      () =>
        evaluateValue<KeybindingUiProbe>(
          cdp,
          "Verse keybinding capture persistence",
          persistedForwardKeyIProbeExpression,
        ),
    )
    const verseAfterCapture = await runStage(
      "custom keybindings: return to Verse after capture",
      () =>
        evaluateValue<{
          readonly canvasRect: Rect | null
          readonly ok: boolean
          readonly visibleTextSample: string
        }>(cdp, "Verse keybinding return to explore", openVerseExploreExpression),
    )
    if (verseAfterCapture.canvasRect === null) {
      throw new Error(
        `Verse keybinding smoke could not return to a canvas: ${verseAfterCapture.visibleTextSample}`,
      )
    }
    const wSuppressed = await runStage(
      "custom keybindings: verify W no longer moves",
      () =>
        runKeybindingMovementProbe(
          cdp,
          keySpecW,
          "stationary",
          customKeybindingWPath,
        ),
    )
    const iMovement = await runStage(
      "custom keybindings: verify I moves",
      () =>
        runKeybindingMovementProbe(
          cdp,
          keySpecI,
          "movement",
          customKeybindingIPath,
        ),
    )
    const mouseLookAfterRebind = await runStage(
      "custom keybindings: verify mouselook after rebind",
      () =>
        runMouseLookDragSmoke(
          cdp,
          verseAfterCapture.canvasRect,
          customKeybindingMouseLookPath,
        ),
    )
    const diagnosticsBeforeReload = await readVerseSceneDiagnostics(cdp)
    const customPreReloadDiagnostics = diagnosticsBeforeReload.slice(
      diagnosticsBeforeCustomKeybindings.length,
    )

    await cdp.send("Page.reload", { ignoreCache: true })
    await wait(700)
    const persistenceAfterReload = await runStage(
      "custom keybindings: verify profile after reload",
      () =>
        evaluateValue<KeybindingUiProbe>(
          cdp,
          "Verse keybinding reload persistence",
          persistenceAfterReloadProbeExpression,
        ),
    )
    const verseAfterReload = await runStage(
      "custom keybindings: return to Verse after reload",
      () =>
        evaluateValue<{
          readonly canvasRect: Rect | null
          readonly ok: boolean
          readonly visibleTextSample: string
        }>(cdp, "Verse keybinding return after reload", openVerseExploreExpression),
    )
    if (verseAfterReload.canvasRect === null) {
      throw new Error(
        `Verse keybinding smoke could not return to a canvas after reload: ${verseAfterReload.visibleTextSample}`,
      )
    }
    const diagnosticsAfterReloadBaseline = await readVerseSceneDiagnostics(cdp)
    const iMovementAfterReload = await runStage(
      "custom keybindings: verify I moves after reload",
      () =>
        runKeybindingMovementProbe(
          cdp,
          keySpecI,
          "movement",
          customKeybindingReloadIPath,
        ),
    )
    const customCodeOverlayProbe = await runStage(
      "custom keybindings: open coding overlay for focused typing",
      () =>
        evaluateValue<VerseCodingOverlayProbe>(
          cdp,
          "Verse keybinding coding overlay focus setup",
          codingOverlayProbeExpression,
        ),
      60_000,
    )
    const composerFocusedTyping = await runStage(
      "custom keybindings: composer focus captures I",
      () =>
        runFocusedTypingProbe(
          cdp,
          "composer",
          currentFocusedTextareaExpression,
          true,
        ),
    )
    const terminalFocusedTyping = await runStage(
      "custom keybindings: terminal focus suppresses movement",
      () =>
        runFocusedTypingProbe(
          cdp,
          "terminal",
          customTerminalFocusExpression,
          false,
        ),
    )
    const resetProbe = await runStage(
      "custom keybindings: reset defaults",
      () =>
        evaluateValue<KeybindingUiProbe>(
          cdp,
          "Verse keybinding reset defaults",
          resetDefaultsProbeExpression,
        ),
    )
    const verseAfterReset = await runStage(
      "custom keybindings: return to Verse after reset",
      () =>
        evaluateValue<{
          readonly canvasRect: Rect | null
          readonly ok: boolean
          readonly visibleTextSample: string
        }>(cdp, "Verse keybinding return after reset", openVerseExploreExpression),
    )
    if (verseAfterReset.canvasRect === null) {
      throw new Error(
        `Verse keybinding smoke could not return to a canvas after reset: ${verseAfterReset.visibleTextSample}`,
      )
    }
    const wMovementAfterReset = await runStage(
      "custom keybindings: verify W moves after reset",
      () =>
        runKeybindingMovementProbe(
          cdp,
          keySpecW,
          "movement",
          customKeybindingResetWPath,
        ),
    )
    const diagnosticsAfterCustomKeybindings = await readVerseSceneDiagnostics(cdp)
    const customPostReloadDiagnostics = diagnosticsAfterCustomKeybindings.slice(
      diagnosticsAfterReloadBaseline.length,
    )
    const customActiveHostRemounts = [
      ...activeRemountDiagnostics(customPreReloadDiagnostics),
      ...activeRemountDiagnostics(customPostReloadDiagnostics),
    ]
    const customBlackFrames = [
      ...blackFrameDiagnostics(customPreReloadDiagnostics),
      ...blackFrameDiagnostics(customPostReloadDiagnostics),
    ]
    const customKeybindingBlockerRefs = [
      ...captureProbe.blockerRefs,
      ...persistedAfterCapture.blockerRefs,
      ...(verseAfterCapture.ok ? [] : ["verse.keybindings.returnToVerse"]),
      ...(wSuppressed.ok ? [] : ["verse.keybindings.wStillMoves"]),
      ...(iMovement.ok ? [] : ["verse.keybindings.iDoesNotMove"]),
      ...(mouseLookAfterRebind.ok ? [] : ["verse.keybindings.mouseLookAfterRebind"]),
      ...(customCodeOverlayProbe.ok
        ? []
        : customCodeOverlayProbe.blockerRefs.map(ref => `verse.keybindings.${ref}`)),
      ...(composerFocusedTyping.ok ? [] : ["verse.keybindings.composerFocusMovesAvatar"]),
      ...(terminalFocusedTyping.ok ? [] : ["verse.keybindings.terminalFocusMovesAvatar"]),
      ...persistenceAfterReload.blockerRefs,
      ...(verseAfterReload.ok ? [] : ["verse.keybindings.returnAfterReload"]),
      ...(iMovementAfterReload.ok ? [] : ["verse.keybindings.reloadIDoesNotMove"]),
      ...resetProbe.blockerRefs,
      ...(verseAfterReset.ok ? [] : ["verse.keybindings.returnAfterReset"]),
      ...(wMovementAfterReset.ok ? [] : ["verse.keybindings.resetWDoesNotMove"]),
      ...(customActiveHostRemounts.length === 0
        ? []
        : ["verse.keybindings.activeHostRemount"]),
      ...(customBlackFrames.length === 0
        ? []
        : ["verse.keybindings.blackFrame"]),
    ]
    const customKeybindings: CustomKeybindingSmoke = {
      ok: customKeybindingBlockerRefs.length === 0,
      blockerRefs: customKeybindingBlockerRefs,
      captureProbe,
      persistedAfterCapture,
      wSuppressed,
      iMovement,
      mouseLookAfterRebind,
      codeOverlayProbe: customCodeOverlayProbe,
      composerFocusedTyping,
      terminalFocusedTyping,
      persistenceAfterReload,
      iMovementAfterReload,
      resetProbe,
      wMovementAfterReset,
      activeHostRemounts: customActiveHostRemounts,
      blackFrameEvents: customBlackFrames,
      diagnosticsSample: [
        ...customPreReloadDiagnostics.slice(-12),
        ...customPostReloadDiagnostics.slice(-12),
      ],
    }

    const codingOverlayProbeEvaluation = await cdp.send<{
      readonly result?: { readonly value?: VerseCodingOverlayProbe }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: codingOverlayProbeExpression,
      awaitPromise: true,
      returnByValue: true,
    })
    if (codingOverlayProbeEvaluation.exceptionDetails !== undefined) {
      throw new Error(
        `Verse coding overlay smoke threw in Chrome: ${JSON.stringify(codingOverlayProbeEvaluation.exceptionDetails)}`,
      )
    }
    const codingOverlayProbe = codingOverlayProbeEvaluation.result?.value
    if (codingOverlayProbe === undefined) {
      throw new Error("Verse coding overlay smoke returned no probe")
    }
    const codingOverlayScreenshot = await cdp.send<{ readonly data: string }>(
      "Page.captureScreenshot",
      { format: "png", fromSurface: true },
    )
    const codingOverlayScreenshotBytes = Buffer.from(
      codingOverlayScreenshot.data,
      "base64",
    )
    writeFileSync(codingOverlayScreenshotPath, codingOverlayScreenshotBytes)
    const codingOverlayPixels = pixelSmoke(codingOverlayScreenshotBytes)

    await keyDownW(cdp)
    await keyUpW(cdp)
    const focusedTypingEvaluation = await cdp.send<{
      readonly result?: { readonly value?: FocusedTypingProbe }
      readonly exceptionDetails?: unknown
    }>("Runtime.evaluate", {
      expression: focusedTypingProbeExpression,
      returnByValue: true,
    })
    if (focusedTypingEvaluation.exceptionDetails !== undefined) {
      throw new Error(
        `Verse coding focused typing smoke threw in Chrome: ${JSON.stringify(focusedTypingEvaluation.exceptionDetails)}`,
      )
    }
    const focusedTyping = focusedTypingEvaluation.result?.value ?? {
      ok: false,
      activeTag: "missing",
      valueTail: "",
    }

    const diagnosticsBeforeCodingSceneInput = await readVerseSceneDiagnostics(cdp)
    let codingSceneInputFrame: PixelSmoke & { readonly path: string } = {
      ok: false,
      brightPixels: 0,
      distinctLumaBuckets: 0,
      height: 0,
      sampledPixels: 0,
      sha256: "",
      width: 0,
      path: relativePath(codingOverlaySceneInputScreenshotPath),
    }
    let codingSceneInputMovement: MovementSmoke = {
      ok: false,
      changedPixels: 0,
      changedPixelRatio: 0,
      afterSha256: "",
      path: relativePath(codingOverlaySceneInputScreenshotPath),
    }
    if (codingOverlayProbe.scenePoint !== null) {
      const beforeSceneInput = await cdp.send<{ readonly data: string }>(
        "Page.captureScreenshot",
        { format: "png", fromSurface: true },
      )
      const beforeSceneInputBytes = Buffer.from(beforeSceneInput.data, "base64")
      const dragPoint = await dragSceneMouseFromPoint(
        cdp,
        codingOverlayProbe.scenePoint,
      )
      await wheelSceneMouse(cdp, dragPoint)
      const afterSceneInput = await cdp.send<{ readonly data: string }>(
        "Page.captureScreenshot",
        { format: "png", fromSurface: true },
      )
      const afterSceneInputBytes = Buffer.from(afterSceneInput.data, "base64")
      writeFileSync(codingOverlaySceneInputScreenshotPath, afterSceneInputBytes)
      const frame = pixelSmoke(afterSceneInputBytes)
      codingSceneInputFrame = {
        ...frame,
        path: relativePath(codingOverlaySceneInputScreenshotPath),
      }
      codingSceneInputMovement = movementSmoke(
        beforeSceneInputBytes,
        afterSceneInputBytes,
        codingOverlaySceneInputScreenshotPath,
      )
    }
    const diagnosticsAfterCodingSceneInput = await readVerseSceneDiagnostics(cdp)
    const codingSceneInputDiagnostics = diagnosticsAfterCodingSceneInput.slice(
      diagnosticsBeforeCodingSceneInput.length,
    )
    const codingSceneRemounts = codingSceneInputDiagnostics.filter(entry =>
      entry.event === "verse-host.remount.mounted" ||
      entry.event === "verse-host.remount.swapped" ||
      entry.event === "verse-host.remount.scheduled",
    )
    const codingBlackFrames = codingSceneInputDiagnostics.filter(entry =>
      entry.event.includes("black-frame"),
    )
    const codingCameraControlEvents = codingSceneInputDiagnostics.filter(
      entry => entry.event === "verse-host.camera-control",
    )
    const codingObservedDragControl = codingCameraControlEvents.some(
      entry => entry.detail["type"] === "drag",
    )
    const codingObservedWheelControl = codingCameraControlEvents.some(
      entry => entry.detail["type"] === "wheel",
    )
    const codingSceneInput = {
      ok:
        codingSceneInputFrame.ok &&
        codingSceneInputMovement.ok &&
        codingSceneRemounts.length === 0 &&
        codingBlackFrames.length === 0 &&
        codingObservedDragControl &&
        codingObservedWheelControl,
      observedDragControl: codingObservedDragControl,
      observedWheelControl: codingObservedWheelControl,
      activeHostRemounts: codingSceneRemounts,
      blackFrameEvents: codingBlackFrames,
      cameraControlEvents: codingCameraControlEvents,
      frame: codingSceneInputFrame,
      movement: codingSceneInputMovement,
      diagnosticsSample: codingSceneInputDiagnostics.slice(-20),
    }
    const codingOverlay: VerseCodingOverlaySmoke = {
      ok:
        codingOverlayProbe.ok &&
        focusedTyping.ok &&
        codingOverlayPixels.ok &&
        codingSceneInput.ok,
      blockerRefs: [
        ...codingOverlayProbe.blockerRefs,
        ...(focusedTyping.ok ? [] : ["verse.coding.focusedTyping"]),
        ...(codingOverlayPixels.ok ? [] : ["verse.coding.screenshotPixels"]),
        ...(codingSceneInput.ok ? [] : ["verse.coding.sceneInput"]),
      ],
      probe: codingOverlayProbe,
      focusedTyping,
      screenshot: {
        ...codingOverlayPixels,
        path: relativePath(codingOverlayScreenshotPath),
      },
      sceneInput: codingSceneInput,
    }
    const webglFrameTimes = await readVerseFrameTimes(cdp)
    const webglDiagnostics = projectVerseWebglDiagnostics({
      mode: target === "dev" ? "development" : "smoke",
      enabled: true,
      frameTimesMs: webglFrameTimes,
      drawCalls: 0,
      entityCount: dom.canvas.width > 0 && dom.canvas.height > 0 ? 1 : 0,
      sourceRefs: [
        "script:apps/autopilot-desktop/scripts/verse-launch-smoke.ts",
        "github:OpenAgentsInc/openagents#5978",
      ],
    })

    const ok =
      dom.ok &&
      pixels.ok &&
      movement.ok &&
      continuousMovement.ok &&
      mouseLookDrag.ok &&
      retainedLiveUpdate.ok &&
      customKeybindings.ok &&
      codingOverlay.ok
    const receipt: VerseLaunchReceipt = {
      ok,
      message: ok
        ? "Desktop Verse launch and coding overlay checks passed"
        : "Desktop Verse launch or coding overlay failed smoke checks",
      generatedAt: new Date().toISOString(),
      target,
      appBundle: appBundle === null ? null : relativePath(appBundle),
      appUrl: smokeUrl,
      sourceRefs: [
        "github:OpenAgentsInc/openagents#5827",
        "github:OpenAgentsInc/openagents#5910",
        "github:OpenAgentsInc/openagents#5913",
        "github:OpenAgentsInc/openagents#5915",
        "github:OpenAgentsInc/openagents#5914",
        "github:OpenAgentsInc/openagents#5912",
        "github:OpenAgentsInc/openagents#5917",
        "github:OpenAgentsInc/openagents#5916",
        "github:OpenAgentsInc/openagents#5932",
        "github:OpenAgentsInc/openagents#5950",
        "script:apps/autopilot-desktop/scripts/verse-launch-smoke.ts",
      ],
      packagedFiles:
        htmlPath === null ||
        cssPath === null ||
        mainPath === null ||
        pylonNodePath === null
          ? null
          : {
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
      mouseLookDrag,
      retainedLiveUpdate,
      customKeybindings,
      codingOverlay,
      webglDiagnostics,
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
          mouseLookDrag.ok
            ? ""
            : "verse.launch.mouseLookDragWheelCameraControlNoTextCursorSelectionBlankFrameOrRemount",
          customKeybindings.ok
            ? ""
            : `verse.launch.customKeybindings(${customKeybindings.blockerRefs.join(",")})`,
          codingOverlay.ok
            ? ""
            : `verse.launch.codingOverlay(${codingOverlay.blockerRefs.join(",")})`,
        ].filter(Boolean).join("; "),
      )
    }
  } finally {
    cdp?.close()
    if (chrome !== null) {
      chrome.kill()
      await Promise.race([chrome.exited.catch(() => null), wait(2_000)])
    }
    server?.stop(true)
    rmSync(tmpRoot, { force: true, recursive: true })
  }
}

main().then(
  () => process.exit(0),
  error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  },
)
