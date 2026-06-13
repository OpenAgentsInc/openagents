export type DeliveryCheckStatus = "passed" | "failed" | "pending"

export type DeliveryRequiredCheck = {
  readonly name: string
  readonly status: DeliveryCheckStatus
}

export type DeliveryReadiness = {
  readonly repoRef: string
  readonly headRef: string
  readonly requiredChecks: Array<DeliveryRequiredCheck>
  readonly evidenceRefs: string[]
}

export type DeliveryAssessment = {
  readonly ready: boolean
  readonly blockers: string[]
}

export type DeliveryReceiptDecision = "ready" | "blocked"

export type DeliveryReceipt = {
  readonly repoRef: string
  readonly headRef: string
  readonly requiredChecks: Array<DeliveryRequiredCheck>
  readonly evidenceRefs: string[]
  readonly decision: DeliveryReceiptDecision
}

export function assessDelivery(readiness: DeliveryReadiness): DeliveryAssessment {
  const checkBlockers = readiness.requiredChecks
    .filter((check) => check.status !== "passed")
    .map((check) => `required check ${check.name} is ${check.status}`)

  const blockers =
    readiness.evidenceRefs.length > 0
      ? checkBlockers
      : [...checkBlockers, "at least one evidence ref is required"]

  return {
    ready: blockers.length === 0,
    blockers,
  }
}

export function buildDeliveryReceipt(
  readiness: DeliveryReadiness,
  options: { readonly decision: DeliveryReceiptDecision },
): DeliveryReceipt {
  return {
    repoRef: readiness.repoRef,
    headRef: readiness.headRef,
    requiredChecks: readiness.requiredChecks.map((check) => ({
      name: check.name,
      status: check.status,
    })),
    evidenceRefs: [...readiness.evidenceRefs],
    decision: options.decision,
  }
}
