import {
  KIND_JOB_FEEDBACK,
  KIND_JOB_LABOR_CODE_TASK,
  type JobFeedback,
  type LaborJobRequest,
  type LaborJobResult,
  jobFeedbackToTags,
  jobParam,
  laborJobRequestToTags,
  laborJobResultToTags,
  makeJobFeedback,
  makeLaborJobRequest,
  makeLaborJobResult,
  parseJobFeedbackEvent,
  parseLaborJobRequestEvent,
  parseLaborJobResultEvent,
} from "nostr-effect/nip90"

export const LBR_AGENTIC_CODING_REQUEST_KIND = KIND_JOB_LABOR_CODE_TASK
export const LBR_AGENTIC_CODING_RESULT_KIND =
  LBR_AGENTIC_CODING_REQUEST_KIND + 1000
export const LBR_FEEDBACK_KIND = KIND_JOB_FEEDBACK
export const LBR_RESERVED_LABOR_KIND_MIN = 5930
export const LBR_RESERVED_LABOR_KIND_MAX = 5939
export const LBR_OUTPUT_DELIVERY_POLICY = "output_only"

export type LbrUnsignedEventDraft = Readonly<{
  kind: number
  tags: ReadonlyArray<readonly string[]>
  content: string
}>

export type LbrAgenticCodingRequestInput = Readonly<{
  objectiveRef: string
  repositoryRefs: ReadonlyArray<string>
  verificationCommandRef: string
  requiredCapabilityRefs: ReadonlyArray<string>
  bidMsats: number
  deadline?: string
  forumTopicRef?: string
  relays?: ReadonlyArray<string>
}>

export type LbrAgenticCodingRequest = Readonly<{
  kind: typeof LBR_AGENTIC_CODING_REQUEST_KIND
  labor: LaborJobRequest
  objectiveRef: string
  repositoryRefs: ReadonlyArray<string>
  verificationCommandRef: string
  requiredCapabilityRefs: ReadonlyArray<string>
  bidMsats: number
  deadline?: string
  outputDelivery: typeof LBR_OUTPUT_DELIVERY_POLICY
  forumTopicRef?: string
}>

export type LbrQuoteInput = Readonly<{
  requestId: string
  requesterPubkey: string
  amountMsats: number
  providerRef: string
  capabilityRefs: ReadonlyArray<string>
  quoteRef: string
  expiresAt?: string
  requestRelay?: string
}>

export type LbrQuote = Readonly<{
  feedback: JobFeedback
  requestId: string
  requesterPubkey: string
  amountMsats: number
  providerRef: string
  capabilityRefs: ReadonlyArray<string>
  quoteRef: string
  expiresAt?: string
}>

export type LbrAcceptanceInput = Readonly<{
  requestId: string
  providerPubkey: string
  escrowReceiptRef: string
  acceptanceRef: string
  requestRelay?: string
}>

export type LbrAcceptance = Readonly<{
  feedback: JobFeedback
  requestId: string
  providerPubkey: string
  escrowReceiptRef: string
  acceptanceRef: string
}>

export type LbrResultInput = Readonly<{
  requestId: string
  requesterPubkey: string
  artifactRefs: ReadonlyArray<string>
  platformCloseoutRef: string
  summaryRef: string
  testRef: string
  buildRef?: string
  requestRelay?: string
}>

export type LbrResult = Readonly<{
  kind: typeof LBR_AGENTIC_CODING_RESULT_KIND
  labor: LaborJobResult
  artifactRefs: ReadonlyArray<string>
  platformCloseoutRef: string
  summaryRef: string
  testRef: string
  buildRef?: string
}>

export class LbrProtocolError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.name = "LbrProtocolError"
    this.code = code
  }
}

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

const ensurePublicRefs = (
  values: ReadonlyArray<string>,
  field: string,
): ReadonlyArray<string> => {
  if (values.length === 0) {
    fail("missing_ref", `${field} requires at least one ref`)
  }
  return values.map((value) => ensurePublicRef(value, field))
}

const ensureKind = (value: number, expected: number, field: string): void => {
  if (value !== expected) {
    fail("invalid_kind", `${field} must use kind ${expected}`)
  }
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

const ensureContentSafe = (value: string, field: string): void =>
  ensureNoUnsafeMaterial(value, field)

const ensureTagsSafe = (
  tags: ReadonlyArray<readonly string[]>,
  field: string,
): void => ensureContentSafe(JSON.stringify(tags), field)

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

const tagValues = (
  tags: ReadonlyArray<readonly string[]>,
  name: string,
): ReadonlyArray<string> =>
  tags.flatMap((tag) => (tag[0] === name && tag[1] !== undefined ? [tag[1]] : []))

const requiredParam = (
  params: ReadonlyArray<{ key: string; value: string }>,
  key: string,
): string => {
  const value = params.find((param) => param.key === key)?.value
  if (value === undefined || value === "") {
    fail("missing_param", `missing LBR param ${key}`)
  }
  return String(value)
}

const paramValues = (
  params: ReadonlyArray<{ key: string; value: string }>,
  key: string,
): ReadonlyArray<string> =>
  params.filter((param) => param.key === key).map((param) => param.value)

export const makeLbrAgenticCodingRequest = (
  input: LbrAgenticCodingRequestInput,
): LbrAgenticCodingRequest => {
  const objectiveRef = ensurePublicRef(input.objectiveRef, "objectiveRef")
  const repositoryRefs = ensurePublicRefs(input.repositoryRefs, "repositoryRefs")
  const verificationCommandRef = ensurePublicRef(
    input.verificationCommandRef,
    "verificationCommandRef",
  )
  const requiredCapabilityRefs = ensurePublicRefs(
    input.requiredCapabilityRefs,
    "requiredCapabilityRefs",
  )
  const bidMsats = ensurePositiveMsats(input.bidMsats, "bidMsats")
  const deadline =
    input.deadline === undefined
      ? undefined
      : ensurePublicRef(input.deadline, "deadline")
  const forumTopicRef =
    input.forumTopicRef === undefined
      ? undefined
      : ensurePublicRef(input.forumTopicRef, "forumTopicRef")

  const labor = makeLaborJobRequest({
    jobType: "code_task",
    inputRefs: [objectiveRef, ...repositoryRefs],
    acceptanceCriteria: [verificationCommandRef],
    bid: bidMsats,
    content: "",
    ...(input.relays === undefined ? {} : { relays: input.relays }),
    params: [
      jobParam("lbr_objective_ref", objectiveRef),
      ...repositoryRefs.map((ref) => jobParam("lbr_repository_ref", ref)),
      jobParam("lbr_verification_command_ref", verificationCommandRef),
      ...requiredCapabilityRefs.map((ref) =>
        jobParam("lbr_required_capability_ref", ref),
      ),
      jobParam("lbr_output_delivery", LBR_OUTPUT_DELIVERY_POLICY),
      ...(deadline === undefined ? [] : [jobParam("lbr_deadline", deadline)]),
      ...(forumTopicRef === undefined
        ? []
        : [jobParam("lbr_forum_topic_ref", forumTopicRef)]),
    ],
  })

  return {
    kind: LBR_AGENTIC_CODING_REQUEST_KIND,
    labor,
    objectiveRef,
    repositoryRefs,
    verificationCommandRef,
    requiredCapabilityRefs,
    bidMsats,
    ...(deadline === undefined ? {} : { deadline }),
    outputDelivery: LBR_OUTPUT_DELIVERY_POLICY,
    ...(forumTopicRef === undefined ? {} : { forumTopicRef }),
  }
}

export const lbrAgenticCodingRequestToDraft = (
  request: LbrAgenticCodingRequest,
): LbrUnsignedEventDraft => ({
  kind: request.kind,
  tags: laborJobRequestToTags(request.labor),
  content: request.labor.request.content,
})

export const decodeLbrAgenticCodingRequestEvent = (
  event: unknown,
): LbrAgenticCodingRequest => {
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "request tags")
  const labor = parseLaborJobRequestEvent(event)
  ensureKind(labor.kind as number, LBR_AGENTIC_CODING_REQUEST_KIND, "request")
  if (labor.jobType !== "code_task") {
    fail("invalid_job_type", "LBR agentic coding request must use code_task")
  }
  if (labor.request.content.trim() !== "") {
    fail("unsafe_content", "LBR request content must be empty and ref-only")
  }

  const objectiveRef = ensurePublicRef(
    requiredParam(labor.request.params, "lbr_objective_ref"),
    "objectiveRef",
  )
  const repositoryRefs = ensurePublicRefs(
    paramValues(labor.request.params, "lbr_repository_ref"),
    "repositoryRefs",
  )
  const verificationCommandRef = ensurePublicRef(
    requiredParam(labor.request.params, "lbr_verification_command_ref"),
    "verificationCommandRef",
  )
  const requiredCapabilityRefs = ensurePublicRefs(
    paramValues(labor.request.params, "lbr_required_capability_ref"),
    "requiredCapabilityRefs",
  )
  const outputDelivery = requiredParam(labor.request.params, "lbr_output_delivery")
  if (outputDelivery !== LBR_OUTPUT_DELIVERY_POLICY) {
    fail("invalid_output_delivery", "LBR output delivery must be output_only")
  }
  const deadline = paramValues(labor.request.params, "lbr_deadline")[0]
  const forumTopicRef = paramValues(labor.request.params, "lbr_forum_topic_ref")[0]
  const bidMsats =
    labor.request.bid === undefined
      ? fail("missing_bid", "LBR request requires a max budget bid")
      : ensurePositiveMsats(labor.request.bid, "bidMsats")

  return {
    kind: LBR_AGENTIC_CODING_REQUEST_KIND,
    labor,
    objectiveRef,
    repositoryRefs,
    verificationCommandRef,
    requiredCapabilityRefs,
    bidMsats,
    ...(deadline === undefined
      ? {}
      : { deadline: ensurePublicRef(deadline, "deadline") }),
    outputDelivery: LBR_OUTPUT_DELIVERY_POLICY,
    ...(forumTopicRef === undefined
      ? {}
      : { forumTopicRef: ensurePublicRef(forumTopicRef, "forumTopicRef") }),
  }
}

export const makeLbrQuote = (input: LbrQuoteInput): LbrQuote => {
  const amountMsats = ensurePositiveMsats(input.amountMsats, "amountMsats")
  const providerRef = ensurePublicRef(input.providerRef, "providerRef")
  const capabilityRefs = ensurePublicRefs(input.capabilityRefs, "capabilityRefs")
  const quoteRef = ensurePublicRef(input.quoteRef, "quoteRef")
  const expiresAt =
    input.expiresAt === undefined
      ? undefined
      : ensurePublicRef(input.expiresAt, "expiresAt")
  const feedback = makeJobFeedback({
    status: "payment-required",
    statusExtra: "labor_quote",
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.requesterPubkey, "requesterPubkey"),
    amount: amountMsats,
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })

  return {
    feedback,
    requestId: feedback.requestId,
    requesterPubkey: feedback.customerPubkey,
    amountMsats,
    providerRef,
    capabilityRefs,
    quoteRef,
    ...(expiresAt === undefined ? {} : { expiresAt }),
  }
}

export const lbrQuoteToDraft = (quote: LbrQuote): LbrUnsignedEventDraft => ({
  kind: LBR_FEEDBACK_KIND,
  tags: [
    ...jobFeedbackToTags(quote.feedback),
    ["lbr_feedback_type", "quote"],
    ["lbr_provider_ref", quote.providerRef],
    ["lbr_quote_ref", quote.quoteRef],
    ...quote.capabilityRefs.map((ref) => ["lbr_capability_ref", ref]),
    ...(quote.expiresAt === undefined
      ? []
      : [["lbr_expires_at", quote.expiresAt]]),
  ],
  content: quote.feedback.content,
})

export const decodeLbrQuoteEvent = (event: unknown): LbrQuote => {
  const feedback = parseJobFeedbackEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "quote tags")
  if (feedback.status !== "payment-required" || feedback.statusExtra !== "labor_quote") {
    fail("invalid_feedback", "LBR quote must be payment-required labor_quote feedback")
  }
  ensureContentSafe(feedback.content, "quote content")
  const quoteAmount =
    feedback.amount === undefined
      ? fail("missing_amount", "LBR quote requires amount")
      : feedback.amount
  if (tagValues(tags, "lbr_feedback_type")[0] !== "quote") {
    fail("missing_param", "LBR quote requires feedback type quote")
  }

  return {
    feedback,
    requestId: ensureRequestId(feedback.requestId, "requestId"),
    requesterPubkey: ensurePubkey(feedback.customerPubkey, "requesterPubkey"),
    amountMsats: ensurePositiveMsats(quoteAmount, "amountMsats"),
    providerRef: ensurePublicRef(
      tagValues(tags, "lbr_provider_ref")[0] ?? "",
      "providerRef",
    ),
    capabilityRefs: ensurePublicRefs(
      tagValues(tags, "lbr_capability_ref"),
      "capabilityRefs",
    ),
    quoteRef: ensurePublicRef(tagValues(tags, "lbr_quote_ref")[0] ?? "", "quoteRef"),
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

export const makeLbrAcceptance = (input: LbrAcceptanceInput): LbrAcceptance => {
  const feedback = makeJobFeedback({
    status: "processing",
    statusExtra: "labor_quote_accepted",
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.providerPubkey, "providerPubkey"),
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })
  return {
    feedback,
    requestId: feedback.requestId,
    providerPubkey: feedback.customerPubkey,
    escrowReceiptRef: ensurePublicRef(input.escrowReceiptRef, "escrowReceiptRef"),
    acceptanceRef: ensurePublicRef(input.acceptanceRef, "acceptanceRef"),
  }
}

export const lbrAcceptanceToDraft = (
  acceptance: LbrAcceptance,
): LbrUnsignedEventDraft => ({
  kind: LBR_FEEDBACK_KIND,
  tags: [
    ...jobFeedbackToTags(acceptance.feedback),
    ["lbr_feedback_type", "acceptance"],
    ["lbr_escrow_receipt_ref", acceptance.escrowReceiptRef],
    ["lbr_acceptance_ref", acceptance.acceptanceRef],
  ],
  content: acceptance.feedback.content,
})

export const decodeLbrAcceptanceEvent = (event: unknown): LbrAcceptance => {
  const feedback = parseJobFeedbackEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "acceptance tags")
  if (
    feedback.status !== "processing" ||
    feedback.statusExtra !== "labor_quote_accepted"
  ) {
    fail("invalid_feedback", "LBR acceptance must be processing labor_quote_accepted feedback")
  }
  ensureContentSafe(feedback.content, "acceptance content")
  if (tagValues(tags, "lbr_feedback_type")[0] !== "acceptance") {
    fail("missing_param", "LBR acceptance requires feedback type acceptance")
  }

  return {
    feedback,
    requestId: ensureRequestId(feedback.requestId, "requestId"),
    providerPubkey: ensurePubkey(feedback.customerPubkey, "providerPubkey"),
    escrowReceiptRef: ensurePublicRef(
      tagValues(tags, "lbr_escrow_receipt_ref")[0] ?? "",
      "escrowReceiptRef",
    ),
    acceptanceRef: ensurePublicRef(
      tagValues(tags, "lbr_acceptance_ref")[0] ?? "",
      "acceptanceRef",
    ),
  }
}

export const makeLbrResult = (input: LbrResultInput): LbrResult => {
  const artifactRefs = ensurePublicRefs(input.artifactRefs, "artifactRefs")
  const platformCloseoutRef = ensurePublicRef(
    input.platformCloseoutRef,
    "platformCloseoutRef",
  )
  const summaryRef = ensurePublicRef(input.summaryRef, "summaryRef")
  const testRef = ensurePublicRef(input.testRef, "testRef")
  const buildRef =
    input.buildRef === undefined
      ? undefined
      : ensurePublicRef(input.buildRef, "buildRef")
  const labor = makeLaborJobResult({
    jobType: "code_task",
    requestId: ensureRequestId(input.requestId, "requestId"),
    customerPubkey: ensurePubkey(input.requesterPubkey, "requesterPubkey"),
    artifactRefs,
    content: "",
    ...(input.requestRelay === undefined
      ? {}
      : { requestRelay: input.requestRelay }),
  })

  return {
    kind: LBR_AGENTIC_CODING_RESULT_KIND,
    labor,
    artifactRefs,
    platformCloseoutRef,
    summaryRef,
    testRef,
    ...(buildRef === undefined ? {} : { buildRef }),
  }
}

export const lbrResultToDraft = (result: LbrResult): LbrUnsignedEventDraft => ({
  kind: result.kind,
  tags: [
    ...laborJobResultToTags(result.labor),
    ["lbr_platform_closeout_ref", result.platformCloseoutRef],
    ["lbr_summary_ref", result.summaryRef],
    ["lbr_test_ref", result.testRef],
    ...(result.buildRef === undefined ? [] : [["lbr_build_ref", result.buildRef]]),
  ],
  content: result.labor.result.content,
})

export const decodeLbrResultEvent = (event: unknown): LbrResult => {
  const labor = parseLaborJobResultEvent(event)
  const tags = tagsFromEvent(event)
  ensureTagsSafe(tags, "result tags")
  ensureKind(labor.result.kind as number, LBR_AGENTIC_CODING_RESULT_KIND, "result")
  if (labor.jobType !== "code_task") {
    fail("invalid_job_type", "LBR result must be for code_task")
  }
  ensureContentSafe(labor.result.content, "result content")

  return {
    kind: LBR_AGENTIC_CODING_RESULT_KIND,
    labor,
    artifactRefs: ensurePublicRefs(labor.artifactRefs, "artifactRefs"),
    platformCloseoutRef: ensurePublicRef(
      tagValues(tags, "lbr_platform_closeout_ref")[0] ?? "",
      "platformCloseoutRef",
    ),
    summaryRef: ensurePublicRef(
      tagValues(tags, "lbr_summary_ref")[0] ?? "",
      "summaryRef",
    ),
    testRef: ensurePublicRef(tagValues(tags, "lbr_test_ref")[0] ?? "", "testRef"),
    ...(tagValues(tags, "lbr_build_ref")[0] === undefined
      ? {}
      : {
          buildRef: ensurePublicRef(
            tagValues(tags, "lbr_build_ref")[0] ?? "",
            "buildRef",
          ),
        }),
  }
}
