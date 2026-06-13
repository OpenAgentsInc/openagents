export type BuildSubmitOutcome = "built" | "submitted" | "failed"

export type BuildSubmitReceiptInput = {
  ipaRef: string
  buildVersion: string
  submissionId: string | null
  outcome: BuildSubmitOutcome
  observedAt: string
  originIntentRef?: string
}

export type BuildSubmitReceipt = {
  readonly ipaRef: string
  readonly buildVersion: string
  readonly submissionId: string | null
  readonly outcome: BuildSubmitOutcome
  readonly observedAt: string
  readonly originIntentRef?: string
}

const outcomes = new Set<BuildSubmitOutcome>(["built", "submitted", "failed"])

export function buildSubmitReceipt(input: BuildSubmitReceiptInput): BuildSubmitReceipt {
  const originIntentRef = input.originIntentRef?.trim()
  const receipt: BuildSubmitReceipt = {
    ipaRef: input.ipaRef.trim(),
    buildVersion: input.buildVersion.trim(),
    submissionId: input.submissionId === null ? null : input.submissionId.trim(),
    outcome: input.outcome,
    observedAt: input.observedAt.trim(),
  }

  return originIntentRef ? { ...receipt, originIntentRef } : receipt
}

export function validateBuildSubmitReceipt(receipt: unknown): receipt is BuildSubmitReceipt {
  if (typeof receipt !== "object" || receipt === null) {
    return false
  }

  const candidate = receipt as Partial<BuildSubmitReceipt>

  return (
    isNonEmptyString(candidate.ipaRef) &&
    isNonEmptyString(candidate.buildVersion) &&
    isValidSubmissionId(candidate.submissionId) &&
    isValidOutcome(candidate.outcome) &&
    isNonEmptyString(candidate.observedAt) &&
    (candidate.originIntentRef === undefined || isNonEmptyString(candidate.originIntentRef))
  )
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

function isValidSubmissionId(value: unknown): value is string | null {
  return value === null || isNonEmptyString(value)
}

function isValidOutcome(value: unknown): value is BuildSubmitOutcome {
  return typeof value === "string" && outcomes.has(value as BuildSubmitOutcome)
}
