export type PromiseSurfacingSuggestedState =
  | "green"
  | "yellow"
  | "red"
  | "degraded"
  | "planned"
  | "unknown"

export type PromiseSurfacingInput = {
  readonly promiseId: string
  readonly surface: string
  readonly claimText: string
  readonly expectedBehavior: string
  readonly observedBehavior: string
  readonly evidenceOrSteps: string
  readonly environment: string
  readonly impact: string
  readonly suggestedState: PromiseSurfacingSuggestedState
}

export type ProductPromiseLedgerRecord = {
  readonly promiseId: string
  readonly state: string
  readonly safeCopy?: string
  readonly unsafeCopy?: string
  readonly evidenceRefs?: readonly string[]
  readonly blockerRefs?: readonly string[]
}

export type ProductPromiseLedgerDocument = {
  readonly registryVersion?: string
  readonly version?: string
  readonly generatedAt?: string
  readonly reportPath?: {
    readonly forumSlug?: string
    readonly forumTopicApi?: string
  }
  readonly promises?: readonly ProductPromiseLedgerRecord[]
}

export type ProductPromiseForumTopicSummary = {
  readonly topicId: string
  readonly title: string
  readonly url: string | null
}

export type PromiseSurfacingLedgerVerdict =
  | "promise_missing_from_ledger"
  | "ledger_claims_fixed_report_new_mismatch"
  | "ledger_already_scopes_or_blocks_claim"
  | "ledger_withdrawn_or_planned"

export type PromiseSurfacingDraft = {
  readonly title: string
  readonly requestedSlug: string
  readonly bodyText: string
  readonly ledgerVerdict: PromiseSurfacingLedgerVerdict
  readonly registryVersion: string
  readonly promiseState: string | null
  readonly relatedTopicRefs: readonly string[]
}

export type PromiseSurfacingValidation = {
  readonly ok: boolean
  readonly errors: readonly string[]
  readonly input: PromiseSurfacingInput
}

const trim = (value: string): string => value.trim()

const safeSlug = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80) || "promise-report"

const normalizeSuggestedState = (
  value: PromiseSurfacingSuggestedState,
): PromiseSurfacingSuggestedState => {
  switch (value) {
    case "green":
    case "yellow":
    case "red":
    case "degraded":
    case "planned":
    case "unknown":
      return value
  }
}

export const validatePromiseSurfacingInput = (
  input: PromiseSurfacingInput,
): PromiseSurfacingValidation => {
  const normalized = {
    promiseId: trim(input.promiseId),
    surface: trim(input.surface),
    claimText: trim(input.claimText),
    expectedBehavior: trim(input.expectedBehavior),
    observedBehavior: trim(input.observedBehavior),
    evidenceOrSteps: trim(input.evidenceOrSteps),
    environment: trim(input.environment),
    impact: trim(input.impact),
    suggestedState: normalizeSuggestedState(input.suggestedState),
  }
  const required: ReadonlyArray<readonly [keyof PromiseSurfacingInput, string]> = [
    ["promiseId", "promiseId is required"],
    ["surface", "surface is required"],
    ["claimText", "claimText is required"],
    ["expectedBehavior", "expectedBehavior is required"],
    ["observedBehavior", "observedBehavior is required"],
    ["evidenceOrSteps", "evidenceOrSteps is required"],
    ["environment", "environment is required"],
    ["impact", "impact is required"],
  ]
  const errors = required.flatMap(([field, message]) =>
    normalized[field].length === 0 ? [message] : [],
  )
  return { ok: errors.length === 0, errors, input: normalized }
}

const verdictForPromise = (
  promise: ProductPromiseLedgerRecord | null,
): PromiseSurfacingLedgerVerdict => {
  if (promise === null) return "promise_missing_from_ledger"
  switch (promise.state) {
    case "green":
      return "ledger_claims_fixed_report_new_mismatch"
    case "planned":
    case "withdrawn":
      return "ledger_withdrawn_or_planned"
    default:
      return "ledger_already_scopes_or_blocks_claim"
  }
}

const yamlLine = (label: string, value: string | boolean): string =>
  `${label}: ${typeof value === "boolean" ? value : JSON.stringify(value)}`

export const buildPromiseSurfacingDraft = (input: {
  readonly report: PromiseSurfacingInput
  readonly ledger: ProductPromiseLedgerDocument
  readonly relatedTopics: readonly ProductPromiseForumTopicSummary[]
  readonly observedAt: string
}): PromiseSurfacingDraft => {
  const validation = validatePromiseSurfacingInput(input.report)
  if (!validation.ok) {
    throw new Error(validation.errors[0] ?? "invalid promise report")
  }
  const report = validation.input
  const promise =
    input.ledger.promises?.find(item => item.promiseId === report.promiseId) ??
    null
  const registryVersion =
    input.ledger.registryVersion ?? input.ledger.version ?? "unknown"
  const ledgerVerdict = verdictForPromise(promise)
  const relatedTopicRefs = input.relatedTopics.map(topic =>
    topic.url ?? topic.topicId,
  )
  const title = `[Promise Report] ${report.promiseId}`
  const requestedSlug = `promise-report-${safeSlug(report.promiseId)}`
  const blockerRefs = promise?.blockerRefs ?? []
  const evidenceRefs = promise?.evidenceRefs ?? []

  const bodyText = [
    "# Product Promise Report",
    "",
    yamlLine("promiseId", report.promiseId),
    yamlLine("surface", report.surface),
    yamlLine("claimText", report.claimText),
    yamlLine("expectedBehavior", report.expectedBehavior),
    yamlLine("observedBehavior", report.observedBehavior),
    yamlLine("evidenceOrSteps", report.evidenceOrSteps),
    yamlLine("observedAt", input.observedAt),
    yamlLine("environment", report.environment),
    yamlLine("impact", report.impact),
    yamlLine("sensitiveDataRemoved", true),
    yamlLine("requestedFollowUp", "public"),
    yamlLine("suggestedState", report.suggestedState),
    "",
    "## Live Ledger Diff",
    "",
    `- registryVersion: ${registryVersion}`,
    `- ledgerGeneratedAt: ${input.ledger.generatedAt ?? "unknown"}`,
    `- ledgerState: ${promise?.state ?? "missing"}`,
    `- ledgerVerdict: ${ledgerVerdict}`,
    `- relatedExactPromiseForumTopics: ${relatedTopicRefs.length}`,
    "",
    "## Ledger Safe Copy",
    "",
    promise?.safeCopy ?? "No matching promise safeCopy was found.",
    "",
    "## Ledger Blockers",
    "",
    blockerRefs.length === 0
      ? "- none"
      : blockerRefs.map(ref => `- ${ref}`).join("\n"),
    "",
    "## Ledger Evidence",
    "",
    evidenceRefs.length === 0
      ? "- none"
      : evidenceRefs.slice(0, 20).map(ref => `- ${ref}`).join("\n"),
    "",
    "## Related Forum Topics",
    "",
    relatedTopicRefs.length === 0
      ? "- none found by exact promiseId"
      : relatedTopicRefs.map(ref => `- ${ref}`).join("\n"),
    "",
    "## Posture",
    "",
    "Surface only. Do not ship code from this report. Maintainers can triage, reproduce, patch copy/projections/product behavior, and open a strict GitHub issue only if this becomes a concrete reproducible bug.",
  ].join("\n")

  return {
    title,
    requestedSlug,
    bodyText,
    ledgerVerdict,
    registryVersion,
    promiseState: promise?.state ?? null,
    relatedTopicRefs,
  }
}
