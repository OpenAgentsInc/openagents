// Headless app replica harness — the standing way to drive + assert the REAL
// Autopilot desktop renderer without the Electrobun GUI.
//
// WHAT IT BOOTS: the SAME `Model` / `view` / `update` / `subscriptions` + the
// real Foldkit `Runtime.run` mount the live app uses (see
// `scripts/app-replica-entry.ts`), in headless Chromium (WebGL under
// SwiftShader, so the three-effect Verse scene mounts with no GPU). Not a
// reducer mock, not a synthetic DOM — the real renderer.
//
// THE TWO HISTORICAL BLOCKERS, SOLVED:
//
//   1. Component styles. The old compile-plugin styling path was removed; the
//      desktop now serves the SAME generated `styles.out.css` as the packaged
//      app, including the central `--oa-*` token block and shared component
//      classes. The real, styled view mounts headlessly with no runtime shim and
//      no compile-time style plugin.
//
//   2. The Electrobun bridge. `window.bun` / the RPC transport `getRequest()`
//      (which `khalaTurn`, `shellTurn`, token resolution, etc. call) is absent in
//      a plain browser. The entry installs a TEST-CONTROLLED stub through the
//      SAME `setRequest` seam the live `main.ts` uses, and drives the live
//      `khalaToken` push through the SAME `pushInbound`. Combined with the #6045
//      deterministic-env layers (TestClock + seeded RNG + stub Transport) for any
//      service-level timing/seed, the whole non-deterministic surface is injected.
//
// DETERMINISM: the page installs a driver-controlled fake `requestAnimationFrame`
// + `performance.now` (same trick as headless-pixel.ts) so frames advance ONLY
// when the driver calls `stepFrames(n)` — no rAF, no wall-clock. Running the same
// scenario twice produces identical DOM/boxes/text.
//
// THE DRIVER drives the REAL DOM event path via CDP: `pressKey` dispatches a real
// `keydown` (so it flows keyboard subscription → forward gate → interpretKey →
// reducer → re-render), `click` dispatches a real mouse click at the element's
// box (so a Foldkit `OnClick` fires through the real delegated handler). Nothing
// here calls the reducer directly.

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { join, resolve } from "node:path"
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

export const resolveChromePathOrNull = (): string | null =>
  CHROME_CANDIDATES.find((candidate) => existsSync(candidate)) ?? null

const wait = (ms: number): Promise<void> =>
  new Promise((resolveWait) => setTimeout(resolveWait, ms))

const cdpPageTargetTimeoutMs = 30_000

const getFreePort = (): Promise<number> =>
  new Promise((resolvePort, reject) => {
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
        else resolvePort(port)
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
  new Promise((resolveCdp, reject) => {
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
      resolveCdp({
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
  const deadline = Date.now() + cdpPageTargetTimeoutMs
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

// ── Public types ──────────────────────────────────────────────────────────────

export type KeyModifiers = Readonly<{
  meta?: boolean
  ctrl?: boolean
  shift?: boolean
  alt?: boolean
}>

export type BoundingBox = Readonly<{
  x: number
  y: number
  width: number
  height: number
  top: number
  left: number
  right: number
  bottom: number
}>

// A canned Khala turn the replica replays through the stubbed bridge: streamed
// deltas (via the live `khalaToken` push) + a terminal RPC answer.
export type KhalaScript = Readonly<{
  deltas: ReadonlyArray<string>
  text: string
  ok?: boolean
  live?: boolean
  receipt?: unknown
  // Resolve the RPC BEFORE streaming the deltas — reproduces the live race that
  // doubles the answer (terminal lands first, late deltas append on top).
  resolveBeforeStream?: boolean
}>

export type AppReplica = Readonly<{
  // ── Driver API ──────────────────────────────────────────────────────────────
  // Dispatch a real DOM keydown at the focused element (or document). Flows
  // through the real keyboard subscription → forward gate → reducer → re-render.
  pressKey: (key: string, mods?: KeyModifiers) => Promise<void>
  // Focus a field and type text char-by-char via real keydown/input.
  type: (selector: string, text: string) => Promise<void>
  // Real mouse click at the element's centre (a real Foldkit OnClick fires).
  click: (selector: string) => Promise<void>
  // Hold a key DOWN, pump `frames` deterministic frames (so a held movement key
  // integrates in the third-person controller), then release it. The keydown/up
  // hit `window` (blur any field first) like a real held keystroke.
  holdKey: (key: string, frames: number, mods?: KeyModifiers) => Promise<void>
  // Focus an element (so a subsequent pressKey is "in editable" if it is a field).
  focus: (selector: string) => Promise<void>
  // The visible text content of the first matching element ("" if absent).
  text: (selector: string) => Promise<string>
  // Count of matching elements.
  count: (selector: string) => Promise<number>
  // The element's bounding box (null if absent / not laid out).
  boundingBox: (selector: string) => Promise<BoundingBox | null>
  // Advance EXACTLY n deterministic frames (no rAF / wall-clock).
  stepFrames: (n: number) => Promise<void>
  // PNG screenshot (base64), optionally written to a path.
  screenshot: (path?: string) => Promise<string>
  // Script the next Khala turn (deltas + terminal text + ordering).
  scriptKhala: (script: KhalaScript) => Promise<void>
  // Wait until the runtime has flushed pending work + rendered (settles streaming
  // microtasks). Deterministic: polls a settle predicate, not a fixed sleep.
  settle: () => Promise<void>
  // Evaluate an arbitrary expression in the page (escape hatch for assertions).
  evaluate: <T>(expression: string) => Promise<T>
  // Tear down Chrome + the static server.
  close: () => Promise<void>
}>

export type LaunchOptions = Readonly<{
  width?: number
  height?: number
  // Override the entry module (defaults to scripts/app-replica-entry.ts).
  entryModulePath?: string
}>

const APP_ROOT = resolve(import.meta.dir, "..", "..")
const DEFAULT_ENTRY = join(APP_ROOT, "scripts", "app-replica-entry.ts")
const STYLES_OUT = join(APP_ROOT, "src", "ui", "styles.out.css")

// Installed BEFORE the entry module imports, so three-effect's "always" frame
// clock captures the fake rAF + clock and the runtime never animates on its own.
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
    <title>Autopilot desktop replica</title>
    <link rel="stylesheet" href="/styles.out.css" />
    <style>
      :root { color-scheme: dark; }
      html, body { margin: 0; width: ${width}px; height: ${height}px; background: #0b0d12; color: #e6e9ef; overflow: hidden; }
      #root { width: ${width}px; height: ${height}px; }
    </style>
    <script>${pageBootstrap()}</script>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/dist/app-replica-entry.js"></script>
  </body>
</html>
`

// US keyboard "key" → CDP key fields (best-effort for the keys the harness uses:
// digits, letters, Enter, Escape, arrows). Unknown keys still dispatch with the
// raw key text so the forward gate / interpretKey see the right `key`.
const keyToCode = (key: string): { code: string; keyCode: number } => {
  if (/^[0-9]$/.test(key)) {
    return { code: `Digit${key}`, keyCode: key.charCodeAt(0) }
  }
  if (/^[a-z]$/i.test(key)) {
    return { code: `Key${key.toUpperCase()}`, keyCode: key.toUpperCase().charCodeAt(0) }
  }
  switch (key) {
    case "Enter":
      return { code: "Enter", keyCode: 13 }
    case "Escape":
      return { code: "Escape", keyCode: 27 }
    case "ArrowUp":
      return { code: "ArrowUp", keyCode: 38 }
    case "ArrowDown":
      return { code: "ArrowDown", keyCode: 40 }
    case " ":
      return { code: "Space", keyCode: 32 }
    default:
      return { code: "", keyCode: 0 }
  }
}

const cdpModifiers = (mods: KeyModifiers): number =>
  (mods.alt ? 1 : 0) |
  (mods.ctrl ? 2 : 0) |
  (mods.meta ? 4 : 0) |
  (mods.shift ? 8 : 0)

export const launchAppReplica = async (
  options: LaunchOptions = {},
): Promise<AppReplica> => {
  const chromePath = resolveChromePathOrNull()
  if (chromePath === null) {
    throw new Error(
      "Chrome/Chromium not found. Set CHROME_PATH to a Chromium-family binary.",
    )
  }
  if (!existsSync(STYLES_OUT)) {
    throw new Error(
      `Replica needs the compiled stylesheet at ${STYLES_OUT}. Run 'bun run build:css' first.`,
    )
  }
  const width = options.width ?? 1280
  const height = options.height ?? 800
  const entryModulePath = options.entryModulePath ?? DEFAULT_ENTRY

  const tmpRoot = mkdtempSync(join(APP_ROOT, ".app-replica-"))
  const userDataDir = join(tmpRoot, "chrome-profile")
  const distDir = join(tmpRoot, "dist")
  const htmlPath = join(tmpRoot, "index.html")
  mkdirSync(distDir, { recursive: true })
  mkdirSync(userDataDir, { recursive: true })
  writeFileSync(htmlPath, pageHtml(width, height))

  let server: ReturnType<typeof Bun.serve> | null = null
  let chrome: Bun.Subprocess | null = null
  let cdp: CdpClient | null = null

  const cleanup = async (): Promise<void> => {
    cdp?.close()
    if (chrome !== null) {
      chrome.kill()
      await chrome.exited.catch(() => null)
    }
    server?.stop(true)
    rmSync(tmpRoot, { force: true, recursive: true })
  }

  try {
    // The old compile-plugin style layer was removed from the app (the typed-token system in
    // `@openagentsinc/design-tokens` replaced it, #6046/#6050), so the entry no
    // longer needs a compile plugin — a plain browser bundle mounts view.ts. CSS
    // comes from the served styles.out.css.
    const build = await Bun.build({
      entrypoints: [entryModulePath],
      outdir: distDir,
      target: "browser",
      format: "esm",
    })
    if (!build.success) {
      const logs = build.logs.map((log) => String(log)).join("\n")
      throw new Error(`failed to build app-replica entry bundle:\n${logs}`)
    }

    const stylesCss = readFileSync(STYLES_OUT)
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
        if (url.pathname === "/dist/app-replica-entry.js") {
          return new Response(readFileSync(join(distDir, "app-replica-entry.js")), {
            headers: { "content-type": "text/javascript; charset=utf-8" },
          })
        }
        if (url.pathname === "/styles.out.css") {
          return new Response(stylesCss, {
            headers: { "content-type": "text/css; charset=utf-8" },
          })
        }
        return new Response("not found", { status: 404 })
      },
    })

    const debugPort = await getFreePort()
    const url = `http://127.0.0.1:${server.port}/`
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
    const client = cdp
    await client.send("Runtime.enable")
    await client.send("Page.enable")
    await client.send("DOM.enable")
    await client.send("Page.navigate", { url })

    const evaluate = async <T>(expression: string): Promise<T> => {
      const result = await client.send<{
        readonly result?: { readonly value?: T }
        readonly exceptionDetails?: { readonly exception?: { description?: string } }
      }>("Runtime.evaluate", {
        expression,
        awaitPromise: true,
        returnByValue: true,
      })
      if (result.exceptionDetails !== undefined) {
        throw new Error(
          `page eval threw: ${result.exceptionDetails.exception?.description ?? JSON.stringify(result.exceptionDetails)}`,
        )
      }
      return result.result?.value as T
    }

    // Wait for the replica to signal ready (real Foldkit program mounted), and
    // hard-fail if the view crashed (style layer/bridge unsolved).
    const mounted = await evaluate<{ ready: boolean; crash: string | null }>(`
      (async () => {
        for (let i = 0; i < 300; i += 1) {
          const crash = (window).__OA_REPLICA_CRASH__ ?? null
          if (crash) return { ready: false, crash }
          if ((window).__OA_REPLICA__ && (window).__OA_REPLICA__.ready) {
            return { ready: true, crash: null }
          }
          await new Promise((r) => setTimeout(r, 20))
        }
        return { ready: false, crash: (window).__OA_REPLICA_CRASH__ ?? "timeout" }
      })()
    `)
    if (!mounted.ready) {
      throw new Error(
        `app replica did not mount the real view: ${mounted.crash ?? "unknown"}`,
      )
    }

    // ── Driver primitives ────────────────────────────────────────────────────

    // Flush n render frames. Foldkit batches its DOM patch through
    // `requestAnimationFrame` (runtime.js); the page's fake rAF means the runtime
    // NEVER repaints on its own — a render happens ONLY when we pump frames here.
    // That is the determinism guarantee: the same scenario pumps the same number
    // of frames and produces identical DOM every run.
    const pumpFrames = async (n: number): Promise<void> => {
      await evaluate(`__stepFrames(${n}, 16)`)
    }

    const settle = async (): Promise<void> => {
      // One driver action can enqueue async Effects (RPC promises, streamed token
      // pushes) that each schedule a render frame. We interleave macrotask drains
      // (so those promises resolve + queue their renders) with frame pumps (so the
      // queued renders flush to the DOM). A fixed, bounded number of rounds keeps
      // it deterministic: nothing advances on its own under the fake clock.
      await evaluate(`
        (async () => {
          for (let i = 0; i < 12; i += 1) {
            await new Promise((r) => setTimeout(r, 0))
            __stepFrames(1, 16)
          }
          return true
        })()
      `)
    }

    const boundingBox = async (selector: string): Promise<BoundingBox | null> =>
      evaluate<BoundingBox | null>(`
        (() => {
          const el = document.querySelector(${JSON.stringify(selector)})
          if (!el) return null
          const r = el.getBoundingClientRect()
          if (r.width === 0 && r.height === 0) {
            return { x: r.x, y: r.y, width: 0, height: 0, top: r.top, left: r.left, right: r.right, bottom: r.bottom }
          }
          return { x: r.x, y: r.y, width: r.width, height: r.height, top: r.top, left: r.left, right: r.right, bottom: r.bottom }
        })()
      `)

    const focus = async (selector: string): Promise<void> => {
      await evaluate(`
        (() => {
          const el = document.querySelector(${JSON.stringify(selector)})
          if (el && typeof el.focus === "function") el.focus()
          return true
        })()
      `)
    }

    const dispatchKey = async (
      key: string,
      mods: KeyModifiers,
    ): Promise<void> => {
      const { code, keyCode } = keyToCode(key)
      const modifiers = cdpModifiers(mods)
      const isText = key.length === 1 && modifiers === 0
      // Real keydown via CDP — flows to the window keydown listener the keyboard
      // subscription installed (the live forward gate path). For a printable char
      // a `keyDown` carrying `text` is enough for Chromium to both fire the JS
      // `keydown` event AND insert the character into a focused field (a separate
      // `char` event would double-insert), matching a real keystroke.
      await client.send("Input.dispatchKeyEvent", {
        type: isText ? "keyDown" : "rawKeyDown",
        key,
        ...(code === "" ? {} : { code }),
        ...(keyCode === 0 ? {} : { windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode }),
        modifiers,
        ...(isText ? { text: key } : {}),
      })
      await client.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key,
        ...(code === "" ? {} : { code }),
        ...(keyCode === 0 ? {} : { windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode }),
        modifiers,
      })
      await settle()
    }

    const pressKey = async (key: string, mods: KeyModifiers = {}): Promise<void> =>
      dispatchKey(key, mods)

    // Hold a key down across `frames` pumped frames, then release — so a movement
    // key actually integrates in the third-person controller's `update(delta)`
    // (a back-to-back keydown/keyup moves nothing). keydown/keyup target `window`
    // (the controller listens there), so we do NOT route through a focused field.
    const holdKey = async (
      key: string,
      frames: number,
      mods: KeyModifiers = {},
    ): Promise<void> => {
      const { code, keyCode } = keyToCode(key)
      const modifiers = cdpModifiers(mods)
      await client.send("Input.dispatchKeyEvent", {
        type: "rawKeyDown",
        key,
        ...(code === "" ? {} : { code }),
        ...(keyCode === 0
          ? {}
          : { windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode }),
        modifiers,
      })
      // Pump frames WHILE the key is held so the controller integrates motion.
      await pumpFrames(frames)
      await client.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key,
        ...(code === "" ? {} : { code }),
        ...(keyCode === 0
          ? {}
          : { windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode }),
        modifiers,
      })
      await pumpFrames(2)
    }

    const typeInto = async (selector: string, text: string): Promise<void> => {
      await focus(selector)
      for (const ch of text) {
        await dispatchKey(ch, {})
      }
    }

    const click = async (selector: string): Promise<void> => {
      const box = await boundingBox(selector)
      if (box === null || (box.width === 0 && box.height === 0)) {
        throw new Error(`click target not found / not laid out: ${selector}`)
      }
      const x = box.left + box.width / 2
      const y = box.top + box.height / 2
      await client.send("Input.dispatchMouseEvent", {
        type: "mousePressed",
        x,
        y,
        button: "left",
        clickCount: 1,
      })
      await client.send("Input.dispatchMouseEvent", {
        type: "mouseReleased",
        x,
        y,
        button: "left",
        clickCount: 1,
      })
      await settle()
    }

    const text = async (selector: string): Promise<string> =>
      evaluate<string>(`
        (() => {
          const el = document.querySelector(${JSON.stringify(selector)})
          return el ? (el.textContent ?? "") : ""
        })()
      `)

    const count = async (selector: string): Promise<number> =>
      evaluate<number>(
        `document.querySelectorAll(${JSON.stringify(selector)}).length`,
      )

    const stepFrames = async (n: number): Promise<void> => {
      await pumpFrames(n)
    }

    const screenshot = async (path?: string): Promise<string> => {
      const shot = await client.send<{ readonly data: string }>(
        "Page.captureScreenshot",
        { format: "png", fromSurface: true },
      )
      if (path !== undefined) {
        writeFileSync(path, Buffer.from(shot.data, "base64"))
      }
      return shot.data
    }

    const scriptKhala = async (script: KhalaScript): Promise<void> => {
      await evaluate(
        `(window.__OA_REPLICA__.scriptKhala(${JSON.stringify(script)}), true)`,
      )
    }

    return {
      pressKey,
      type: typeInto,
      click,
      holdKey,
      focus,
      text,
      count,
      boundingBox,
      stepFrames,
      screenshot,
      scriptKhala,
      settle,
      evaluate,
      close: cleanup,
    }
  } catch (error) {
    await cleanup()
    throw error
  }
}
