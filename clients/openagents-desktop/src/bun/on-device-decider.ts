import { existsSync, statSync } from "node:fs"
import { dirname, join, resolve } from "node:path"

import {
  APPLE_FM_BACKEND_KIND,
  APPLE_FM_DEFAULT_MODEL_ID,
  appleFmPreferredOnPlatform,
  buildOnDeviceDeciderStatus,
  decisionFromBackendJson,
  decodeOnDeviceDeciderRequest,
  disabledOnDeviceDeciderStatus,
  GPT_OSS_BACKEND_KIND,
  GPT_OSS_DEFAULT_MODEL_ID,
  OPENAGENTS_DESKTOP_APPLE_FM_DEFAULT_BASE_URL,
  parseOnDeviceDeciderConfig,
  selectOnDeviceDeciderBackend,
  type OnDeviceDeciderBackendKind,
  type OnDeviceDeciderDecision,
  type OnDeviceDeciderMode,
  type OnDeviceDeciderPlatform,
  type OnDeviceDeciderRequest,
  type OnDeviceDeciderRunResult,
  type OnDeviceDeciderStatus,
} from "../shared/on-device-decider.js"

type HelperSource = "env" | "source-wrapper" | "source-build" | "packaged-resource"

type DiscoveredAppleFmBridgeHelper = {
  readonly path: string
  readonly source: HelperSource
}

export type OnDeviceDeciderBackend = {
  readonly decide: (
    input: OnDeviceDeciderRequest,
  ) => Promise<OnDeviceDeciderDecision>
  readonly kind: OnDeviceDeciderBackendKind
  readonly status: () => Promise<OnDeviceDeciderStatus>
}

export type OnDeviceDeciderService = {
  readonly decide: (
    input: OnDeviceDeciderRequest,
  ) => Promise<OnDeviceDeciderRunResult>
  readonly status: () => Promise<OnDeviceDeciderStatus>
}

type OnDeviceDeciderServiceOptions = {
  readonly arch?: string
  readonly cwd?: string
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly fetchFn?: typeof fetch
  readonly now?: () => string
  readonly platform?: NodeJS.Platform | "ios" | string
  readonly resourcesDir?: string
  readonly spawn?: typeof Bun.spawn
}

const APPLE_FM_BRIDGE_RESOURCES_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const
const APPLE_FM_BRIDGE_DEFAULT_PORT = 11435

const trim = (value: string | undefined): string | null => {
  const trimmed = value?.trim() ?? ""
  return trimmed.length === 0 ? null : trimmed
}

const compactError = (value: unknown): string => {
  const text = (value instanceof Error ? value.message : String(value))
    .replace(/Bearer\s+[A-Za-z0-9._~+/=-]+/gi, "Bearer [REDACTED]")
    .replace(/\/Users\/[^/\s]+/g, "/Users/[REDACTED]")
    .replace(/https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?[^\s"']*/gi, "[LOCAL_URL]")
  return text.length > 180 ? `${text.slice(0, 177)}...` : text
}

const ancestors = (start: string): readonly string[] => {
  const values: string[] = []
  let current = resolve(start)
  while (true) {
    values.push(current)
    const next = dirname(current)
    if (next === current) return values
    current = next
  }
}

const helperExecutable = (path: string): boolean => {
  try {
    const fileStat = statSync(path)
    return fileStat.isFile() && fileStat.size > 0 && (fileStat.mode & 0o100) !== 0
  } catch {
    return false
  }
}

const discoverAppleFmBridgeHelper = (input: {
  readonly cwd: string
  readonly env: Readonly<Record<string, string | undefined>>
  readonly resourcesDir?: string
}): DiscoveredAppleFmBridgeHelper | null => {
  const explicit = trim(input.env.OPENAGENTS_APPLE_FM_BRIDGE_PATH)
  if (explicit !== null) {
    const path = resolve(explicit)
    if (existsSync(path)) return { path, source: "env" }
  }

  if (input.resourcesDir !== undefined) {
    const packaged = join(input.resourcesDir, APPLE_FM_BRIDGE_RESOURCES_SUBPATH)
    if (existsSync(packaged)) {
      return { path: packaged, source: "packaged-resource" }
    }
  }

  for (const ancestor of ancestors(input.cwd)) {
    for (const pylonRoot of [ancestor, join(ancestor, "apps", "pylon")]) {
      const wrapper = join(pylonRoot, "bin", "foundation-bridge")
      if (existsSync(wrapper)) return { path: wrapper, source: "source-wrapper" }

      const sourceBuild = join(
        pylonRoot,
        "swift",
        "foundation-bridge",
        ".build",
        "release",
        "foundation-bridge",
      )
      if (existsSync(sourceBuild)) {
        return { path: sourceBuild, source: "source-build" }
      }
    }
  }

  return null
}

const loopbackHost = (host: string): boolean =>
  host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "[::1]"

const localBaseUrl = (value: string | undefined): string | null => {
  const raw = trim(value)
  if (raw === null) return null
  try {
    const url = new URL(raw)
    if (url.protocol !== "http:" || !loopbackHost(url.hostname)) return null
    url.pathname = url.pathname.replace(/\/+$/, "")
    url.search = ""
    url.hash = ""
    return url.toString().replace(/\/+$/, "")
  } catch {
    return null
  }
}

const endpointUrl = (baseUrl: string, path: string): string =>
  new URL(path, `${baseUrl}/`).toString()

const extractAssistantText = (payload: unknown): string | null => {
  if (typeof payload === "string") return payload
  if (typeof payload !== "object" || payload === null || Array.isArray(payload)) {
    return null
  }
  const record = payload as Record<string, unknown>
  const choices = record.choices
  if (Array.isArray(choices)) {
    const first = choices[0]
    if (typeof first === "object" && first !== null && !Array.isArray(first)) {
      const message = (first as Record<string, unknown>).message
      if (typeof message === "object" && message !== null && !Array.isArray(message)) {
        const content = (message as Record<string, unknown>).content
        return typeof content === "string" ? content : null
      }
      const text = (first as Record<string, unknown>).text
      return typeof text === "string" ? text : null
    }
  }
  const content = record.content
  return typeof content === "string" ? content : null
}

const extractJson = (text: string): unknown => {
  try {
    return JSON.parse(text)
  } catch {
    const start = text.indexOf("{")
    const end = text.lastIndexOf("}")
    if (start >= 0 && end > start) {
      return JSON.parse(text.slice(start, end + 1))
    }
    throw new Error("decider response did not contain JSON")
  }
}

const promptFor = (input: OnDeviceDeciderRequest): readonly {
  readonly content: string
  readonly role: "system" | "user"
}[] => [
  {
    role: "system",
    content: [
      "You are the optional on-device Khala Code desktop decider.",
      "Choose a small set of tool names and at most one model id from the supplied candidates.",
      "Return only compact JSON: {\"selectedToolNames\":[],\"selectedModelId\":null,\"confidence\":0,\"reasonRefs\":[]}.",
      "Do not claim coding-model parity. Do not include prompts, file contents, paths, URLs, or secrets.",
    ].join(" "),
  },
  {
    role: "user",
    content: JSON.stringify({
      maxToolSelections: Math.max(0, Math.trunc(input.maxToolSelections ?? 3)),
      modelCandidates: input.modelCandidates,
      taskSummary: input.taskSummary.slice(0, 2_000),
      toolCandidates: input.toolCandidates,
    }),
  },
]

const postLocalChatCompletion = async (input: {
  readonly baseUrl: string
  readonly fetchFn: typeof fetch
  readonly maxTokens: number
  readonly messages: readonly { readonly content: string; readonly role: "system" | "user" }[]
  readonly model: string
}): Promise<unknown> => {
  const response = await input.fetchFn(endpointUrl(input.baseUrl, "/v1/chat/completions"), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      max_tokens: input.maxTokens,
      messages: input.messages,
      model: input.model,
      temperature: 0,
    }),
    signal: AbortSignal.timeout(2_500),
  })
  const payload = await response.json()
  if (!response.ok) {
    throw new Error(`local decider returned HTTP ${response.status}`)
  }
  return payload
}

const appleFmHealthReady = async (input: {
  readonly baseUrl: string
  readonly fetchFn: typeof fetch
}): Promise<boolean> => {
  try {
    const response = await input.fetchFn(endpointUrl(input.baseUrl, "/health"), {
      method: "GET",
      signal: AbortSignal.timeout(1_500),
    })
    if (!response.ok) return false
    const payload = await response.json() as { ready?: unknown }
    return payload.ready === true
  } catch {
    return false
  }
}

const createAppleFmBackend = (options: {
  readonly env: Readonly<Record<string, string | undefined>>
  readonly fetchFn: typeof fetch
  readonly helper: DiscoveredAppleFmBridgeHelper | null
  readonly helperExecutable: boolean
  readonly mode: OnDeviceDeciderMode
  readonly now: () => string
  readonly platform: OnDeviceDeciderPlatform
  readonly spawn: typeof Bun.spawn
}): OnDeviceDeciderBackend => {
  const baseUrl =
    localBaseUrl(options.env.OPENAGENTS_DESKTOP_APPLE_FM_DECIDER_URL) ??
    localBaseUrl(options.env.OPENAGENTS_APPLE_FM_BASE_URL) ??
    OPENAGENTS_DESKTOP_APPLE_FM_DEFAULT_BASE_URL
  const model =
    trim(options.env.OPENAGENTS_DESKTOP_APPLE_FM_DECIDER_MODEL) ??
    APPLE_FM_DEFAULT_MODEL_ID
  let child: ReturnType<typeof Bun.spawn> | null = null
  let launchAttempted = false

  const startHelper = () => {
    if (
      launchAttempted ||
      child !== null ||
      options.helper === null ||
      !options.helperExecutable
    ) {
      return
    }
    launchAttempted = true
    try {
      child = options.spawn(
        [options.helper.path, "--port", String(APPLE_FM_BRIDGE_DEFAULT_PORT)],
        {
          stderr: "ignore",
          stdin: "ignore",
          stdout: "ignore",
        },
      )
      void child.exited.then(() => {
        child = null
      })
    } catch {
      child = null
    }
  }

  const status = async (): Promise<OnDeviceDeciderStatus> => {
    const observedAt = options.now()
    if (!appleFmPreferredOnPlatform(options.platform)) {
      return buildOnDeviceDeciderStatus({
        available: false,
        backendKind: APPLE_FM_BACKEND_KIND,
        blockerRefs: [
          "blocker.openagents_desktop.on_device_decider.apple_fm.unsupported_platform",
        ],
        mode: options.mode,
        model,
        observedAt,
        platform: options.platform,
        state: "not_supported",
      })
    }

    startHelper()
    const ready = await appleFmHealthReady({
      baseUrl,
      fetchFn: options.fetchFn,
    })
    if (ready) {
      return buildOnDeviceDeciderStatus({
        available: true,
        backendKind: APPLE_FM_BACKEND_KIND,
        mode: options.mode,
        model,
        observedAt,
        platform: options.platform,
        state: "ready",
      })
    }

    return buildOnDeviceDeciderStatus({
      available: false,
      backendKind: APPLE_FM_BACKEND_KIND,
      blockerRefs: [
        options.helper === null
          ? "blocker.openagents_desktop.on_device_decider.apple_fm.helper_missing"
          : !options.helperExecutable
            ? "blocker.openagents_desktop.on_device_decider.apple_fm.helper_not_executable"
            : "blocker.openagents_desktop.on_device_decider.apple_fm.bridge_unavailable",
      ],
      mode: options.mode,
      model,
      observedAt,
      platform: options.platform,
      state: options.helper === null ? "unconfigured" : "unavailable",
    })
  }

  return {
    kind: APPLE_FM_BACKEND_KIND,
    async decide(request) {
      const payload = await postLocalChatCompletion({
        baseUrl,
        fetchFn: options.fetchFn,
        maxTokens: 256,
        messages: promptFor(request),
        model,
      })
      const text = extractAssistantText(payload)
      if (text === null) throw new Error("Apple FM decider response was empty")
      return decisionFromBackendJson({
        backendKind: APPLE_FM_BACKEND_KIND,
        modelCandidates: request.modelCandidates,
        observedAt: options.now(),
        raw: extractJson(text),
        toolCandidates: request.toolCandidates,
      })
    },
    status,
  }
}

const createGptOssBackend = (options: {
  readonly env: Readonly<Record<string, string | undefined>>
  readonly fetchFn: typeof fetch
  readonly mode: OnDeviceDeciderMode
  readonly now: () => string
  readonly platform: OnDeviceDeciderPlatform
}): OnDeviceDeciderBackend => {
  const baseUrl = localBaseUrl(options.env.OPENAGENTS_DESKTOP_GPT_OSS_DECIDER_URL)
  const model =
    trim(options.env.OPENAGENTS_DESKTOP_GPT_OSS_DECIDER_MODEL) ??
    GPT_OSS_DEFAULT_MODEL_ID

  const status = async (): Promise<OnDeviceDeciderStatus> => {
    const observedAt = options.now()
    if (baseUrl === null) {
      return buildOnDeviceDeciderStatus({
        available: false,
        backendKind: GPT_OSS_BACKEND_KIND,
        blockerRefs: [
          "blocker.openagents_desktop.on_device_decider.gpt_oss.local_endpoint_missing",
        ],
        mode: options.mode,
        model,
        observedAt,
        platform: options.platform,
        state: "unconfigured",
      })
    }

    return buildOnDeviceDeciderStatus({
      available: true,
      backendKind: GPT_OSS_BACKEND_KIND,
      mode: options.mode,
      model,
      observedAt,
      platform: options.platform,
      state: "ready",
    })
  }

  return {
    kind: GPT_OSS_BACKEND_KIND,
    async decide(request) {
      if (baseUrl === null) {
        throw new Error("GPT-OSS local decider endpoint is not configured")
      }
      const payload = await postLocalChatCompletion({
        baseUrl,
        fetchFn: options.fetchFn,
        maxTokens: 256,
        messages: promptFor(request),
        model,
      })
      const text = extractAssistantText(payload)
      if (text === null) throw new Error("GPT-OSS decider response was empty")
      return decisionFromBackendJson({
        backendKind: GPT_OSS_BACKEND_KIND,
        modelCandidates: request.modelCandidates,
        observedAt: options.now(),
        raw: extractJson(text),
        toolCandidates: request.toolCandidates,
      })
    },
    status,
  }
}

export const createOnDeviceDeciderService = (
  options: OnDeviceDeciderServiceOptions = {},
): OnDeviceDeciderService => {
  const env = options.env ?? Bun.env
  const platform = {
    arch: options.arch ?? process.arch,
    platform: options.platform ?? process.platform,
  }
  const config = parseOnDeviceDeciderConfig(env)
  const now = options.now ?? (() => new Date().toISOString())
  const fetchFn = options.fetchFn ?? fetch
  const spawn = options.spawn ?? Bun.spawn
  const cwd = options.cwd ?? process.cwd()
  const resourcesDir =
    options.resourcesDir ??
    (process as NodeJS.Process & { resourcesPath?: string }).resourcesPath
  const helper = discoverAppleFmBridgeHelper({
    cwd,
    env,
    ...(resourcesDir === undefined ? {} : { resourcesDir }),
  })
  const helperIsExecutable = helper === null ? false : helperExecutable(helper.path)

  let cachedBackend: OnDeviceDeciderBackend | null | undefined
  const backend = (): OnDeviceDeciderBackend | null => {
    if (cachedBackend !== undefined) return cachedBackend
    const backendKind = selectOnDeviceDeciderBackend(config.mode, platform)
    if (backendKind === null) {
      cachedBackend = null
      return cachedBackend
    }
    if (backendKind === APPLE_FM_BACKEND_KIND) {
      cachedBackend = createAppleFmBackend({
        env,
        fetchFn,
        helper,
        helperExecutable: helperIsExecutable,
        mode: config.mode,
        now,
        platform,
        spawn,
      })
      return cachedBackend
    }
    cachedBackend = createGptOssBackend({
      env,
      fetchFn,
      mode: config.mode,
      now,
      platform,
    })
    return cachedBackend
  }

  const status = async (): Promise<OnDeviceDeciderStatus> => {
    const selected = backend()
    if (selected === null) {
      return disabledOnDeviceDeciderStatus({
        observedAt: now(),
        platform,
      })
    }
    return selected.status()
  }

  return {
    async decide(input) {
      let request: OnDeviceDeciderRequest
      try {
        request = decodeOnDeviceDeciderRequest(input)
      } catch (error) {
        const currentStatus = await status()
        return {
          ok: false,
          blockerRefs: [
            "blocker.openagents_desktop.on_device_decider.invalid_request",
          ],
          error: compactError(error),
          observedAt: now(),
          status: currentStatus,
        }
      }

      const selected = backend()
      if (selected === null) {
        const disabled = disabledOnDeviceDeciderStatus({
          observedAt: now(),
          platform,
        })
        return {
          ok: false,
          blockerRefs: disabled.blockerRefs,
          error: "on-device decider is disabled",
          observedAt: disabled.observedAt,
          status: disabled,
        }
      }

      const currentStatus = await selected.status()
      if (!currentStatus.available) {
        return {
          ok: false,
          blockerRefs: currentStatus.blockerRefs,
          error: "on-device decider is unavailable",
          observedAt: currentStatus.observedAt,
          status: currentStatus,
        }
      }

      try {
        return {
          ok: true,
          decision: await selected.decide(request),
          status: currentStatus,
        }
      } catch (error) {
        return {
          ok: false,
          blockerRefs: [
            `blocker.openagents_desktop.on_device_decider.${selected.kind}.decision_failed`,
          ],
          error: compactError(error),
          observedAt: now(),
          status: currentStatus,
        }
      }
    },
    status,
  }
}
