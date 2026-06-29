import { existsSync } from "node:fs"
import { arch, platform } from "node:os"
import { join, resolve } from "node:path"

import {
  APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST,
  APPLE_FM_BRIDGE_HELPER_BASENAME,
} from "../shared/apple-fm-packaging.js"

export const APPLE_FM_DEFAULT_PORT = 11435 as const
export const APPLE_FM_READY_TIMEOUT_MS = 2_000 as const
export const OPENAGENTS_APPLE_FM_BRIDGE_PATH = "OPENAGENTS_APPLE_FM_BRIDGE_PATH" as const
export const OPENAGENTS_APPLE_FM_BASE_URL = "OPENAGENTS_APPLE_FM_BASE_URL" as const
export const PROBE_APPLE_FM_BASE_URL = "PROBE_APPLE_FM_BASE_URL" as const
export const KHALA_DESKTOP_APPLE_FM_AUTOSTART =
  "KHALA_DESKTOP_APPLE_FM_AUTOSTART" as const

export type AppleFmSidecarState =
  | "not_supported"
  | "helper_missing"
  | "candidate"
  | "launching"
  | "adopted"
  | "running"
  | "unavailable"
  | "ready"
  | "failed"
  | "stopped"

export type AppleFmSidecarHelperSource =
  | "env"
  | "packaged-resource"
  | "source-build"
  | "source-wrapper"

export type AppleFmSidecarPublicStatus = {
  readonly available: boolean
  readonly blockerRefs: ReadonlyArray<string>
  readonly contentRedacted: true
  readonly helperSource: AppleFmSidecarHelperSource | null
  readonly launchedByApp: boolean
  readonly message: string
  readonly observedAt: string
  readonly state: AppleFmSidecarState
}

export type AppleFmSidecarProcess = {
  readonly exited?: Promise<unknown>
  kill(): void
}

export type AppleFmSidecarSpawn = (
  command: ReadonlyArray<string>,
  options: { readonly stderr: "ignore"; readonly stdin: "ignore"; readonly stdout: "ignore" },
) => AppleFmSidecarProcess

export type AppleFmSidecarRuntime = {
  readonly arch?: string
  readonly cwd?: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly exists?: (path: string) => boolean
  readonly fetch?: typeof fetch
  readonly now?: () => Date
  readonly platform?: string
  readonly resourcesDir?: string
  readonly spawn?: AppleFmSidecarSpawn
}

type HelperCandidate = {
  readonly path: string
  readonly source: AppleFmSidecarHelperSource
}

export const buildAppleFmBaseUrl = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): string => {
  const configured =
    env[OPENAGENTS_APPLE_FM_BASE_URL] ?? env[PROBE_APPLE_FM_BASE_URL]
  if (configured !== undefined && configured.trim() !== "") {
    return configured.trim().replace(/\/+$/, "")
  }
  return `http://127.0.0.1:${APPLE_FM_DEFAULT_PORT}`
}

export const appleFmIsSupportedHost = (
  input: Readonly<{ readonly arch?: string; readonly platform?: string }> = {},
): boolean =>
  (input.platform ?? platform()) === "darwin" && (input.arch ?? arch()) === "arm64"

export const discoverAppleFmHelper = (
  runtime: AppleFmSidecarRuntime = {},
): HelperCandidate | null => {
  const env = runtime.env ?? process.env
  const exists = runtime.exists ?? existsSync
  const explicit = env[OPENAGENTS_APPLE_FM_BRIDGE_PATH]
  if (explicit !== undefined && explicit.trim() !== "") {
    const path = resolve(explicit)
    if (exists(path)) return { path, source: "env" }
  }

  const processWithResources = process as NodeJS.Process & { readonly resourcesPath?: string }
  const resourcesDir = runtime.resourcesDir ?? processWithResources.resourcesPath
  if (resourcesDir !== undefined) {
    const packaged = join(resourcesDir, "app", APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST)
    if (exists(packaged)) return { path: packaged, source: "packaged-resource" }
  }

  for (const root of candidateWorkspaceRoots(resolve(runtime.cwd ?? process.cwd()))) {
    const built = join(
      root,
      "apps",
      "pylon",
      "swift",
      "foundation-bridge",
      ".build",
      "release",
      APPLE_FM_BRIDGE_HELPER_BASENAME,
    )
    if (exists(built)) return { path: built, source: "source-build" }

    const wrapper = join(root, "apps", "pylon", "bin", APPLE_FM_BRIDGE_HELPER_BASENAME)
    if (exists(wrapper)) return { path: wrapper, source: "source-wrapper" }
  }

  return null
}

export class AppleFmSidecarManager {
  readonly #runtime: AppleFmSidecarRuntime
  #process: AppleFmSidecarProcess | null = null
  #state: AppleFmSidecarState = "candidate"
  #helperSource: AppleFmSidecarHelperSource | null = null
  #lastMessage = "Local Apple FM sidecar has not been checked yet."
  #launchedByApp = false

  constructor(runtime: AppleFmSidecarRuntime = {}) {
    this.#runtime = runtime
  }

  async status(): Promise<AppleFmSidecarPublicStatus> {
    const observedAt = this.#now().toISOString()

    if (!appleFmIsSupportedHost(this.#runtime)) {
      return this.#snapshot({
        message: "Local Apple FM requires macOS on Apple Silicon.",
        observedAt,
        state: "not_supported",
      })
    }

    const helper = discoverAppleFmHelper(this.#runtime)
    this.#helperSource = helper?.source ?? null
    if (helper === null) {
      return this.#snapshot({
        message: "The Foundation Models bridge helper is not bundled or configured.",
        observedAt,
        state: "helper_missing",
      })
    }

    const baseUrl = buildAppleFmBaseUrl(this.#runtime.env)
    const health = await probeAppleFmHealth({
      baseUrl,
      fetch: this.#runtime.fetch ?? fetch,
      timeoutMs: APPLE_FM_READY_TIMEOUT_MS,
    })
    if (health.ready) {
      const adopted = this.#process === null
      return this.#snapshot({
        message: adopted
          ? "A trusted loopback Foundation Models bridge is already ready."
          : "The app-launched Foundation Models bridge is ready.",
        observedAt,
        state: adopted ? "adopted" : "ready",
      })
    }

    if (this.#process !== null) {
      return this.#snapshot({
        message: "The Foundation Models bridge is launching; Pylon readiness is not ready yet.",
        observedAt,
        state: "launching",
      })
    }

    if (!shouldAutostart(this.#runtime.env)) {
      return this.#snapshot({
        message: "Local Apple FM sidecar is available but autostart is disabled.",
        observedAt,
        state: "candidate",
      })
    }

    try {
      this.#process = (this.#runtime.spawn ?? Bun.spawn)([helper.path, "--port", String(APPLE_FM_DEFAULT_PORT)], {
        stderr: "ignore",
        stdin: "ignore",
        stdout: "ignore",
      })
      this.#launchedByApp = true
      void this.#process.exited?.then(() => {
        this.#process = null
        this.#state = "stopped"
        this.#lastMessage = "The app-launched Foundation Models bridge exited."
      })
      return this.#snapshot({
        message: "Launching the local Foundation Models bridge for Pylon readiness.",
        observedAt,
        state: "launching",
      })
    } catch {
      return this.#snapshot({
        message: "The Foundation Models bridge could not be launched.",
        observedAt,
        state: "failed",
      })
    }
  }

  stop(): AppleFmSidecarPublicStatus {
    this.#process?.kill()
    this.#process = null
    return this.#snapshot({
      message: "The app-launched Foundation Models bridge has been stopped.",
      observedAt: this.#now().toISOString(),
      state: "stopped",
    })
  }

  #now(): Date {
    return (this.#runtime.now ?? (() => new Date()))()
  }

  #snapshot(input: {
    readonly message: string
    readonly observedAt: string
    readonly state: AppleFmSidecarState
  }): AppleFmSidecarPublicStatus {
    this.#state = input.state
    this.#lastMessage = input.message
    const available = input.state === "ready" || input.state === "adopted"
    return {
      available,
      blockerRefs: sidecarBlockerRefs(input.state),
      contentRedacted: true,
      helperSource: this.#helperSource,
      launchedByApp: this.#launchedByApp,
      message: this.#lastMessage,
      observedAt: input.observedAt,
      state: this.#state,
    }
  }
}

export const probeAppleFmHealth = async (input: {
  readonly baseUrl: string
  readonly fetch: typeof fetch
  readonly timeoutMs: number
}): Promise<{ readonly ready: boolean }> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), input.timeoutMs)
  try {
    const response = await input.fetch(`${input.baseUrl}/health`, {
      signal: controller.signal,
    })
    if (!response.ok) return { ready: false }
    const body = await response.json().catch(() => null)
    if (typeof body !== "object" || body === null) return { ready: false }
    const record = body as Record<string, unknown>
    return {
      ready:
        record.ready === true ||
        record.status === "ready" ||
        record.available === true,
    }
  } catch {
    return { ready: false }
  } finally {
    clearTimeout(timer)
  }
}

export const sidecarBlockerRefs = (
  state: AppleFmSidecarState,
): ReadonlyArray<string> => {
  switch (state) {
    case "ready":
    case "adopted":
    case "running":
      return []
    case "not_supported":
      return ["blocker.khala_desktop.apple_fm.unsupported_host"]
    case "helper_missing":
      return ["blocker.khala_desktop.apple_fm.helper_missing"]
    case "candidate":
      return ["blocker.khala_desktop.apple_fm.autostart_disabled"]
    case "launching":
      return ["blocker.khala_desktop.apple_fm.pylon_readiness_pending"]
    case "unavailable":
      return ["blocker.khala_desktop.apple_fm.bridge_unavailable"]
    case "failed":
      return ["blocker.khala_desktop.apple_fm.launch_failed"]
    case "stopped":
      return ["blocker.khala_desktop.apple_fm.stopped"]
  }
}

const shouldAutostart = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean => env[KHALA_DESKTOP_APPLE_FM_AUTOSTART] === "1"

const candidateWorkspaceRoots = (cwd: string): ReadonlyArray<string> => {
  const roots: string[] = []
  let current = cwd
  while (true) {
    roots.push(current)
    const parent = join(current, "..")
    const resolvedParent = resolve(parent)
    if (resolvedParent === current) return roots
    current = resolvedParent
  }
}
