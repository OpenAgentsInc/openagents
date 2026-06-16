export type AppleFmEnergyEvidenceState = "modeled" | "measured" | "unavailable"

type AppleFmEnergyBase = {
  evidenceState: AppleFmEnergyEvidenceState
  methodRef: string
  wallClockSeconds: number
  wallClockHours: number
  assumptionRefs: string[]
  caveatRefs: string[]
  sourceRefs: string[]
}

export type AppleFmModeledEnergyEstimate = AppleFmEnergyBase & {
  evidenceState: "modeled"
  modeledPowerKw: number
  energyKwh: number
}

export type AppleFmUnavailableEnergyEstimate = AppleFmEnergyBase & {
  evidenceState: "unavailable"
  energyKwh: null
  blockerRefs: string[]
}

export type AppleFmEnergyEstimate =
  | AppleFmModeledEnergyEstimate
  | AppleFmUnavailableEnergyEstimate

export const DEFAULT_APPLE_FM_MODELED_POWER_KW = 0.02

const DEFAULT_MODELED_METHOD_REF = "method.apple_fm.power.modeled_default_kw_wall_clock"
const CONFIGURED_MODELED_METHOD_REF = "method.apple_fm.power.modeled_configured_kw_wall_clock"
const UNAVAILABLE_METHOD_REF = "method.apple_fm.power.unavailable"

const MODELED_CAVEAT_REFS = [
  "caveat.apple_fm.power.modeled_not_measured",
  "caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome",
]

const UNAVAILABLE_CAVEAT_REFS = [
  "caveat.apple_fm.power.no_power_value_published",
  "caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome",
]

type EstimateInput = {
  env: Record<string, string | undefined>
  startedAt: string | null
  completedAt: string | null
}

export function estimateAppleFmLocalSessionEnergy(input: EstimateInput): AppleFmEnergyEstimate {
  const window = sessionWindow(input.startedAt, input.completedAt)
  const mode = powerEstimateMode(input.env)

  if (window === null) {
    return unavailableEstimate(0, 0, ["blocker.apple_fm.energy.invalid_session_window"])
  }

  if (mode === "disabled") {
    return unavailableEstimate(window.wallClockSeconds, window.wallClockHours, [
      "blocker.apple_fm.energy.estimate_disabled",
    ])
  }

  const configuredPowerKw = configuredModeledPowerKw(input.env)
  const modeledPowerKw = configuredPowerKw ?? DEFAULT_APPLE_FM_MODELED_POWER_KW
  const methodRef = configuredPowerKw === undefined
    ? DEFAULT_MODELED_METHOD_REF
    : CONFIGURED_MODELED_METHOD_REF
  const assumptionRefs = [
    configuredPowerKw === undefined
      ? "assumption.apple_fm.power.default_20w_modeled_load"
      : "assumption.apple_fm.power.operator_configured_modeled_load",
    "assumption.apple_fm.power.session_wall_clock_window",
  ]

  return {
    evidenceState: "modeled",
    methodRef,
    modeledPowerKw: round(modeledPowerKw, 6),
    wallClockSeconds: window.wallClockSeconds,
    wallClockHours: window.wallClockHours,
    energyKwh: round(modeledPowerKw * window.wallClockHours, 9),
    assumptionRefs,
    caveatRefs: MODELED_CAVEAT_REFS,
    sourceRefs: [
      "source.apple_fm.control_session.retained_window",
      configuredPowerKw === undefined
        ? "source.apple_fm.power.default_model"
        : "source.apple_fm.power.operator_configured_model",
    ],
  }
}

function sessionWindow(startedAt: string | null, completedAt: string | null) {
  if (startedAt === null || completedAt === null) return null

  const startedMs = Date.parse(startedAt)
  const completedMs = Date.parse(completedAt)
  if (!Number.isFinite(startedMs) || !Number.isFinite(completedMs) || completedMs < startedMs) {
    return null
  }

  const wallClockSeconds = round((completedMs - startedMs) / 1000, 3)
  return {
    wallClockSeconds,
    wallClockHours: round(wallClockSeconds / 3600, 9),
  }
}

function powerEstimateMode(env: Record<string, string | undefined>) {
  const value =
    env.OPENAGENTS_APPLE_FM_POWER_ESTIMATE_MODE ??
    env.PROBE_APPLE_FM_POWER_ESTIMATE_MODE ??
    ""
  return value.trim().toLowerCase() === "disabled" ? "disabled" : "modeled"
}

function configuredModeledPowerKw(env: Record<string, string | undefined>) {
  const value =
    env.OPENAGENTS_APPLE_FM_MODELED_POWER_KW ??
    env.PROBE_APPLE_FM_MODELED_POWER_KW
  if (value === undefined || value.trim() === "") return undefined

  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed <= 0) return undefined

  return parsed
}

function unavailableEstimate(
  wallClockSeconds: number,
  wallClockHours: number,
  blockerRefs: string[],
): AppleFmUnavailableEnergyEstimate {
  return {
    evidenceState: "unavailable",
    methodRef: UNAVAILABLE_METHOD_REF,
    wallClockSeconds,
    wallClockHours,
    energyKwh: null,
    assumptionRefs: [],
    caveatRefs: UNAVAILABLE_CAVEAT_REFS,
    sourceRefs: ["source.apple_fm.control_session.retained_window"],
    blockerRefs,
  }
}

function round(value: number, digits: number) {
  return Number(value.toFixed(digits))
}
