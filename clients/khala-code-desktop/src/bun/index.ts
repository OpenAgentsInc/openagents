import { existsSync } from "node:fs"
import { createHash, randomBytes, timingSafeEqual } from "node:crypto"
import * as os from "node:os"
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
  type KhalaCodeDesktopClaudeApprovalRequestProjection,
  type KhalaCodeDesktopFleetLifecycleEvent,
  type KhalaCodeDesktopQaMetricSample,
  type KhalaCodeDesktopQaMetricsSnapshot,
  type KhalaCodeDesktopRpcMethodName,
  type KhalaCodeDesktopRPCSchema,
} from "../shared/rpc.js"
import {
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
} from "../shared/qa-metrics.js"
import { buildKhalaAppleFmDisabledReadiness } from "../shared/apple-fm-readiness.js"
import { khalaCodeDesktopApplicationMenu } from "./application-menu.js"
import {
  parseKhalaCodeHeadlessArgs,
  readKhalaCodeHeadlessPrompt,
  runKhalaCodeDesktopHeadlessJsonl,
} from "./headless.js"
import {
  KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
  makeKhalaCodeArchitectCoderJudgeRegistry,
} from "../shared/model-role-preset.js"
import { createCodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import { createClaudeAppSdkChatRuntime } from "./claude-app-sdk-chat-runtime.js"
import { createClaudeHeadlessAutoDenyApprovalService } from "./claude-approvals.js"
import { createKhalaCodeDesktopClaudeTokenUsageReporter } from "./claude-token-usage-telemetry.js"
import { createCodexAppServerHost } from "./codex-app-server-client.js"
import {
  createKhalaCodeDesktopCodexMessageTokenAuditRecorder,
  createKhalaCodeDesktopCodexTokenUsageReporter,
  startKhalaCodeDesktopTokenUsageBackgroundSync,
} from "./codex-token-usage-telemetry.js"
import { createOnDeviceDeciderHost } from "./on-device-decider-host.js"
import { ensureKhalaCodeDesktopBundledSkillsInstalled } from "./khala-bundled-skills.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"
import { resolveKhalaCodeDesktopMobilePairingCredentials } from "./harness-setting.js"
import {
  createKhalaCodeDesktopKhalaSyncService,
  khalaCodeDesktopKhalaSyncFleetEnabled,
} from "./khala-sync-service.js"
import { startKhalaCodeDesktopFleetAccountStateReporter } from "./fleet-account-state-reporter.js"
import { createKhalaCodeDesktopFleetRunSupervisorRpcAdapter } from "./fleet-run-supervisor-rpc-adapter.js"
import { createKhalaCodeDesktopRpcRequestHandlers } from "./rpc-handlers.js"
import { mutatingPreviewRpcMethods } from "./preview-rpc-policy.js"
import { openExternalUrl } from "./khala-fleet-tools.js"
import { createKhalaCodeDesktopUpdaterController } from "./khala-code-updater-controller.js"
import { createKhalaCodeDesktopElectrobunUpdaterBackend } from "./khala-code-updater-electrobun-backend.js"
import {
  handleKhalaCodeApplicationMenuAction,
} from "./khala-code-updater-menu-actions.js"
import {
  KHALA_CODE_DESKTOP_UPDATER_DEV_CHANNEL,
  khalaCodeDesktopUpdaterDisabledLocalInfo,
} from "../shared/updater.js"

const khalaCodeConfig = khalaCodeConfigFromRuntimeEnv()
const khalaCodeEnv = khalaCodeConfig.env

// Bundled skills (default-on, KHALA_CODE_DESKTOP_BUNDLED_SKILLS=0 disables):
// materialize into ~/.agents/skills so the Codex harness discovers them on
// the next skills/list. Fail-soft — a write failure never blocks startup.
// Logs go to stderr: headless `khala code --json` owns stdout for JSONL.
void ensureKhalaCodeDesktopBundledSkillsInstalled({ env: khalaCodeEnv })
  .then(results => {
    for (const result of results) {
      if (result.status === "installed" || result.status === "updated") {
        process.stderr.write(`[khala-code] bundled skill ${result.name} ${result.status} at ${result.path}\n`)
      } else if (result.status === "write_failed") {
        process.stderr.write(`[khala-code] bundled skill ${result.name} write failed at ${result.path}\n`)
      }
    }
  })
  .catch(error => {
    process.stderr.write(`[khala-code] bundled skill install failed: ${String(error)}\n`)
  })

const previewPort = (): number => {
  const parsed = Number(
    khalaCodeEnv.KHALA_CODE_DESKTOP_PREVIEW_PORT ??
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

export const KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN_HEADER =
  "x-khala-code-preview-token"

const previewRpcToken = (): string => {
  const configured = khalaCodeEnv.KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN?.trim()
  return configured === undefined || configured.length === 0
    ? randomBytes(32).toString("base64url")
    : configured
}

const previewBridgeAccessToken = previewRpcToken()
const previewBridgeReadOnly = khalaCodeEnv.KHALA_CODE_DESKTOP_PREVIEW_READONLY === "1"

const isAuthorizedPreviewRpcRequest = (request: Request): boolean => {
  const presented = request.headers.get(KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN_HEADER)
  if (presented === null) return false
  const a = createHash("sha256").update(presented).digest()
  const b = createHash("sha256").update(previewBridgeAccessToken).digest()
  return timingSafeEqual(a, b)
}


const isPreviewRpcMutation = (method: KhalaCodeDesktopRpcMethodName): boolean =>
  mutatingPreviewRpcMethods.has(method)

type PreviewBridgeEvent =
  | Readonly<{
      event: KhalaCodeDesktopChatTurnEvent
      observedAt: string
      type: "chatTurnEvent"
    }>
  | Readonly<{
      detail: unknown
      observedAt: string
      type: "fleetLifecycleEvent" | "runCounterEvent" | "claudeApprovalRequested"
    }>
  | Readonly<{
      args: readonly unknown[]
      level: "debug" | "info" | "warn" | "error"
      observedAt: string
      type: "consoleDiagnostic"
    }>
  | Readonly<{
      message: string
      observedAt: string
      type: "crashDiagnostic"
    }>

const previewEventClients = new Set<ReadableStreamDefaultController<Uint8Array>>()
const textEncoder = new TextEncoder()

const writePreviewSseEvent = (
  controller: ReadableStreamDefaultController<Uint8Array>,
  event: PreviewBridgeEvent,
): void => {
  controller.enqueue(textEncoder.encode(
    `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`,
  ))
}

const publishPreviewBridgeEvent = (event: PreviewBridgeEvent): void => {
  for (const controller of previewEventClients) {
    try {
      writePreviewSseEvent(controller, event)
    } catch {
      previewEventClients.delete(controller)
    }
  }
}

const previewEventsResponse = (request: Request): Response => {
  if (request.method !== "GET") {
    return jsonResponse({
      ok: false,
      error: "method_not_allowed",
      method: "events",
      tag: "rpc_method_not_allowed",
    }, { status: 405 })
  }
  if (!isAuthorizedPreviewRpcRequest(request)) {
    return jsonResponse({
      ok: false,
      error: "unauthorized",
      method: "events",
      tag: "rpc_unauthorized",
    }, { status: 401 })
  }

  // ReadableStream cancel() receives the cancellation reason, not the
  // controller — hold the controller in a closure so disconnects clean up.
  let eventsController: ReadableStreamDefaultController<Uint8Array> | null = null
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      eventsController = controller
      previewEventClients.add(controller)
      controller.enqueue(textEncoder.encode(": khala-code-preview-events\n\n"))
    },
    cancel() {
      if (eventsController !== null) previewEventClients.delete(eventsController)
    },
  })
  return new Response(stream, {
    headers: {
      "cache-control": "no-cache, no-transform",
      "connection": "keep-alive",
      "content-type": "text/event-stream; charset=utf-8",
      "x-accel-buffering": "no",
    },
  })
}

let emitChatTurnEvent = (_event: KhalaCodeDesktopChatTurnEvent): void => {}
let emitFleetLifecycleEvent = (_event: KhalaCodeDesktopFleetLifecycleEvent): void => {}
let emitClaudeApprovalRequested = (_request: KhalaCodeDesktopClaudeApprovalRequestProjection): void => {}
let rpcRequestHandlers: KhalaCodeDesktopRPCSchema["requests"]
const QA_METRIC_SAMPLE_LIMIT = 240
const qaMetricSamples: KhalaCodeDesktopQaMetricSample[] = []

const cloneQaMetricSample = (
  sample: KhalaCodeDesktopQaMetricSample,
): KhalaCodeDesktopQaMetricSample => ({
  ...(sample.context === undefined ? {} : { context: { ...sample.context } }),
  metric: sample.metric,
  observedAt: sample.observedAt,
  unit: sample.unit,
  value: sample.value,
})

const recordQaMetricSample = (sample: KhalaCodeDesktopQaMetricSample): void => {
  if (!Number.isFinite(sample.value)) return
  qaMetricSamples.push(cloneQaMetricSample(sample))
  while (qaMetricSamples.length > QA_METRIC_SAMPLE_LIMIT) qaMetricSamples.shift()
}

const qaMetricsSnapshot = (): KhalaCodeDesktopQaMetricsSnapshot => {
  const samples = qaMetricSamples.map(cloneQaMetricSample)
  return {
    budgets: khalaCodeQaMetricBudgets,
    definitions: khalaCodeQaMetricDefinitions,
    evaluations: evaluateKhalaCodeQaMetricBudgets(samples),
    ok: true,
    observedAt: new Date().toISOString(),
    samples,
    schema: "openagents.khala_code.qa_metrics.v1",
  }
}

const previewRpcResponse = async (
  request: Request,
  method: string,
): Promise<Response> => {
  if (!isAuthorizedPreviewRpcRequest(request)) {
    return jsonResponse({
      ok: false,
      error: "unauthorized",
      method,
      tag: "rpc_unauthorized",
    }, { status: 401 })
  }
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
  if (previewBridgeReadOnly && isPreviewRpcMutation(rpcMethod)) {
    return jsonResponse({
      ok: false,
      error: "read_only",
      method,
      tag: "rpc_read_only",
    }, { status: 403 })
  }
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
    if (rpcMethod.startsWith("fleet") || rpcMethod.startsWith("codexFleet")) {
      publishPreviewBridgeEvent({
        detail: { method: rpcMethod, phase: "started" },
        observedAt: new Date().toISOString(),
        type: "fleetLifecycleEvent",
      })
    }
    const result = await invoke(...args)
    if (rpcMethod.startsWith("fleetRun")) {
      publishPreviewBridgeEvent({
        detail: { method: rpcMethod, phase: "completed" },
        observedAt: new Date().toISOString(),
        type: "runCounterEvent",
      })
    }
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
  if (url.pathname === "/rpc/events") {
    return previewEventsResponse(request)
  }
  if (url.pathname.startsWith("/rpc/")) {
    return previewRpcResponse(request, decodeURIComponent(url.pathname.slice(5)))
  }
  return previewAssetResponse(url.pathname)
}

const startPreviewServer = (): void => {
  if (khalaCodeEnv.KHALA_CODE_DESKTOP_PREVIEW_SERVER === "0") return
  const requestedPort = previewPort()
  for (let offset = 0; offset < 10; offset += 1) {
    const port = requestedPort + offset
    try {
      const server = Bun.serve({
        hostname: "127.0.0.1",
        port,
        fetch: previewFetch,
      })
      console.info(
        `Khala Code desktop web preview: http://localhost:${server.port} ` +
        `(RPC header ${KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN_HEADER})`,
      )
      // Print the per-boot secret exactly once, strictly to local stdout —
      // the instrumented console broadcasts to SSE subscribers, so use the
      // raw stream here.
      process.stdout.write(
        `Khala Code preview access token (this boot): ${previewBridgeAccessToken}\n`,
      )
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

// Tailnet health beacon (MC-5): a separate, read-only listener from the
// RPC-bearing preview server above. It exists solely so a paired mobile
// device can answer "can I see a Khala Code instance on the network" —
// GET /health only, no RPC surface, no token. The preview server stays
// loopback-only on purpose (its /rpc/* routes carry a per-boot secret);
// widening that bind address would widen the RPC attack surface, so this
// beacon is intentionally a second, minimal server bound to all
// interfaces instead.
const KHALA_CODE_DESKTOP_TAILNET_HEALTH_PORT = 50099

const tailnetHealthFetch = (): Response =>
  jsonResponse({
    ok: true,
    app: "Khala Code Desktop",
    hostname: os.hostname(),
    observedAt: new Date().toISOString(),
  })

// Mobile Tailnet auto-auth handoff (MC-6, owner mandate 2026-07-04: "IF
// THERES A DEVICE ON TAILNET THATS AUTHED, USE THAT AUTOMATICALLY - NO LOGIN
// SCREEN"). Lives on the SAME 0.0.0.0 beacon as /health rather than a new
// listener — Tailscale's own network ACL is the real security boundary here
// (only devices already authorized on this tailnet can reach this port at
// all), so this stays a narrowly-scoped read: it returns the (ownerUserId,
// token) pair a signed-in desktop already holds, or an honest
// `not_signed_in` when it doesn't. GET only, no request body, and the
// response is never logged (see resolveKhalaCodeDesktopMobilePairingCredentials
// / harness-setting.ts for where the credentials come from). Disable with
// KHALA_CODE_DESKTOP_MOBILE_PAIRING=0.
const KHALA_CODE_DESKTOP_MOBILE_PAIRING_PATH = "/khala-mobile-pairing"

const tailnetMobilePairingFetch = async (request: Request): Promise<Response> => {
  if (request.method !== "GET") {
    return new Response("method not allowed", { status: 405 })
  }
  const credentials = await resolveKhalaCodeDesktopMobilePairingCredentials(khalaCodeEnv)
  if (credentials === null) {
    // Still name the host even when not signed in — it's not secret, and it
    // lets the mobile fallback screen say "found your Mac, but it isn't
    // signed in yet" instead of a bare "nothing found".
    return jsonResponse({ hostname: os.hostname(), ok: false, reason: "not_signed_in" })
  }
  return jsonResponse({
    ok: true,
    hostname: os.hostname(),
    observedAt: new Date().toISOString(),
    ownerUserId: credentials.ownerUserId,
    token: credentials.token,
  })
}

const startTailnetHealthBeacon = (): void => {
  if (khalaCodeEnv.KHALA_CODE_DESKTOP_TAILNET_HEALTH === "0") return
  const requestedPort = Number(
    khalaCodeEnv.KHALA_CODE_DESKTOP_TAILNET_HEALTH_PORT ??
      String(KHALA_CODE_DESKTOP_TAILNET_HEALTH_PORT),
  )
  const port = Number.isInteger(requestedPort) && requestedPort > 0
    ? requestedPort
    : KHALA_CODE_DESKTOP_TAILNET_HEALTH_PORT
  const mobilePairingEnabled = khalaCodeEnv.KHALA_CODE_DESKTOP_MOBILE_PAIRING !== "0"
  try {
    const server = Bun.serve({
      hostname: "0.0.0.0",
      port,
      fetch: (request) => {
        const url = new URL(request.url)
        if (url.pathname === "/health") return tailnetHealthFetch()
        if (mobilePairingEnabled && url.pathname === KHALA_CODE_DESKTOP_MOBILE_PAIRING_PATH) {
          return tailnetMobilePairingFetch(request)
        }
        return new Response("not found", { status: 404 })
      },
    })
    console.info(
      `Khala Code Tailnet health beacon: http://0.0.0.0:${server.port}/health`,
    )
  } catch (error) {
    console.warn(
      `Khala Code Tailnet health beacon unavailable: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
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
  const headlessArgs = parseKhalaCodeHeadlessArgs(promptArgv)
  const prompt = await readKhalaCodeHeadlessPrompt(promptArgv)
  if (prompt.length === 0) {
    process.stderr.write("khala code --json requires a prompt argument or stdin.\n")
    process.exit(2)
  }
  const workingDirectory = resolveToolWorkingDirectory(khalaCodeEnv)
  const headlessCodexAppServerHost = createCodexAppServerHost({ env: khalaCodeEnv })
  const interruptAfterMs = headlessInterruptAfterMs(khalaCodeEnv)
  let exitCode = 0
  try {
    if (headlessArgs.preset === "architect-coder-judge") {
      await headlessCodexAppServerHost.request("config/value/write", {
        keyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
        mergeStrategy: "replace",
        value: makeKhalaCodeArchitectCoderJudgeRegistry(),
      })
      process.stderr.write("[khala-code] applied preset architect-coder-judge\n")
    }
    await runKhalaCodeDesktopHeadlessJsonl({
      createChatRuntime: ({ onEvent }) =>
        khalaCodeEnv.KHALA_CODE_DESKTOP_RUNTIME === "claude_runtime"
          ? createClaudeAppSdkChatRuntime({
            approvalService: createClaudeHeadlessAutoDenyApprovalService(),
            env: khalaCodeEnv,
            onEvent,
            repoRoot: resolveSourceRepositoryRoot(),
            tokenUsageReporter: createKhalaCodeDesktopClaudeTokenUsageReporter({ env: khalaCodeEnv }),
            workingDirectory,
          })
          : createCodexAppServerChatRuntime({
            env: khalaCodeEnv,
            host: headlessCodexAppServerHost,
            onEvent,
            messageTokenAuditRecorder:
              createKhalaCodeDesktopCodexMessageTokenAuditRecorder({ env: khalaCodeEnv }),
            tokenUsageReporter: createKhalaCodeDesktopCodexTokenUsageReporter({ env: khalaCodeEnv }),
            workingDirectory,
          }),
      env: khalaCodeEnv,
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

const { ApplicationMenu, BrowserView, BrowserWindow, Screen, Updater } = await import("electrobun/bun")

// #8440 in-app updater plumbing. Electrobun's Updater.getLocalInfo() reads
// the packaged `Resources/version.json`; it resolves to empty strings (never
// rejects) when unpackaged/dev, which the disabled local-info fallback below
// treats as the honest "updater disabled" state rather than a real channel.
const rawKhalaCodeUpdaterLocalInfo = await Updater.getLocalInfo()
const khalaCodeUpdaterLocalInfo = rawKhalaCodeUpdaterLocalInfo.baseUrl.length === 0
  ? khalaCodeDesktopUpdaterDisabledLocalInfo(rawKhalaCodeUpdaterLocalInfo.version || "0.0.0-dev")
  : rawKhalaCodeUpdaterLocalInfo
const khalaCodeUpdaterEnabled =
  khalaCodeUpdaterLocalInfo.channel !== KHALA_CODE_DESKTOP_UPDATER_DEV_CHANNEL
const khalaCodeUpdaterController = createKhalaCodeDesktopUpdaterController({
  backend: createKhalaCodeDesktopElectrobunUpdaterBackend({
    currentVersion: khalaCodeUpdaterLocalInfo.version,
    updater: Updater,
  }),
  channel: khalaCodeUpdaterLocalInfo.channel,
  currentVersion: khalaCodeUpdaterLocalInfo.version,
  enabled: khalaCodeUpdaterEnabled,
  log: (message, data) => {
    process.stderr.write(`[khala-code-updater] ${message} ${JSON.stringify(data ?? {})}\n`)
  },
})
const stopKhalaCodeUpdaterPeriodicChecks = khalaCodeUpdaterController.startPeriodicChecks(
  Number(khalaCodeEnv.KHALA_CODE_DESKTOP_UPDATE_CHECK_INTERVAL_MS ?? String(4 * 60 * 60 * 1000)),
)
ApplicationMenu.on("application-menu-clicked", event => {
  const action = event.data?.action
  handleKhalaCodeApplicationMenuAction(action, {
    checkForUpdates: () => khalaCodeUpdaterController.check(),
    openReleaseNotes: () => openExternalUrl(khalaCodeUpdaterController.status().releaseNotesUrl),
  })
})

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
const codexAppServerHost = createCodexAppServerHost({ env: khalaCodeEnv })
const tokenUsageBackgroundSync = startKhalaCodeDesktopTokenUsageBackgroundSync({
  env: khalaCodeEnv,
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
const onDeviceDecider = createOnDeviceDeciderHost({ env: khalaCodeEnv })

// Khala Sync fleet consumer (KS-6.2, #8303): default-on. Set
// KHALA_SYNC_FLEET=0/false/off to force the honest disabled RPC state.
const khalaSyncService = khalaCodeDesktopKhalaSyncFleetEnabled(khalaCodeEnv)
  ? createKhalaCodeDesktopKhalaSyncService({ env: khalaCodeEnv })
  : null

// Recurring Khala Sync `fleet_account` state reporter (#8406): enumerates
// BOTH local Codex and Claude Pylon accounts (`inspectCodexFleet({ workerKind:
// "auto" })`) and pushes each into the configured `fleet_run` scope(s) via
// `fleet.reportAccountState` on a fixed interval. A no-op (never guesses a
// run id) when KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID is unset; see
// fleet-account-state-reporter.ts for the full honesty contract.
const fleetAccountStateReporter = khalaSyncService === null
  ? null
  : startKhalaCodeDesktopFleetAccountStateReporter({
    env: khalaCodeEnv,
    khalaSync: khalaSyncService,
    onError: error => {
      console.warn(
        `Khala Sync fleet account state report failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    },
  })

rpcRequestHandlers = createKhalaCodeDesktopRpcRequestHandlers({
  appleFmReadiness,
  codexAppServerHost,
  enableFleetMcpBridge: true,
  emitChatTurnEvent: event => emitChatTurnEvent(event),
  emitClaudeApprovalRequested: request => emitClaudeApprovalRequested(request),
  env: khalaCodeEnv,
  fleetRunSupervisor: createKhalaCodeDesktopFleetRunSupervisorRpcAdapter({
    env: khalaCodeEnv,
    onLifecycleNdjson: line => emitFleetLifecycleEvent({
      line,
      observedAt: new Date().toISOString(),
    }),
  }),
  fleetMcpBridgeRepoRoot: resolveSourceRepositoryRoot(),
  ...(khalaSyncService === null ? {} : { khalaSync: khalaSyncService }),
  onDeviceDeciderStatus: () => onDeviceDecider.select(),
  qaMetrics: qaMetricsSnapshot,
  recordQaMetricSample,
  updaterController: khalaCodeUpdaterController,
  workingDirectory: resolveToolWorkingDirectory(khalaCodeEnv),
})

const disposeRuntime = (): void => {
  tokenUsageBackgroundSync.dispose()
  codexAppServerHost.dispose()
  fleetAccountStateReporter?.dispose()
  stopKhalaCodeUpdaterPeriodicChecks()
  void khalaSyncService?.close()
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

emitChatTurnEvent = event => {
  try {
    rpc.send.chatTurnEvent(event)
  } catch {
    // Headless preview runs have no native window transport; SSE remains active.
  }
  publishPreviewBridgeEvent({
    event,
    observedAt: new Date().toISOString(),
    type: "chatTurnEvent",
  })
}

emitFleetLifecycleEvent = event => {
  try {
    rpc.send.fleetLifecycleEvent(event)
  } catch {
    // Headless preview runs have no native window transport; SSE remains active.
  }
  publishPreviewBridgeEvent({
    detail: event,
    observedAt: event.observedAt,
    type: "fleetLifecycleEvent",
  })
}

emitClaudeApprovalRequested = request => {
  try {
    rpc.send.claudeApprovalRequested(request)
  } catch {
    // Headless preview runs have no native window transport; the 1s
    // claudeApprovalPending poll remains the fallback for those runs.
  }
  publishPreviewBridgeEvent({
    detail: request,
    observedAt: new Date().toISOString(),
    type: "claudeApprovalRequested",
  })
}

const originalConsole = {
  debug: console.debug.bind(console),
  error: console.error.bind(console),
  info: console.info.bind(console),
  warn: console.warn.bind(console),
}

for (const level of ["debug", "info", "warn", "error"] as const) {
  console[level] = (...args: unknown[]) => {
    originalConsole[level](...args)
    publishPreviewBridgeEvent({
      args,
      level,
      observedAt: new Date().toISOString(),
      type: "consoleDiagnostic",
    })
  }
}

const publishCrashDiagnostic = (error: unknown): void => {
  publishPreviewBridgeEvent({
    message: error instanceof Error ? error.message : String(error),
    observedAt: new Date().toISOString(),
    type: "crashDiagnostic",
  })
}

process.on("uncaughtExceptionMonitor", publishCrashDiagnostic)
process.on("unhandledRejection", publishCrashDiagnostic)

startPreviewServer()
startTailnetHealthBeacon()

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

if (khalaCodeEnv.KHALA_CODE_DESKTOP_OPEN_WINDOW !== "0") {
  ApplicationMenu.setApplicationMenu(khalaCodeDesktopApplicationMenu)

  new BrowserWindow({
    title: "Khala Code",
    url: await resolveMainViewUrl(),
    frame: resolveMainWindowFrame(),
    titleBarStyle: "hiddenInset",
    rpc,
  })
}
