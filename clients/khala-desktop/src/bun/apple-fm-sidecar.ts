import { existsSync, statSync } from "node:fs"
import { dirname, join, resolve } from "node:path"

import {
  APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
} from "../shared/apple-fm-packaging.js"
import {
  buildKhalaAppleFmReadiness,
  type KhalaAppleFmReadiness,
  type PylonAppleFmStatusPublicInput,
} from "../shared/apple-fm-readiness.js"

type SidecarLaunchState = "idle" | "launching" | "running" | "failed" | "stopped" | "adopted"
type HelperSource = "env" | "source-wrapper" | "source-build" | "packaged-resource"

type DiscoveredAppleFmBridgeHelper = {
  readonly path: string
  readonly source: HelperSource
}

export type AppleFmSidecarHost = {
  readonly readiness: () => Promise<KhalaAppleFmReadiness>
  readonly stop: () => void
}

type AppleFmShutdownEvent = "beforeExit" | "exit" | "SIGINT" | "SIGTERM"

type AppleFmShutdownProcess = {
  readonly once: (event: AppleFmShutdownEvent, handler: (...args: ReadonlyArray<unknown>) => void) => unknown
  readonly exit?: (code?: number) => never
}

type AppleFmSidecarHostOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly platform?: NodeJS.Platform
  readonly arch?: string
  readonly resourcesDir?: string
  readonly fetchFn?: typeof fetch
  readonly spawn?: typeof Bun.spawn
  readonly now?: () => string
}

const APPLE_FM_BRIDGE_DEFAULT_PORT = 11435

const trim = (value: string | undefined): string | null => {
  const trimmed = value?.trim() ?? ""
  return trimmed.length === 0 ? null : trimmed
}

function ancestors(start: string): ReadonlyArray<string> {
  const values: string[] = []
  let current = start
  while (true) {
    values.push(current)
    const next = dirname(current)
    if (next === current) return values
    current = next
  }
}

function discoverAppleFmBridgeHelper(input: {
  readonly cwd: string
  readonly env: Readonly<Record<string, string | undefined>>
  readonly resourcesDir?: string
}): DiscoveredAppleFmBridgeHelper | null {
  const explicit = trim(input.env.OPENAGENTS_APPLE_FM_BRIDGE_PATH)
  if (explicit !== null) {
    const path = resolve(explicit)
    if (existsSync(path)) return { path, source: "env" }
  }

  if (input.resourcesDir !== undefined) {
    const packaged = join(input.resourcesDir, APPLE_FM_BRIDGE_RESOURCES_SUBPATH)
    if (existsSync(packaged)) return { path: packaged, source: "packaged-resource" }
  }

  for (const ancestor of ancestors(resolve(input.cwd))) {
    for (const pylonRoot of [ancestor, join(ancestor, "apps", "pylon")]) {
      const wrapper = join(pylonRoot, "bin", "foundation-bridge")
      if (existsSync(wrapper)) return { path: wrapper, source: "source-wrapper" }

      const sourceBuild = join(pylonRoot, "swift", "foundation-bridge", ".build", "release", "foundation-bridge")
      if (existsSync(sourceBuild)) return { path: sourceBuild, source: "source-build" }
    }
  }

  return null
}

function controlBaseUrlFromEnv(env: Readonly<Record<string, string | undefined>>): string {
  const explicit = trim(env.PYLON_CONTROL_URL)
  if (explicit !== null) return explicit.replace(/\/+$/, "")
  const host = trim(env.PYLON_CONTROL_HOST) ?? "127.0.0.1"
  const port = Number(trim(env.PYLON_CONTROL_PORT) ?? 4716)
  return `http://${host}:${Number.isFinite(port) ? port : 4716}`
}

async function controlTokenFromEnv(
  env: Readonly<Record<string, string | undefined>>,
): Promise<string | null> {
  const explicit = trim(env.PYLON_CONTROL_TOKEN)
  if (explicit !== null) return explicit
  const home = trim(env.PYLON_HOME)
  if (home === null) return null
  try {
    const text = await Bun.file(join(home, "control-token")).text()
    const token = text.trim()
    return token.length >= 16 ? token : null
  } catch {
    return null
  }
}

function helperExecutable(path: string): boolean {
  try {
    const stat = statSync(path)
    return stat.isFile() && stat.size > 0 && (stat.mode & 0o100) !== 0
  } catch {
    return false
  }
}

async function fetchPylonAppleFmStatus(input: {
  readonly baseUrl: string
  readonly token: string
  readonly fetchFn: typeof fetch
}): Promise<PylonAppleFmStatusPublicInput | null> {
  try {
    const response = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${input.token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({ type: "apple_fm.status" }),
      signal: AbortSignal.timeout(1_500),
    })
    if (!response.ok) return null
    const json = await response.json() as { ok?: unknown; result?: unknown }
    return json.ok === true && typeof json.result === "object" && json.result !== null
      ? json.result as PylonAppleFmStatusPublicInput
      : null
  } catch {
    return null
  }
}

export function createAppleFmSidecarHost(
  options: AppleFmSidecarHostOptions = {},
): AppleFmSidecarHost {
  const env = options.env ?? Bun.env
  const platform = options.platform ?? process.platform
  const arch = options.arch ?? process.arch
  const resourcesDir = options.resourcesDir ?? (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath
  const fetchFn = options.fetchFn ?? fetch
  const spawn = options.spawn ?? Bun.spawn
  const now = options.now ?? (() => new Date().toISOString())
  const discoverOptions = {
    cwd: process.cwd(),
    env,
    ...(resourcesDir === undefined ? {} : { resourcesDir }),
  }
  const helper = discoverAppleFmBridgeHelper(discoverOptions)
  const supported = platform === "darwin" && arch === "arm64"
  const executable = helper === null ? false : helperExecutable(helper.path)
  let launchState: SidecarLaunchState = "idle"
  let child: ReturnType<typeof Bun.spawn> | null = null

  const start = () => {
    if (!supported || helper === null || !executable || child !== null) return
    if (helper.source !== "packaged-resource" && trim(env.OPENAGENTS_APPLE_FM_BRIDGE_PATH) === null) {
      launchState = "adopted"
      return
    }
    try {
      launchState = "launching"
      child = spawn([helper.path, "--port", String(APPLE_FM_BRIDGE_DEFAULT_PORT)], {
        stdin: "ignore",
        stdout: "ignore",
        stderr: "ignore",
      })
      launchState = "running"
      void child.exited.then((exitCode) => {
        child = null
        launchState = exitCode === 0 ? "stopped" : "failed"
      })
    } catch {
      launchState = "failed"
      child = null
    }
  }

  return {
    async readiness() {
      start()
      const token = await controlTokenFromEnv(env)
      const pylonStatus =
        token === null
          ? null
          : await fetchPylonAppleFmStatus({
              baseUrl: controlBaseUrlFromEnv(env),
              token,
              fetchFn,
            })
      return buildKhalaAppleFmReadiness({
        platform: { platform, arch },
        helperFound: helper !== null,
        helperExecutable: executable,
        helperLaunchState: launchState,
        pylonControlConfigured: token !== null,
        pylonStatus,
        observedAt: now(),
      })
    },
    stop() {
      if (child !== null) {
        child.kill()
        child = null
      }
      if (launchState === "running" || launchState === "launching") {
        launchState = "stopped"
      }
    },
  }
}

export function installAppleFmSidecarShutdownHandlers(
  host: AppleFmSidecarHost,
  processLike: AppleFmShutdownProcess = process,
): void {
  let stopped = false
  const stopOnce = () => {
    if (stopped) return
    stopped = true
    host.stop()
  }

  processLike.once("beforeExit", stopOnce)
  processLike.once("exit", stopOnce)
  processLike.once("SIGINT", () => {
    stopOnce()
    processLike.exit?.(130)
  })
  processLike.once("SIGTERM", () => {
    stopOnce()
    processLike.exit?.(143)
  })
}
