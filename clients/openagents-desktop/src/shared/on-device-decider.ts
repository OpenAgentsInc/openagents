import { Schema } from "effect"

export const OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA =
  "openagents.desktop.on_device_decider.v0.1" as const

export const APPLE_FM_BACKEND_KIND = "apple_fm" as const
export const GPT_OSS_BACKEND_KIND = "gpt_oss" as const
export const APPLE_FM_DEFAULT_MODEL_ID = "apple-foundation-model" as const
export const GPT_OSS_DEFAULT_MODEL_ID = "gpt-oss:decider" as const
export const OPENAGENTS_DESKTOP_APPLE_FM_DEFAULT_BASE_URL =
  "http://127.0.0.1:11435" as const

export const OnDeviceDeciderBackendKind = Schema.Literals([
  APPLE_FM_BACKEND_KIND,
  GPT_OSS_BACKEND_KIND,
])
export type OnDeviceDeciderBackendKind =
  typeof OnDeviceDeciderBackendKind.Type

export const OnDeviceDeciderMode = Schema.Literals([
  "off",
  "auto",
  APPLE_FM_BACKEND_KIND,
  GPT_OSS_BACKEND_KIND,
])
export type OnDeviceDeciderMode = typeof OnDeviceDeciderMode.Type

export const OnDeviceDeciderState = Schema.Literals([
  "disabled",
  "not_supported",
  "unconfigured",
  "ready",
  "unavailable",
])
export type OnDeviceDeciderState = typeof OnDeviceDeciderState.Type

export const OnDeviceDeciderToolCandidate = Schema.Struct({
  description: Schema.optional(Schema.String),
  name: Schema.String,
})
export type OnDeviceDeciderToolCandidate =
  typeof OnDeviceDeciderToolCandidate.Type

export const OnDeviceDeciderModelCandidate = Schema.Struct({
  id: Schema.String,
  label: Schema.optional(Schema.String),
})
export type OnDeviceDeciderModelCandidate =
  typeof OnDeviceDeciderModelCandidate.Type

export const OnDeviceDeciderRequest = Schema.Struct({
  maxToolSelections: Schema.optional(Schema.Number),
  modelCandidates: Schema.Array(OnDeviceDeciderModelCandidate),
  taskSummary: Schema.String,
  toolCandidates: Schema.Array(OnDeviceDeciderToolCandidate),
})
export type OnDeviceDeciderRequest = typeof OnDeviceDeciderRequest.Type

export const decodeOnDeviceDeciderRequest = (
  value: unknown,
): OnDeviceDeciderRequest =>
  Schema.decodeUnknownSync(OnDeviceDeciderRequest)(value)

export const OnDeviceDeciderDecision = Schema.Struct({
  backendKind: OnDeviceDeciderBackendKind,
  confidence: Schema.Number,
  contentRedacted: Schema.Literal(true),
  kind: Schema.Literal("openagents_desktop_on_device_decider_decision"),
  mainModelParityClaim: Schema.Literal(false),
  noSpend: Schema.Literal(true),
  observedAt: Schema.String,
  reasonRefs: Schema.Array(Schema.String),
  schema: Schema.Literal(OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA),
  selectedModelId: Schema.NullOr(Schema.String),
  selectedToolNames: Schema.Array(Schema.String),
})
export type OnDeviceDeciderDecision = typeof OnDeviceDeciderDecision.Type

export const OnDeviceDeciderStatus = Schema.Struct({
  available: Schema.Boolean,
  backendKind: Schema.NullOr(OnDeviceDeciderBackendKind),
  backendLabel: Schema.String,
  blockerRefs: Schema.Array(Schema.String),
  contentRedacted: Schema.Literal(true),
  enabled: Schema.Boolean,
  kind: Schema.Literal("openagents_desktop_on_device_decider_status"),
  mainModelParityClaim: Schema.Literal(false),
  mode: OnDeviceDeciderMode,
  model: Schema.NullOr(Schema.String),
  noSpend: Schema.Literal(true),
  observedAt: Schema.String,
  platform: Schema.Struct({
    arch: Schema.String,
    platform: Schema.String,
  }),
  schema: Schema.Literal(OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA),
  state: OnDeviceDeciderState,
})
export type OnDeviceDeciderStatus = typeof OnDeviceDeciderStatus.Type

export type OnDeviceDeciderRunResult =
  | {
      readonly ok: true
      readonly decision: OnDeviceDeciderDecision
      readonly status: OnDeviceDeciderStatus
    }
  | {
      readonly ok: false
      readonly blockerRefs: readonly string[]
      readonly error: string
      readonly observedAt: string
      readonly status: OnDeviceDeciderStatus
    }

export type OnDeviceDeciderPlatform = {
  readonly arch: string
  readonly platform: string
}

export type OnDeviceDeciderConfig = {
  readonly mode: OnDeviceDeciderMode
}

export type OnDeviceDeciderBackendJsonDecision = {
  readonly confidence?: number
  readonly reasonRefs?: readonly string[]
  readonly selectedModelId?: string | null
  readonly selectedToolNames?: readonly string[]
}

const OnDeviceDeciderBackendJsonDecisionSchema = Schema.Struct({
  confidence: Schema.optional(Schema.Number),
  reasonRefs: Schema.optional(Schema.Array(Schema.String)),
  selectedModelId: Schema.optional(Schema.NullOr(Schema.String)),
  selectedToolNames: Schema.optional(Schema.Array(Schema.String)),
})

const modeAliases = new Map<string, OnDeviceDeciderMode>([
  ["1", "auto"],
  ["true", "auto"],
  ["yes", "auto"],
  ["on", "auto"],
  ["enabled", "auto"],
  ["auto", "auto"],
  ["apple", APPLE_FM_BACKEND_KIND],
  ["apple_fm", APPLE_FM_BACKEND_KIND],
  ["foundation_models", APPLE_FM_BACKEND_KIND],
  ["gpt_oss", GPT_OSS_BACKEND_KIND],
  ["gpt-oss", GPT_OSS_BACKEND_KIND],
  ["0", "off"],
  ["false", "off"],
  ["no", "off"],
  ["disabled", "off"],
  ["off", "off"],
])

export const parseOnDeviceDeciderConfig = (
  env: Readonly<Record<string, string | undefined>>,
): OnDeviceDeciderConfig => {
  const raw = env.OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER?.trim().toLowerCase()
  if (raw === undefined || raw === "") return { mode: "off" }
  const mode = modeAliases.get(raw) ?? "off"
  if (mode !== "auto") return { mode }

  const backend = env.OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_BACKEND
    ?.trim()
    .toLowerCase()
  return {
    mode: backend === undefined || backend === ""
      ? "auto"
      : modeAliases.get(backend) ?? "auto",
  }
}

export const appleFmPreferredOnPlatform = (
  platform: OnDeviceDeciderPlatform,
): boolean =>
  (platform.platform === "darwin" && platform.arch === "arm64") ||
  platform.platform === "ios"

export const selectOnDeviceDeciderBackend = (
  mode: OnDeviceDeciderMode,
  platform: OnDeviceDeciderPlatform,
): OnDeviceDeciderBackendKind | null => {
  if (mode === "off") return null
  if (mode === APPLE_FM_BACKEND_KIND || mode === GPT_OSS_BACKEND_KIND) {
    return mode
  }
  return appleFmPreferredOnPlatform(platform)
    ? APPLE_FM_BACKEND_KIND
    : GPT_OSS_BACKEND_KIND
}

export const backendLabel = (
  backendKind: OnDeviceDeciderBackendKind | null,
): string => {
  switch (backendKind) {
    case APPLE_FM_BACKEND_KIND:
      return "Apple Foundation Models"
    case GPT_OSS_BACKEND_KIND:
      return "GPT-OSS local"
    default:
      return "Off"
  }
}

export const disabledOnDeviceDeciderStatus = (input: {
  readonly observedAt: string
  readonly platform: OnDeviceDeciderPlatform
}): OnDeviceDeciderStatus => ({
  available: false,
  backendKind: null,
  backendLabel: "Off",
  blockerRefs: ["blocker.openagents_desktop.on_device_decider.disabled"],
  contentRedacted: true,
  enabled: false,
  kind: "openagents_desktop_on_device_decider_status",
  mainModelParityClaim: false,
  mode: "off",
  model: null,
  noSpend: true,
  observedAt: input.observedAt,
  platform: input.platform,
  schema: OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA,
  state: "disabled",
})

export const buildOnDeviceDeciderStatus = (input: {
  readonly available: boolean
  readonly backendKind: OnDeviceDeciderBackendKind
  readonly blockerRefs?: readonly string[]
  readonly mode: OnDeviceDeciderMode
  readonly model: string | null
  readonly observedAt: string
  readonly platform: OnDeviceDeciderPlatform
  readonly state: Exclude<OnDeviceDeciderState, "disabled">
}): OnDeviceDeciderStatus => ({
  available: input.available,
  backendKind: input.backendKind,
  backendLabel: backendLabel(input.backendKind),
  blockerRefs: [...(input.blockerRefs ?? [])].sort(),
  contentRedacted: true,
  enabled: true,
  kind: "openagents_desktop_on_device_decider_status",
  mainModelParityClaim: false,
  mode: input.mode,
  model: input.model,
  noSpend: true,
  observedAt: input.observedAt,
  platform: input.platform,
  schema: OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA,
  state: input.state,
})

const publicRef = (value: string): string | null =>
  /^[a-z][a-z0-9_.:-]+$/i.test(value) ? value : null

const clampConfidence = (value: number | undefined): number =>
  Math.max(0, Math.min(1, Number.isFinite(value ?? NaN) ? value ?? 0 : 0))

export const decisionFromBackendJson = (input: {
  readonly backendKind: OnDeviceDeciderBackendKind
  readonly modelCandidates: readonly OnDeviceDeciderModelCandidate[]
  readonly observedAt: string
  readonly raw: unknown
  readonly toolCandidates: readonly OnDeviceDeciderToolCandidate[]
}): OnDeviceDeciderDecision => {
  const decoded = Schema.decodeUnknownSync(
    OnDeviceDeciderBackendJsonDecisionSchema,
  )(input.raw)
  const toolNames = new Set(input.toolCandidates.map(tool => tool.name))
  const modelIds = new Set(input.modelCandidates.map(model => model.id))
  const selectedToolNames = [...new Set(decoded.selectedToolNames ?? [])]
    .filter(name => toolNames.has(name))
    .slice(0, Math.max(0, input.toolCandidates.length))
  const selectedModelId =
    decoded.selectedModelId !== undefined &&
    decoded.selectedModelId !== null &&
    modelIds.has(decoded.selectedModelId)
      ? decoded.selectedModelId
      : null
  const reasonRefs = (decoded.reasonRefs ?? [])
    .map(publicRef)
    .filter((ref): ref is string => ref !== null)

  return {
    backendKind: input.backendKind,
    confidence: clampConfidence(decoded.confidence),
    contentRedacted: true,
    kind: "openagents_desktop_on_device_decider_decision",
    mainModelParityClaim: false,
    noSpend: true,
    observedAt: input.observedAt,
    reasonRefs: reasonRefs.length === 0
      ? ["on_device_decider.reason.model_json"]
      : reasonRefs,
    schema: OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER_SCHEMA,
    selectedModelId,
    selectedToolNames,
  }
}
