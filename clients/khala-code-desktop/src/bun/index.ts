import { ApplicationMenu, BrowserView, BrowserWindow } from "electrobun/bun"
import { resolve } from "node:path"

import {
  KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT,
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaCodeDesktopRPCSchema,
} from "../shared/rpc.js"
import { khalaCodeDesktopApplicationMenu } from "./application-menu.js"
import { createAppleFmSidecarHost } from "./apple-fm-sidecar.js"
import { createOnDeviceDeciderHost } from "./on-device-decider-host.js"
import { createKhalaCodeDesktopRpcRequestHandlers } from "./rpc-handlers.js"

// Optional on-device Apple Foundation Models sidecar (Mac/Apple-Silicon only).
// Off by default and fails soft: readiness reports unavailability rather than
// throwing when the FM bridge or hardware is missing.
const appleFmSidecar = createAppleFmSidecarHost()

// Optional on-device decider: a small local model that selects a
// platform-appropriate backend (Apple FM on Apple Silicon, self-hosted GPT-OSS
// elsewhere). Reuses the sidecar above so a single helper is supervised.
const onDeviceDecider = createOnDeviceDeciderHost({ sidecar: appleFmSidecar })

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

const previewAssetPaths = (pathname: string): readonly string[] => {
  const clean = pathname === "/" ? "/index.html" : pathname
  if (clean === "/index.html") {
    return [
      resolve(process.cwd(), "src/ui/index.html"),
      packagedViewPath("index.html"),
    ]
  }
  if (clean === "/main.js") {
    return [
      resolve(process.cwd(), "resources/ui/main.js"),
      packagedViewPath("main.js"),
    ]
  }
  if (clean === "/main.css") {
    return [
      resolve(process.cwd(), "resources/ui/main.css"),
      packagedViewPath("main.css"),
    ]
  }
  if (clean.startsWith("/fonts/")) {
    return [
      resolve(process.cwd(), "src/ui", clean.slice(1)),
      packagedViewPath(clean.slice(1)),
    ]
  }
  const asset = clean.replace(/^\/+/, "")
  return [
    resolve(process.cwd(), "resources/ui", asset),
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

const rpcRequestHandlers: KhalaCodeDesktopRPCSchema["requests"] =
  createKhalaCodeDesktopRpcRequestHandlers({
    appleFmReadiness: () => appleFmSidecar.readiness(),
    env: Bun.env,
    onDeviceDeciderStatus: () => onDeviceDecider.select(),
    workingDirectory: process.cwd(),
  })

const previewRpcResponse = async (
  request: Request,
  method: string,
): Promise<Response> => {
  if (request.method !== "POST") {
    return jsonResponse({ ok: false, error: "method_not_allowed" }, { status: 405 })
  }
  const handler = rpcRequestHandlers[method as keyof typeof rpcRequestHandlers]
  if (handler === undefined) {
    return jsonResponse({ ok: false, error: "unknown_method" }, { status: 404 })
  }

  let args: unknown[] = []
  try {
    const body = await request.json() as { args?: unknown }
    args = Array.isArray(body.args) ? body.args : []
  } catch {
    args = []
  }

  const invoke = handler as (...args: readonly unknown[]) => Promise<unknown>
  return jsonResponse(await invoke(...args))
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

const rpc = BrowserView.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: rpcRequestHandlers,
    messages: {},
  },
})

startPreviewServer()

if (Bun.env.KHALA_CODE_DESKTOP_OPEN_WINDOW !== "0") {
  ApplicationMenu.setApplicationMenu(khalaCodeDesktopApplicationMenu)

  new BrowserWindow({
    title: "Khala Code",
    url: "views://khala-code-desktop/index.html",
    frame: { x: 152, y: 96, width: 980, height: 740 },
    rpc,
  })
}
