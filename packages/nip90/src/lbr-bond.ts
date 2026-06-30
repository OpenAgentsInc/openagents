/**
 * NIP-LBR forfeitable provider-bond feedback events.
 *
 * These helpers intentionally model only the relay contract for a provider-side
 * performance bond. They do not hold funds, create invoices, reveal preimages,
 * or move sats. The event body stays empty and every business field is a
 * public-safe ref or integer amount so the eventual credit-ledger, Lightning,
 * or Ark adapter can remain off-event and receipt-backed.
 */

import {
  type JobFeedback,
  jobFeedbackToTags,
  makeJobFeedback,
  parseJobFeedbackEvent,
} from "nostr-effect/nip90"

import {
  LBR_FEEDBACK_KIND,
  LbrProtocolError,
  type LbrUnsignedEventDraft,
} from "./lbr.js"

export const LBR_PROVIDER_BOND_STATUS_EXTRA = "labor_provider_bond"
export const LBR_BOND_RELEASE_STATUS_EXTRA = "labor_bond_released"
export const LBR_BOND_FORFEIT_STATUS_EXTRA = "labor_bond_forfeited"

export const LBR_PROVIDER_BOND_FEEDBACK_TYPE = "provider_bond"
export const LBR_BOND_RELEASE_FEEDBACK_TYPE = "bond_release"
export const LBR_BOND_FORFEIT_FEEDBACK_TYPE = "bond_forfeit"

export const LBR_FORFEIT_DESTINATIONS = [
  "refund_payer",
  "counterparty",
  "burn",
] as const

export type LbrForfeitDestination = typeof LBR_FORFEIT_DESTINATIONS[number]

export type LbrProviderBondInput = Readonly<{
  requestId: string
  requesterPubkey: string
  providerRef: string
  bondMsats: number
  bondReceiptRef: string
  forfeitDestination: LbrForfeitDestination
  forfeitConditionRef: string
  expiresAt?: string
  requestRelay?: string
}>

export type LbrProviderBond = Readonly<{
  feedback: JobFeedback
  requestId: string
  requesterPubkey: string
  providerRef: string
  bondMsats: number
  bondReceiptRef: string
  forfeitDestination: LbrForfeitDestination
  forfeitConditionRef: string
  expiresAt?: string
}>

export type LbrBondReleaseInput = Readonly<{
  requestId: string
  requesterPubkey: string
  bondReceiptRef: string
  releaseReceiptRef: string
  authorityRef: string
  requestRelay?: string
}>

export type LbrBondRelease = Readonly<{
  feedback: JobFeedback
  requestId: string
  requesterPubkey: string
  bondReceiptRef: string
  releaseReceiptRef: string
  authorityRef: string
}>

export type LbrBondForfeitInput = Readonly<{
  requestId: string
  requesterPubkey: string
  bondReceiptRef: string
  forfeitReceiptRef: string
  forfeitDestination: LbrForfeitDestination
  forfeitConditionRef: string
  authorityRef: string
  requestRelay?: string
}>

export type LbrBondForfeit = Readonly<{
  feedback: JobFeedback
  requestId: string
  requesterPubkey: string
  bondReceiptRef: string
  forfeitReceiptRef: string
  forfeitDestination: LbrForfeitDestination
  forfeitConditionRef: string
  authorityRef: string
}>

export type LbrBondOutcome =
  | Readonly<{
      kind: "released"
      requestId: string
      requesterPubkey: string
      bondReceiptRef: string
      releaseReceiptRef: string
      authorityRef: string
    }>
  | Readonly<{
      kind: "forfeited"
      requestId: string
      requesterPubkey: string
      bondReceiptRef: string
      forfeitReceiptRef: string
      forfeitDestination: LbrForfeitDestination
      forfeitConditionRef: string
      authorityRef: string
    }>

const publicRefPattern = /^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/
const pubkeyPattern = /^[a-f0-9]{64}$/i
const requestIdPattern = /^[a-f0-9]{64}$/i
const unsafeMaterialPattern =
  /(ANTHROPIC_API_KEY|OPENAI_API_KEY|MDK_ACCESS_TOKEN|SECRET|TOKEN=|-----BEGIN|mnemonic|payment_hash|payment_preimage|preimage|lnbc|lntb|lno1|file:\/\/|\/Users\/|\/home\/|C:\\|ssh:\/\/|private[_-]?repo|raw prompt|provider payload)/iu

const fail = (code: string, message: string): never => {
  throw new LbrProtocolError(code, message)
}

const ensureNoUnsafeMaterial = (value: string, field: string): void => {
  if (unsafeMaterialPattern.test(value)) {
    fail("unsafe_material", `${field} contains private or payment material`)
  }
}

const ensurePublicRef = (value: string, field: string): string => {
  ensureNoUnsafeMaterial(value, field)
  if (!publicRefPattern.test(value)) {
    fail("invalid_ref", `${field} must be a public-safe ref`)
  }
  return value
}

const ensurePositiveMsats = (value: number, field: string): number => {
  if (!Number.isInteger(value) || value <= 0) {
    fail("invalid_amount", `${field} must be a positive integer msat amount`)
  }
  return value
}

const ensurePubkey = (value: string, field: string): string => {
  if (!pubkeyPattern.test(value)) {
    fail("invalid_pubkey", `${field} must be a 32-byte hex pubkey`)
  }
  return value.toLowerCase()
}

const ensureRequestId = (value: string, field: string): string => {
  if (!requestIdPattern.test(value)) {
    fail("invalid_request_id", `${field} must be a 32-byte hex event id`)
  }
  return value.toLowerCase()
}

const ensureForfeitDestination = (
  value: string,
  field: string,
): LbrForfeitDestination => {
  if (
    value === "refund_payer" ||
    value === "counterparty" ||
    value === "burn"
  ) {
    return value
  }
  return fail(
    "invalid_forfeit_destination",
    `${field} must be refund_payer, counterparty, or burn`,
  )
}

const ensureContentSafe = (value: string, field: string): void =>
  ensureNoUnsafeMaterial(value, field)

const tagsFromEvent = (event: unknown): ReadonlyArray<readonly string[]> => {
  const tags = (event as { tags?: unknown } | null)?.tags
  if (!Array.isArray(tags)) {
    fail("invalid_event", "event tags must be an array")
  }
  return (tags as unknown[]).map((tag): readonly string[] => {
    if (!Array.isArray(tag) || !tag.every((part) => typeof part === "string")) {
      fail("invalid_event", "event tags must be string arrays")
    }
    return tag as string[]
  })
}

const ensureTagsSafe = (
  tags: ReadonlyArray<readonly string[]>,
  field: string,
): void => ensureContentSafe(JSON.stringify(tags), field)

const tagValues = (
  tags: ReadonlyArray<readonly string[]>,
  name: string,
): ReadonlyArray<string> =>
  tags.flatMap((tag) => (tag[0] === name && tag[1] !== undefined ? [tag[1]] : []))

const requiredTagValue = (
  tags: ReadonlyArray<readonly string[]>,
  name: string,
  field: string,
): string => {
  const value = tagValues(tags, name)[0]
  if (value === undefined || value === "") {
    return fail("missing_param", `${field} is required`)
  }
  return value
}

const ensureNoTerminalAmount = (
  feedback: JobFeedback,
  field: string,
): void => {
  const bolt11 = (feedback as { readonly bolt11?: string }).bolt11
  if (feedback.amount !== undefined || bolt11 !== undefined) {
    fail("unexpected_amount", `${field} must not carry payment amount material`)
  }
}

const ensureBondAmountTags = (
  feedback: JobFeedback,
  tags: ReadonlyArray<readonly string[]>,
): number => {
  const feedbackAmount =
    feedback.amount === undefined
      ? fail("missing_amount", "LBR provider bond requires amount")
      : ensurePositiveMsats(feedback.amount, "bondMsats")
  const taggedAmount = ensurePositiveMsats(
    Number(requiredTagValue(tags, "lbr_bond_msats", "bondMsats")),
    "bondMsats",
  )
  if (taggedAmount !== feedbackAmount) {
    fail("amount_mismatch", "bond amount tag must match feedback amount")
  }
  return feedbackAmount
}

const releasedOutcome = (release: LbrBondRelease): LbrBondOutcome => ({
  kind: "released",
  requestId: release.requestId,
  requesterPubkey: release.requesterPubkey,
  bondReceiptRef: release.bondReceiptRef,
  releaseReceiptRef: release.releaseReceiptRef,
  authorityRef: release.authorityRef,
})

const forfeitedOutcome = (forfeit: LbrBondForfeit): LbrBondOutcome => ({
  kind: "forfeited",
  requestId: forfeit.requestId,
  requesterPubkey: forfeit.requesterPubkey,
  bondReceiptRef: forfeit.bondReceiptRef,
  forfeitReceiptRef: forfeit.forfeitReceiptRef,
  forfeitDestination: forfeit.forfeitDestination,
  forfeitConditionRef: forfeit.forfeitConditionRef,
  authorityRef: forfeit.authorityRef,
})

export const makeLbrProviderBond = (
  input: LbrProviderBondInput,
): LbrProviderBond => {
  const bondMsats = ensurePositiveMsats(input.bondMsats, "bondMsats")
  const feedback = makeJobFeedback({
    status: "processing",
    statusExtra: LBR_PROVIDER_BOND_STATUS_EXTRA,
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.requesterPubkey, "requesterPubkey"),
    amount: bondMsats,
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })
  const expiresAt =
    input.expiresAt === undefined
      ? undefined
      : ensurePublicRef(input.expiresAt, "expiresAt")

  return {
    feedback,
    requestId: feedback.requestId,
    requesterPubkey: feedback.customerPubkey,
    providerRef: ensurePublicRef(input.providerRef, "providerRef"),
    bondMsats,
    bondReceiptRef: ensurePublicRef(input.bondReceiptRef, "bondReceiptRef"),
    forfeitDestination: ensureForfeitDestination(
      input.forfeitDestination,
      "forfeitDestination",
    ),
    forfeitConditionRef: ensurePublicRef(
      input.forfeitConditionRef,
      "forfeitConditionRef",
    ),
    ...(expiresAt === undefined ? {} : { expiresAt }),
  }
}

export const lbrProviderBondToDraft = (
  bond: LbrProviderBond,
): LbrUnsignedEventDraft => ({
  kind: LBR_FEEDBACK_KIND,
  tags: [
    ...jobFeedbackToTags(bond.feedback),
    ["lbr_feedback_type", LBR_PROVIDER_BOND_FEEDBACK_TYPE],
    ["lbr_provider_ref", bond.providerRef],
    ["lbr_bond_msats", String(bond.bondMsats)],
    ["lbr_bond_receipt_ref", bond.bondReceiptRef],
    ["lbr_forfeit_destination", bond.forfeitDestination],
    ["lbr_forfeit_condition_ref", bond.forfeitConditionRef],
    ...(bond.expiresAt === undefined
      ? []
      : [["lbr_expires_at", bond.expiresAt]]),
  ],
  content: bond.feedback.content,
})

export const decodeLbrProviderBondEvent = (
  event: unknown,
): LbrProviderBond => {
  const feedback = parseJobFeedbackEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "provider bond tags")
  if (
    feedback.status !== "processing" ||
    feedback.statusExtra !== LBR_PROVIDER_BOND_STATUS_EXTRA
  ) {
    fail(
      "invalid_feedback",
      "LBR provider bond must be processing labor_provider_bond feedback",
    )
  }
  ensureContentSafe(feedback.content, "provider bond content")
  if (tagValues(tags, "lbr_feedback_type")[0] !== LBR_PROVIDER_BOND_FEEDBACK_TYPE) {
    fail("missing_param", "LBR provider bond requires feedback type provider_bond")
  }

  return {
    feedback,
    requestId: ensureRequestId(feedback.requestId, "requestId"),
    requesterPubkey: ensurePubkey(feedback.customerPubkey, "requesterPubkey"),
    providerRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_provider_ref", "providerRef"),
      "providerRef",
    ),
    bondMsats: ensureBondAmountTags(feedback, tags),
    bondReceiptRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_bond_receipt_ref", "bondReceiptRef"),
      "bondReceiptRef",
    ),
    forfeitDestination: ensureForfeitDestination(
      requiredTagValue(tags, "lbr_forfeit_destination", "forfeitDestination"),
      "forfeitDestination",
    ),
    forfeitConditionRef: ensurePublicRef(
      requiredTagValue(
        tags,
        "lbr_forfeit_condition_ref",
        "forfeitConditionRef",
      ),
      "forfeitConditionRef",
    ),
    ...(tagValues(tags, "lbr_expires_at")[0] === undefined
      ? {}
      : {
          expiresAt: ensurePublicRef(
            tagValues(tags, "lbr_expires_at")[0] ?? "",
            "expiresAt",
          ),
        }),
  }
}

export const makeLbrBondRelease = (
  input: LbrBondReleaseInput,
): LbrBondRelease => {
  const feedback = makeJobFeedback({
    status: "success",
    statusExtra: LBR_BOND_RELEASE_STATUS_EXTRA,
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.requesterPubkey, "requesterPubkey"),
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })

  return {
    feedback,
    requestId: feedback.requestId,
    requesterPubkey: feedback.customerPubkey,
    bondReceiptRef: ensurePublicRef(input.bondReceiptRef, "bondReceiptRef"),
    releaseReceiptRef: ensurePublicRef(
      input.releaseReceiptRef,
      "releaseReceiptRef",
    ),
    authorityRef: ensurePublicRef(input.authorityRef, "authorityRef"),
  }
}

export const lbrBondReleaseToDraft = (
  release: LbrBondRelease,
): LbrUnsignedEventDraft => ({
  kind: LBR_FEEDBACK_KIND,
  tags: [
    ...jobFeedbackToTags(release.feedback),
    ["lbr_feedback_type", LBR_BOND_RELEASE_FEEDBACK_TYPE],
    ["lbr_bond_receipt_ref", release.bondReceiptRef],
    ["lbr_release_receipt_ref", release.releaseReceiptRef],
    ["lbr_authority_ref", release.authorityRef],
  ],
  content: release.feedback.content,
})

export const decodeLbrBondReleaseEvent = (
  event: unknown,
): LbrBondRelease => {
  const feedback = parseJobFeedbackEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "bond release tags")
  if (
    feedback.status !== "success" ||
    feedback.statusExtra !== LBR_BOND_RELEASE_STATUS_EXTRA
  ) {
    fail(
      "invalid_feedback",
      "LBR bond release must be success labor_bond_released feedback",
    )
  }
  ensureContentSafe(feedback.content, "bond release content")
  ensureNoTerminalAmount(feedback, "bond release")
  if (tagValues(tags, "lbr_feedback_type")[0] !== LBR_BOND_RELEASE_FEEDBACK_TYPE) {
    fail("missing_param", "LBR bond release requires feedback type bond_release")
  }

  return {
    feedback,
    requestId: ensureRequestId(feedback.requestId, "requestId"),
    requesterPubkey: ensurePubkey(feedback.customerPubkey, "requesterPubkey"),
    bondReceiptRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_bond_receipt_ref", "bondReceiptRef"),
      "bondReceiptRef",
    ),
    releaseReceiptRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_release_receipt_ref", "releaseReceiptRef"),
      "releaseReceiptRef",
    ),
    authorityRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_authority_ref", "authorityRef"),
      "authorityRef",
    ),
  }
}

export const makeLbrBondForfeit = (
  input: LbrBondForfeitInput,
): LbrBondForfeit => {
  const feedback = makeJobFeedback({
    status: "success",
    statusExtra: LBR_BOND_FORFEIT_STATUS_EXTRA,
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.requesterPubkey, "requesterPubkey"),
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })

  return {
    feedback,
    requestId: feedback.requestId,
    requesterPubkey: feedback.customerPubkey,
    bondReceiptRef: ensurePublicRef(input.bondReceiptRef, "bondReceiptRef"),
    forfeitReceiptRef: ensurePublicRef(
      input.forfeitReceiptRef,
      "forfeitReceiptRef",
    ),
    forfeitDestination: ensureForfeitDestination(
      input.forfeitDestination,
      "forfeitDestination",
    ),
    forfeitConditionRef: ensurePublicRef(
      input.forfeitConditionRef,
      "forfeitConditionRef",
    ),
    authorityRef: ensurePublicRef(input.authorityRef, "authorityRef"),
  }
}

export const lbrBondForfeitToDraft = (
  forfeit: LbrBondForfeit,
): LbrUnsignedEventDraft => ({
  kind: LBR_FEEDBACK_KIND,
  tags: [
    ...jobFeedbackToTags(forfeit.feedback),
    ["lbr_feedback_type", LBR_BOND_FORFEIT_FEEDBACK_TYPE],
    ["lbr_bond_receipt_ref", forfeit.bondReceiptRef],
    ["lbr_forfeit_receipt_ref", forfeit.forfeitReceiptRef],
    ["lbr_forfeit_destination", forfeit.forfeitDestination],
    ["lbr_forfeit_condition_ref", forfeit.forfeitConditionRef],
    ["lbr_authority_ref", forfeit.authorityRef],
  ],
  content: forfeit.feedback.content,
})

export const decodeLbrBondForfeitEvent = (
  event: unknown,
): LbrBondForfeit => {
  const feedback = parseJobFeedbackEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "bond forfeit tags")
  if (
    feedback.status !== "success" ||
    feedback.statusExtra !== LBR_BOND_FORFEIT_STATUS_EXTRA
  ) {
    fail(
      "invalid_feedback",
      "LBR bond forfeit must be success labor_bond_forfeited feedback",
    )
  }
  ensureContentSafe(feedback.content, "bond forfeit content")
  ensureNoTerminalAmount(feedback, "bond forfeit")
  if (tagValues(tags, "lbr_feedback_type")[0] !== LBR_BOND_FORFEIT_FEEDBACK_TYPE) {
    fail("missing_param", "LBR bond forfeit requires feedback type bond_forfeit")
  }

  return {
    feedback,
    requestId: ensureRequestId(feedback.requestId, "requestId"),
    requesterPubkey: ensurePubkey(feedback.customerPubkey, "requesterPubkey"),
    bondReceiptRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_bond_receipt_ref", "bondReceiptRef"),
      "bondReceiptRef",
    ),
    forfeitReceiptRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_forfeit_receipt_ref", "forfeitReceiptRef"),
      "forfeitReceiptRef",
    ),
    forfeitDestination: ensureForfeitDestination(
      requiredTagValue(tags, "lbr_forfeit_destination", "forfeitDestination"),
      "forfeitDestination",
    ),
    forfeitConditionRef: ensurePublicRef(
      requiredTagValue(
        tags,
        "lbr_forfeit_condition_ref",
        "forfeitConditionRef",
      ),
      "forfeitConditionRef",
    ),
    authorityRef: ensurePublicRef(
      requiredTagValue(tags, "lbr_authority_ref", "authorityRef"),
      "authorityRef",
    ),
  }
}

export const decodeLbrBondOutcomeEvent = (event: unknown): LbrBondOutcome => {
  const feedbackType = tagValues(tagsFromEvent(event), "lbr_feedback_type")[0]
  if (feedbackType === LBR_BOND_RELEASE_FEEDBACK_TYPE) {
    return releasedOutcome(decodeLbrBondReleaseEvent(event))
  }
  if (feedbackType === LBR_BOND_FORFEIT_FEEDBACK_TYPE) {
    return forfeitedOutcome(decodeLbrBondForfeitEvent(event))
  }
  return fail(
    "invalid_feedback",
    "LBR bond outcome must be bond_release or bond_forfeit feedback",
  )
}

export const makeLbrBondOutcome = (input: {
  readonly release?: LbrBondRelease | undefined
  readonly forfeit?: LbrBondForfeit | undefined
}): LbrBondOutcome => {
  const hasRelease = input.release !== undefined
  const hasForfeit = input.forfeit !== undefined
  if (hasRelease === hasForfeit) {
    fail(
      "invalid_bond_outcome",
      "LBR bond outcome requires exactly one release or forfeit terminal event",
    )
  }
  return input.release === undefined
    ? forfeitedOutcome(input.forfeit as LbrBondForfeit)
    : releasedOutcome(input.release)
}
