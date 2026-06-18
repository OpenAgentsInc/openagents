import {
  DefaultForumWorkRequestCapabilityRef,
  DefaultForumWorkRequestRepositoryRef,
  ForumWorkRequestUnsafe,
  type ForumWorkRequestInput,
  type NormalizedForumWorkRequestInput,
  normalizeForumWorkRequestInput,
} from './forum-work-requests'
import {
  type DebtReceiptSettlementInput,
  type DebtReceiptSettlementProjection,
  projectDebtReceiptSettlement,
} from './debt-receipt-policy'

export type DebtReceiptWorkRequestFilingInput =
  DebtReceiptSettlementInput &
    Readonly<{
      deadlineRef: string
      debtReceiptRef: string
      repositoryRefs?: ReadonlyArray<string> | undefined
      requiredCapabilityRefs?: ReadonlyArray<string> | undefined
      title: string
    }>

export type DebtReceiptWorkRequestFiling = Readonly<{
  debtReceiptProjection: DebtReceiptSettlementProjection
  debtReceiptRef: string
  idempotencyKey: string
  input: ForumWorkRequestInput
  normalizedInput: NormalizedForumWorkRequestInput
}>

export class DebtReceiptWorkRequestUnsafe extends Error {
  override readonly name = 'DebtReceiptWorkRequestUnsafe'
}

const DebtReceiptRefPattern = /^receipt\.public\.debt\.[A-Za-z0-9][A-Za-z0-9_.-]*$/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

export const debtReceiptWorkRequestIdempotencyKey = (
  debtReceiptRef: string,
): string =>
  `debt-receipt:${debtReceiptRef
    .replace(/[^A-Za-z0-9_-]+/gu, '_')
    .toLowerCase()}`

const singleVerificationCommandRef = (
  refs: ReadonlyArray<string> | undefined,
): string => {
  const verificationCommandRefs = uniqueRefs(refs)

  if (verificationCommandRefs.length !== 1) {
    throw new DebtReceiptWorkRequestUnsafe(
      'A debt-receipt work request must carry exactly one verification command ref.',
    )
  }

  return verificationCommandRefs[0]!
}

export const buildDebtReceiptWorkRequestFiling = (
  input: DebtReceiptWorkRequestFilingInput,
): DebtReceiptWorkRequestFiling => {
  const debtReceiptRef = input.debtReceiptRef.trim()

  if (!DebtReceiptRefPattern.test(debtReceiptRef)) {
    throw new DebtReceiptWorkRequestUnsafe(
      'debtReceiptRef must be a receipt.public.debt.* ref.',
    )
  }

  const verificationCommandRef = singleVerificationCommandRef(
    input.verificationCommandRefs,
  )
  const debtReceiptProjection = projectDebtReceiptSettlement({
    ...input,
    verificationCommandRefs: [verificationCommandRef],
  })

  if (debtReceiptProjection.state !== 'funded') {
    throw new DebtReceiptWorkRequestUnsafe(
      `A debt-receipt work request must be funded before market listing; got ${debtReceiptProjection.state}.`,
    )
  }

  const workRequestInput: ForumWorkRequestInput = {
    budgetSats: debtReceiptProjection.budgetCapSats,
    deadlineRef: input.deadlineRef,
    objectiveRef: debtReceiptRef,
    repositoryRefs:
      input.repositoryRefs === undefined || input.repositoryRefs.length === 0
        ? [DefaultForumWorkRequestRepositoryRef]
        : input.repositoryRefs,
    requiredCapabilityRefs:
      input.requiredCapabilityRefs === undefined ||
      input.requiredCapabilityRefs.length === 0
        ? [DefaultForumWorkRequestCapabilityRef]
        : input.requiredCapabilityRefs,
    title: input.title,
    verificationCommandRef,
  }

  try {
    const normalizedInput = normalizeForumWorkRequestInput(workRequestInput)

    return {
      debtReceiptProjection,
      debtReceiptRef,
      idempotencyKey: debtReceiptWorkRequestIdempotencyKey(debtReceiptRef),
      input: workRequestInput,
      normalizedInput,
    }
  } catch (error) {
    if (error instanceof ForumWorkRequestUnsafe) {
      throw new DebtReceiptWorkRequestUnsafe(error.reason)
    }
    throw error
  }
}
