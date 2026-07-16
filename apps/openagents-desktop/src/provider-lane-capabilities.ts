/**
 * Provider-lane capability truth (L2 #8900).
 *
 * The SPI reports what a lane observed. This module intersects that report
 * with the lane's static/native declaration or admitted ACP peer-profile
 * evidence before any value can become a composer affordance. A report that
 * over-claims is quarantined as a whole: hiding only the disputed control
 * would leave the rest of an untrustworthy lane actionable.
 */

export const ProviderLaneCapabilitiesChannel = "openagents:provider-lanes:capabilities" as const

export const providerLaneFeatureKeys = [
  "skills",
  "planOnly",
  "reasoningEffort",
  "images",
  "fullAuto",
  "interrupt",
  "queueFollowup",
  "steerTurn",
  "steerChild",
  "answerQuestion",
] as const

export type ProviderLaneFeatureKey = (typeof providerLaneFeatureKeys)[number]

export type ProviderLaneCapabilityPolicy = Readonly<{
  source: "native-static-declaration" | "trusted-acp-peer-profile"
  profileRef: string
  evidence: "conformant" | "experimental"
  allowedModels: ReadonlyArray<string>
  allowedFeatures: ReadonlyArray<ProviderLaneFeatureKey>
  allowedExtensions: ReadonlyArray<string>
}>

export type ProviderLaneComposerCapabilities = Readonly<{
  displayName: string
  reasoningEfforts: ReadonlyArray<string>
  permissionModes: ReadonlyArray<"owner_full" | "plan_only">
  approvals: "provider_native" | "host_mediated" | "none"
  extensions: ReadonlyArray<string>
}>

export type ProviderLaneCapabilityReport = Readonly<{
  laneRef: string
  provider: string
  models: ReadonlyArray<string>
  features: Readonly<Record<ProviderLaneFeatureKey, boolean>>
  recovery: "provider_session_replay" | "interrupt_on_restart"
  composer: ProviderLaneComposerCapabilities
  policy: ProviderLaneCapabilityPolicy
}>

export type ProviderLaneComposerProjection = Readonly<{
  laneRef: string
  provider: string
  displayName: string
  admission: "admitted" | "quarantined"
  reason: string | null
  models: ReadonlyArray<string>
  reasoningEfforts: ReadonlyArray<string>
  permissionModes: ReadonlyArray<"owner_full" | "plan_only">
  approvals: "provider_native" | "host_mediated" | "none"
  questions: boolean
  skills: boolean
  images: boolean
  fullAuto: boolean
  interrupt: boolean
  queueFollowup: boolean
  steerTurn: boolean
  extensions: ReadonlyArray<string>
  evidence: "conformant" | "experimental"
}>

const uniqueStrings = (values: ReadonlyArray<string>): boolean =>
  values.every((value, index) => value.length > 0 && values.indexOf(value) === index)

export const projectProviderLaneCapabilities = (
  report: ProviderLaneCapabilityReport,
): ProviderLaneComposerProjection => {
  const allowedFeatures = new Set(report.policy.allowedFeatures)
  const overclaimedFeatures = providerLaneFeatureKeys.filter(
    feature => report.features[feature] && !allowedFeatures.has(feature),
  )
  const allowedModels = new Set(report.policy.allowedModels)
  const overclaimedModels = report.models.filter(model => !allowedModels.has(model))
  const allowedExtensions = new Set(report.policy.allowedExtensions)
  const overclaimedExtensions = report.composer.extensions.filter(
    extension => !allowedExtensions.has(extension),
  )
  const malformed = report.laneRef.length === 0 || report.provider.length === 0 ||
    report.composer.displayName.length === 0 || report.models.length === 0 ||
    !uniqueStrings(report.models) || !uniqueStrings(report.composer.extensions) ||
    !uniqueStrings(report.composer.reasoningEfforts) ||
    (report.features.reasoningEffort && report.composer.reasoningEfforts.length === 0) ||
    (!report.features.reasoningEffort && report.composer.reasoningEfforts.length > 0) ||
    (report.features.planOnly && !report.composer.permissionModes.includes("plan_only")) ||
    (!report.features.planOnly && report.composer.permissionModes.includes("plan_only"))
  const lies = [
    ...overclaimedFeatures.map(value => `feature:${value}`),
    ...overclaimedModels.map(value => `model:${value}`),
    ...overclaimedExtensions.map(value => `extension:${value}`),
  ]
  const quarantined = malformed || lies.length > 0
  return {
    laneRef: report.laneRef,
    provider: report.provider,
    displayName: report.composer.displayName,
    admission: quarantined ? "quarantined" : "admitted",
    reason: quarantined
      ? malformed
        ? "Lane capability report is internally inconsistent."
        : `Lane capability over-claim quarantined (${lies.join(", ")}).`
      : null,
    models: quarantined ? [] : report.models,
    reasoningEfforts: quarantined ? [] : report.composer.reasoningEfforts,
    permissionModes: quarantined ? [] : report.composer.permissionModes,
    approvals: quarantined ? "none" : report.composer.approvals,
    questions: !quarantined && report.features.answerQuestion,
    skills: !quarantined && report.features.skills,
    images: !quarantined && report.features.images,
    fullAuto: !quarantined && report.features.fullAuto,
    interrupt: !quarantined && report.features.interrupt,
    queueFollowup: !quarantined && report.features.queueFollowup,
    steerTurn: !quarantined && report.features.steerTurn,
    extensions: quarantined ? [] : report.composer.extensions,
    evidence: report.policy.evidence,
  }
}

export const decodeProviderLaneComposerProjections = (
  value: unknown,
): ReadonlyArray<ProviderLaneComposerProjection> | null => {
  if (!Array.isArray(value) || value.length > 16) return null
  for (const row of value) {
    if (typeof row !== "object" || row === null) return null
    const candidate = row as Partial<ProviderLaneComposerProjection>
    if (typeof candidate.laneRef !== "string" || typeof candidate.provider !== "string" ||
      typeof candidate.displayName !== "string" ||
      (candidate.admission !== "admitted" && candidate.admission !== "quarantined") ||
      !(candidate.reason === null || typeof candidate.reason === "string") ||
      !Array.isArray(candidate.models) || !candidate.models.every(item => typeof item === "string") ||
      !Array.isArray(candidate.reasoningEfforts) || !candidate.reasoningEfforts.every(item => typeof item === "string") ||
      !Array.isArray(candidate.permissionModes) ||
      !candidate.permissionModes.every(item => item === "owner_full" || item === "plan_only") ||
      !["provider_native", "host_mediated", "none"].includes(String(candidate.approvals)) ||
      typeof candidate.questions !== "boolean" || typeof candidate.skills !== "boolean" || typeof candidate.images !== "boolean" ||
      typeof candidate.fullAuto !== "boolean" || typeof candidate.interrupt !== "boolean" ||
      typeof candidate.queueFollowup !== "boolean" || typeof candidate.steerTurn !== "boolean" ||
      !Array.isArray(candidate.extensions) || !candidate.extensions.every(item => typeof item === "string") ||
      (candidate.evidence !== "conformant" && candidate.evidence !== "experimental")) return null
  }
  return value as ReadonlyArray<ProviderLaneComposerProjection>
}
