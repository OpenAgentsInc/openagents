import { evaluateArtanisLaborBudgetGate } from './labor-escrow'

// Artanis requester surface for the labor market (#4731). The mind proposes,
// schemas and budgets gate, and side effects are injected so the default
// scheduler path can stay off until an operator enables it.

export type ArtanisLaborRequestProposal = Readonly<{
  budgetSats: number
  deadlineRef: string
  objectiveRef: string
  repositoryRefs: ReadonlyArray<string>
  requiredCapabilityRefs: ReadonlyArray<string>
  title: string
  verificationCommandRef: string
}>

export type ArtanisLaborWorkRequestReceipt = Readonly<{
  jobEventId: string
  topicId: string
  workRequestId: string
}>

export type ArtanisLaborResultDelivery = Readonly<{
  acceptanceEventRef: string
  providerActorRef: string
  resultRef: string
  verificationCommandRef: string
  workRequestId: string
}>

export type ArtanisLaborRequesterOutcome =
  | Readonly<{ kind: 'skipped'; reason: 'config_disabled' }>
  | Readonly<{ kind: 'refused'; reason: string; refusalRef: string }>
  | Readonly<{
      kind: 'requested'
      budgetMsat: number
      receipt: ArtanisLaborWorkRequestReceipt
      reserveReceiptRef: string
    }>

export type ArtanisLaborAcceptanceOutcome =
  | Readonly<{ kind: 'accepted'; releaseReceiptRef: string }>
  | Readonly<{ kind: 'rejected_refunded'; refundReceiptRef: string; reasonRef: string }>

export type ArtanisLaborRequesterDeps = Readonly<{
  alreadyReservedThisTickMsat: number
  artanisActorRef: string
  enabled: boolean
  nowIso: string
  perTickBudgetMsat: number
  propose: () => Promise<ArtanisLaborRequestProposal>
  recordTickReceipt: (input: Readonly<{
    kind: 'request_labor_proposed' | 'request_labor_refused'
    receiptRef: string
    refs: ReadonlyArray<string>
  }>) => Promise<void>
  reserveEscrow: (input: Readonly<{
    amountMsat: number
    jobEventId: string
    requesterActorRef: string
    workRequestId: string
  }>) => Promise<{ ok: true; reserveReceiptRef: string } | { ok: false; reason: string }>
  seedBalanceAvailableMsat: number
  submitWorkRequest: (
    proposal: ArtanisLaborRequestProposal,
  ) => Promise<ArtanisLaborWorkRequestReceipt>
}>

export type ArtanisLaborAcceptanceDeps = Readonly<{
  recordTickReceipt: (input: Readonly<{
    kind: 'request_labor_accepted' | 'request_labor_rejected'
    receiptRef: string
    refs: ReadonlyArray<string>
  }>) => Promise<void>
  refundEscrow: (input: Readonly<{
    reasonRef: string
    workRequestId: string
  }>) => Promise<{ ok: true; refundReceiptRef: string }>
  releaseEscrow: (input: Readonly<{
    acceptanceEventRef: string
    providerActorRef: string
    workRequestId: string
  }>) => Promise<{ ok: true; releaseReceiptRef: string }>
  validateResult: (
    delivery: ArtanisLaborResultDelivery,
  ) => Promise<{ passed: true; verifierRef: string } | { passed: false; reasonRef: string }>
}>

const unsafeArtanisLaborPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

class ArtanisLaborValidationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ArtanisLaborValidationError'
  }
}

export const assertArtanisLaborPublicSafe = (value: unknown): void => {
  if (unsafeArtanisLaborPattern.test(JSON.stringify(value) ?? '')) {
    throw new ArtanisLaborValidationError(
      'Artanis labor request contains private or payment material.',
    )
  }
}

export const validateArtanisLaborProposal = (
  proposal: ArtanisLaborRequestProposal,
): ArtanisLaborRequestProposal => {
  if (!Number.isInteger(proposal.budgetSats) || proposal.budgetSats <= 0) {
    throw new ArtanisLaborValidationError(
      'Artanis labor budget must be positive sats.',
    )
  }
  if (proposal.title.trim().length < 3 || proposal.title.length > 160) {
    throw new ArtanisLaborValidationError(
      'Artanis labor title must be 3-160 characters.',
    )
  }
  if (
    proposal.repositoryRefs.length === 0 ||
    proposal.requiredCapabilityRefs.length === 0
  ) {
    throw new ArtanisLaborValidationError(
      'Artanis labor request requires repository and capability refs.',
    )
  }
  assertArtanisLaborPublicSafe(proposal)
  return proposal
}

export const runArtanisLaborRequestTick = async (
  deps: ArtanisLaborRequesterDeps,
): Promise<ArtanisLaborRequesterOutcome> => {
  if (!deps.enabled) {
    return { kind: 'skipped', reason: 'config_disabled' }
  }

  let proposal: ArtanisLaborRequestProposal
  try {
    proposal = validateArtanisLaborProposal(await deps.propose())
  } catch {
    const refusalRef = 'refusal.artanis_labor_request.schema_invalid'
    await deps.recordTickReceipt({
      kind: 'request_labor_refused',
      receiptRef: 'receipt.artanis_labor_request.refused.schema_invalid',
      refs: [refusalRef],
    })
    return {
      kind: 'refused',
      reason: 'schema_invalid',
      refusalRef,
    }
  }

  const budgetMsat = proposal.budgetSats * 1000
  const budgetGate = evaluateArtanisLaborBudgetGate({
    alreadyReservedThisTickMsat: deps.alreadyReservedThisTickMsat,
    perTickBudgetMsat: deps.perTickBudgetMsat,
    requestedAmountMsat: budgetMsat,
    seededBalanceAvailableMsat: deps.seedBalanceAvailableMsat,
  })

  if (budgetGate.kind === 'refused') {
    await deps.recordTickReceipt({
      kind: 'request_labor_refused',
      receiptRef: `receipt.artanis_labor_request.refused.${budgetGate.reason}`,
      refs: [budgetGate.refusalRef],
    })
    return {
      kind: 'refused',
      reason: budgetGate.reason,
      refusalRef: budgetGate.refusalRef,
    }
  }

  const receipt = await deps.submitWorkRequest(proposal)
  const reserved = await deps.reserveEscrow({
    amountMsat: budgetMsat,
    jobEventId: receipt.jobEventId,
    requesterActorRef: deps.artanisActorRef,
    workRequestId: receipt.workRequestId,
  })

  if (!reserved.ok) {
    const refusalRef = `refusal.artanis_labor_request.${reserved.reason}`
    await deps.recordTickReceipt({
      kind: 'request_labor_refused',
      receiptRef: `receipt.artanis_labor_request.refused.${reserved.reason}`,
      refs: [refusalRef, `work_request.public.${receipt.workRequestId}`],
    })
    return {
      kind: 'refused',
      reason: reserved.reason,
      refusalRef,
    }
  }

  await deps.recordTickReceipt({
    kind: 'request_labor_proposed',
    receiptRef: 'receipt.artanis_labor_request.proposed',
    refs: [
      `work_request.public.${receipt.workRequestId}`,
      `nostr.event.${receipt.jobEventId}`,
      reserved.reserveReceiptRef,
    ],
  })

  return {
    budgetMsat,
    kind: 'requested',
    receipt,
    reserveReceiptRef: reserved.reserveReceiptRef,
  }
}

export const handleArtanisLaborResultDelivery = async (
  delivery: ArtanisLaborResultDelivery,
  deps: ArtanisLaborAcceptanceDeps,
): Promise<ArtanisLaborAcceptanceOutcome> => {
  assertArtanisLaborPublicSafe(delivery)
  const verdict = await deps.validateResult(delivery)

  if (verdict.passed) {
    const released = await deps.releaseEscrow({
      acceptanceEventRef: delivery.acceptanceEventRef,
      providerActorRef: delivery.providerActorRef,
      workRequestId: delivery.workRequestId,
    })
    await deps.recordTickReceipt({
      kind: 'request_labor_accepted',
      receiptRef: released.releaseReceiptRef,
      refs: [delivery.resultRef, verdict.verifierRef, released.releaseReceiptRef],
    })
    return { kind: 'accepted', releaseReceiptRef: released.releaseReceiptRef }
  }

  const refunded = await deps.refundEscrow({
    reasonRef: verdict.reasonRef,
    workRequestId: delivery.workRequestId,
  })
  await deps.recordTickReceipt({
    kind: 'request_labor_rejected',
    receiptRef: refunded.refundReceiptRef,
    refs: [delivery.resultRef, verdict.reasonRef, refunded.refundReceiptRef],
  })
  return {
    kind: 'rejected_refunded',
    reasonRef: verdict.reasonRef,
    refundReceiptRef: refunded.refundReceiptRef,
  }
}
