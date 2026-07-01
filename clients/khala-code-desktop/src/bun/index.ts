import { existsSync } from "node:fs"
import { resolve } from "node:path"

import {
  KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT,
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  khalaCodeDesktopRpcDecodeFailure,
  khalaCodeDesktopRpcHandlerFailure,
  khalaCodeDesktopRpcMethodSchema,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopRpcMethodName,
  type KhalaCodeDesktopRPCSchema,
} from "../shared/rpc.js"
import { buildKhalaAppleFmDisabledReadiness } from "../shared/apple-fm-readiness.js"
import { khalaCodeDesktopApplicationMenu } from "./application-menu.js"
import {
  readKhalaCodeHeadlessPrompt,
  runKhalaCodeDesktopHeadlessJsonl,
} from "./headless.js"
import { createCodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import { createCodexAppServerHost } from "./codex-app-server-client.js"
import {
  createKhalaCodeDesktopCodexMessageTokenAuditRecorder,
  createKhalaCodeDesktopCodexTokenUsageReporter,
  startKhalaCodeDesktopTokenUsageBackgroundSync,
} from "./codex-token-usage-telemetry.js"
import { createOnDeviceDeciderHost } from "./on-device-decider-host.js"
import { createKhalaCodeDesktopRpcRequestHandlers } from "./rpc-handlers.js"

const previewPort = (): number => {
  const parsed = Number(
    Bun.env.KHALA_CODE_DESKTOP_PREVIEW_PORT ??
      String(KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT),
  )
  return Number.isInteger(parsed) && parsed > 0
    ? parsed
    : KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT
}

const contentTypeFor = (path: string): string => {
  if (path.endsWith(".css")) return "text/css; charset=utf-8"
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8"
  if (path.endsWith(".woff2")) return "font/woff2"
  return "text/html; charset=utf-8"
}

const packagedViewPath = (...segments: readonly string[]): string =>
  resolve(process.cwd(), "../Resources/app/views/khala-code-desktop", ...segments)

const envPath = (
  env: Readonly<Record<string, string | undefined>>,
  key: string,
): string | undefined => {
  const value = env[key]?.trim()
  return value === undefined || value.length === 0 ? undefined : value
}

const isMacAppBundleCwd = (path: string): boolean =>
  /\/[^/]+\.app\/Contents\/MacOS\/?$/.test(path)

// Launched via `khala`, INIT_CWD/PWD point at the desktop package
// (clients/khala-code-desktop) — the app's own source, never a useful workspace
// for the owner's tools. Treat it like the app bundle and fall through to the
// real workspace (KHALA_CODE_DESKTOP_WORKSPACE, exported by the launcher, or
// ~/work).
const isDesktopPackageCwd = (path: string): boolean =>
  /\/clients\/khala-code-desktop\/?$/.test(path)

const isNonWorkspaceCwd = (path: string): boolean =>
  isMacAppBundleCwd(path) || isDesktopPackageCwd(path)

const resolveToolWorkingDirectory = (
  env: Readonly<Record<string, string | undefined>>,
): string => {
  const explicit = envPath(env, "KHALA_CODE_DESKTOP_WORKSPACE")
  if (explicit !== undefined) return explicit

  for (const key of ["INIT_CWD", "PWD"]) {
    const candidate = envPath(env, key)
    if (candidate !== undefined && !isNonWorkspaceCwd(candidate)) return candidate
  }

  const cwd = process.cwd()
  if (!isNonWorkspaceCwd(cwd)) return cwd

  const home = envPath(env, "HOME")
  if (home !== undefined) {
    const workspace = resolve(home, "work")
    return existsSync(workspace) ? workspace : home
  }
  return cwd
}

const resolveSourceRepositoryRoot = (): string =>
  resolve(import.meta.dir, "../../../..")

// The webview is a Vite build under ./dist (index.html + assets/*, with fonts
// and scene assets self-contained). The browser preview server mirrors that
// layout, falling back to the packaged view directory for release builds.
const previewAssetPaths = (pathname: string): readonly string[] => {
  const clean = pathname === "/" ? "/index.html" : pathname
  if (clean === "/index.html") {
    return [
      resolve(process.cwd(), "dist/index.html"),
      packagedViewPath("index.html"),
    ]
  }
  const asset = clean.replace(/^\/+/, "")
  return [
    resolve(process.cwd(), "dist", asset),
    packagedViewPath(asset),
  ]
}

const jsonResponse = (payload: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(payload), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  })

let emitChatTurnEvent = (_event: KhalaCodeDesktopChatTurnEvent): void => {}
let rpcRequestHandlers: KhalaCodeDesktopRPCSchema["requests"]

const previewRpcResponse = async (
  request: Request,
  method: string,
): Promise<Response> => {
  if (request.method !== "POST") {
    return jsonResponse({
      ok: false,
      error: "method_not_allowed",
      method,
      tag: "rpc_method_not_allowed",
    }, { status: 405 })
  }
  const methodSchema = khalaCodeDesktopRpcMethodSchema(method)
  if (methodSchema === null) {
    return jsonResponse({
      ok: false,
      error: "unknown_method",
      method,
      tag: "rpc_unknown_method",
    }, { status: 404 })
  }
  const rpcMethod = method as KhalaCodeDesktopRpcMethodName
  const handler = rpcRequestHandlers[rpcMethod as keyof typeof rpcRequestHandlers]
  if (handler === undefined) {
    return jsonResponse({
      ok: false,
      error: "unknown_method",
      method,
      tag: "rpc_unknown_method",
    }, { status: 404 })
  }

  let rawArgs: unknown = []
  try {
    const body = await request.json() as { args?: unknown }
    rawArgs = body.args ?? []
  } catch (error) {
    return jsonResponse(khalaCodeDesktopRpcDecodeFailure(method, error), { status: 400 })
  }

  let args: readonly unknown[]
  try {
    args = decodeKhalaCodeDesktopRpcParameters(rpcMethod, rawArgs)
  } catch (error) {
    return jsonResponse(khalaCodeDesktopRpcDecodeFailure(method, error), { status: 400 })
  }

  const invoke = handler as (...args: readonly unknown[]) => Promise<unknown>
  try {
    const result = await invoke(...args)
    try {
      return jsonResponse(decodeKhalaCodeDesktopRpcResult(rpcMethod, result))
    } catch (error) {
      return jsonResponse(khalaCodeDesktopRpcDecodeFailure(method, error), { status: 500 })
    }
  } catch (error) {
    return jsonResponse(khalaCodeDesktopRpcHandlerFailure(method, error), { status: 500 })
  }
}

const previewAssetResponse = async (pathname: string): Promise<Response> => {
  for (const path of previewAssetPaths(pathname)) {
    if (!(await Bun.file(path).exists())) continue
    return new Response(Bun.file(path), {
      headers: { "content-type": contentTypeFor(path) },
    })
  }
  return new Response("not found", { status: 404 })
}

const previewFetch = async (request: Request): Promise<Response> => {
  const url = new URL(request.url)
  if (url.pathname === "/health") {
    return jsonResponse({
      ok: true,
      app: "Khala Code Desktop",
      observedAt: new Date().toISOString(),
    })
  }
  if (url.pathname.startsWith("/rpc/")) {
    return previewRpcResponse(request, decodeURIComponent(url.pathname.slice(5)))
  }
  return previewAssetResponse(url.pathname)
}

const startPreviewServer = (): void => {
  if (Bun.env.KHALA_CODE_DESKTOP_PREVIEW_SERVER === "0") return
  const requestedPort = previewPort()
  for (let offset = 0; offset < 10; offset += 1) {
    const port = requestedPort + offset
    try {
      const server = Bun.serve({
        port,
        fetch: previewFetch,
      })
      console.info(`Khala Code desktop web preview: http://localhost:${server.port}`)
      return
    } catch (error) {
      if (!String(error).includes("EADDRINUSE") || offset === 9) {
        console.warn(
          `Khala Code desktop web preview unavailable: ${
            error instanceof Error ? error.message : String(error)
          }`,
        )
      }
    }
  }
}

const headlessInterruptAfterMs = (
  env: Readonly<Record<string, string | undefined>>,
): number | undefined => {
  const raw = env.KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS?.trim()
  if (raw === undefined || raw.length === 0) return undefined
  const parsed = Number.parseInt(raw, 10)
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined
}

const argv = Bun.argv.slice(2)
if (argv.includes("--json")) {
  const promptArgv = argv[0] === "code" ? argv.slice(1) : argv
  const prompt = await readKhalaCodeHeadlessPrompt(promptArgv)
  if (prompt.length === 0) {
    process.stderr.write("khala code --json requires a prompt argument or stdin.\n")
    process.exit(2)
  }
  const workingDirectory = resolveToolWorkingDirectory(Bun.env)
  const headlessCodexAppServerHost = createCodexAppServerHost({ env: Bun.env })
  const interruptAfterMs = headlessInterruptAfterMs(Bun.env)
  let exitCode = 0
  try {
    await runKhalaCodeDesktopHeadlessJsonl({
      createCodexChatRuntime: ({ onEvent }) =>
        createCodexAppServerChatRuntime({
          env: Bun.env,
          host: headlessCodexAppServerHost,
          onEvent,
          messageTokenAuditRecorder:
            createKhalaCodeDesktopCodexMessageTokenAuditRecorder({ env: Bun.env }),
          tokenUsageReporter: createKhalaCodeDesktopCodexTokenUsageReporter({ env: Bun.env }),
          workingDirectory,
        }),
      env: Bun.env,
      ...(interruptAfterMs === undefined ? {} : { interruptAfterMs }),
      prompt,
      workingDirectory,
    })
  } catch {
    exitCode = 1
  } finally {
    headlessCodexAppServerHost.dispose()
  }
  process.exit(exitCode)
}

const { ApplicationMenu, BrowserView, BrowserWindow, Screen } = await import("electrobun/bun")

type KhalaCodeDesktopWindowFrame = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

const FALLBACK_MAIN_WINDOW_FRAME: KhalaCodeDesktopWindowFrame = {
  x: 96,
  y: 56,
  width: 1180,
  height: 820,
}

const isUsableWindowFrame = (
  frame: Readonly<Partial<KhalaCodeDesktopWindowFrame>>,
): frame is KhalaCodeDesktopWindowFrame =>
  Number.isFinite(frame.x) &&
  Number.isFinite(frame.y) &&
  Number.isFinite(frame.width) &&
  Number.isFinite(frame.height) &&
  Number(frame.width) > 0 &&
  Number(frame.height) > 0

const resolveMainWindowFrame = (): KhalaCodeDesktopWindowFrame => {
  const workArea = Screen.getPrimaryDisplay().workArea
  if (!isUsableWindowFrame(workArea)) return FALLBACK_MAIN_WINDOW_FRAME
  return {
    x: Math.round(workArea.x),
    y: Math.round(workArea.y),
    width: Math.round(workArea.width),
    height: Math.round(workArea.height),
  }
}

// Apple FM bridge code remains in the repo, but launch builds intentionally do
// not prepare, bundle, start, or probe it.
const appleFmReadiness = () =>
  buildKhalaAppleFmDisabledReadiness({
    platform: { platform: process.platform, arch: process.arch },
  })
const codexAppServerHost = createCodexAppServerHost({ env: Bun.env })
const tokenUsageBackgroundSync = startKhalaCodeDesktopTokenUsageBackgroundSync({
  env: Bun.env,
  onError: error => {
    console.warn(
      `Khala Code token usage sync failed: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
  },
})

// Optional on-device decider: a small local model that selects a
// platform-appropriate backend. Apple FM is omitted for launch unless a future
// host deliberately opts it back in.
const onDeviceDecider = createOnDeviceDeciderHost({ env: Bun.env })

rpcRequestHandlers = createKhalaCodeDesktopRpcRequestHandlers({
  appleFmReadiness,
  codexAppServerHost,
  enableFleetMcpBridge: true,
  emitChatTurnEvent: event => emitChatTurnEvent(event),
  env: Bun.env,
  fleetMcpBridgeRepoRoot: resolveSourceRepositoryRoot(),
  onDeviceDeciderStatus: () => onDeviceDecider.select(),
  workingDirectory: resolveToolWorkingDirectory(Bun.env),
})

const disposeRuntime = (): void => {
  tokenUsageBackgroundSync.dispose()
  codexAppServerHost.dispose()
}

process.once("exit", disposeRuntime)
process.once("SIGINT", () => {
  disposeRuntime()
  process.exit(130)
})
process.once("SIGTERM", () => {
  disposeRuntime()
  process.exit(143)
})

const rpc = BrowserView.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: rpcRequestHandlers,
    messages: {},
  },
})

emitChatTurnEvent = event => rpc.send.chatTurnEvent(event)

startPreviewServer()

const resolveMainViewUrl = async (): Promise<string> => {
  // HMR: when the Vite dev server (dev:hmr) is up, load the webview from it
  // for live reload; otherwise fall back to the bundled views.
  try {
    const res = await fetch("http://localhost:5173", { signal: AbortSignal.timeout(400) })
    if (res.ok) return "http://localhost:5173/"
  } catch {
    // Vite dev server not running; use bundled views.
  }
  return "views://khala-code-desktop/index.html"
}

if (Bun.env.KHALA_CODE_DESKTOP_OPEN_WINDOW !== "0") {
  ApplicationMenu.setApplicationMenu(khalaCodeDesktopApplicationMenu)

  new BrowserWindow({
    title: "Khala Code",
    url: await resolveMainViewUrl(),
    frame: resolveMainWindowFrame(),
    titleBarStyle: "hiddenInset",
    rpc,
  })
}
