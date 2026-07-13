import { Schema } from "effect"

export const ProductSpecOpenChannel = "openagents:product-spec:open" as const
export const ProductSpecCreateChannel = "openagents:product-spec:create" as const
export const ProductSpecPlanProposeChannel = "openagents:product-spec:plan-propose" as const
export const ProductSpecPlanAcceptChannel = "openagents:product-spec:plan-accept" as const
export const ProductSpecEditProposeChannel = "openagents:product-spec:edit-propose" as const
export const ProductSpecEditConfirmChannel = "openagents:product-spec:edit-confirm" as const
export const ProductSpecPacketAdmitChannel = "openagents:product-spec:packet-admit" as const
export const ProductSpecPacketBlockChannel = "openagents:product-spec:packet-block" as const
export const ProductSpecPacketDispositionChannel = "openagents:product-spec:packet-disposition" as const
export const ProductSpecEvidenceRecordChannel = "openagents:product-spec:evidence-record" as const
export const ProductSpecEvidenceVerifyChannel = "openagents:product-spec:evidence-verify" as const
export const ProductSpecRunGetChannel = "openagents:product-spec:run-get" as const

const RefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const DigestSchema = Schema.String.check(
  Schema.isPattern(/^sha256:[a-f0-9]{64}$/),
)
const CriterionIdSchema = Schema.String.check(
  Schema.isPattern(/^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+$/),
)
const RelativeSpecPathSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[^\\/](?!.*(?:^|\/)\.\.(?:\/|$)).*\.product-spec\.md$/),
)
const NonEmptyTextSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(20_000))
const TimestampSchema = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
)

export const ProductSpecValidationIssueSchema = Schema.Struct({
  code: RefSchema,
  message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
  path: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512))),
})
export type ProductSpecValidationIssue = typeof ProductSpecValidationIssueSchema.Type

export const ProductSpecIdentitySchema = Schema.Struct({
  specRef: RefSchema,
  relativePath: RelativeSpecPathSchema,
  revision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  digest: DigestSchema,
})
export type ProductSpecIdentity = typeof ProductSpecIdentitySchema.Type

export const ProductSpecCriterionSchema = Schema.Struct({
  id: CriterionIdSchema,
  criterionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800)),
  body: NonEmptyTextSchema,
  ordinal: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
})
export type ProductSpecCriterion = typeof ProductSpecCriterionSchema.Type

export const ProductSpecProjectionSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("invalid"),
    relativePath: RelativeSpecPathSchema,
    sourceMarkdown: Schema.String.check(Schema.isMaxLength(1_000_000)),
    standardValid: Schema.Boolean,
    executable: Schema.Literal(false),
    errors: Schema.Array(ProductSpecValidationIssueSchema),
    warnings: Schema.Array(ProductSpecValidationIssueSchema),
  }),
  Schema.Struct({
    state: Schema.Literal("ready"),
    title: NonEmptyTextSchema,
    sourceMarkdown: Schema.String.check(Schema.isMaxLength(1_000_000)),
    identity: ProductSpecIdentitySchema,
    executable: Schema.Literal(true),
    criteria: Schema.Array(ProductSpecCriterionSchema),
    warnings: Schema.Array(ProductSpecValidationIssueSchema),
  }),
])
export type ProductSpecProjection = typeof ProductSpecProjectionSchema.Type

export const ProductSpecPacketStateSchema = Schema.Literals([
  "planned",
  "active",
  "blocked",
  "evidence_present",
  "verified",
  "failed",
  "superseded",
  "cancelled",
])
export type ProductSpecPacketState = typeof ProductSpecPacketStateSchema.Type

export const ProductSpecEvidenceKindSchema = Schema.Literals([
  "test_run",
  "behavior_eval",
  "artifact",
  "diff_review",
  "receipt",
])
export type ProductSpecEvidenceKind = typeof ProductSpecEvidenceKindSchema.Type

export const ProductSpecEvidenceReceiptSchema = Schema.Struct({
  receiptRef: RefSchema,
  evidenceRef: RefSchema,
  kind: ProductSpecEvidenceKindSchema,
  producerRef: RefSchema,
  spec: ProductSpecIdentitySchema,
  criterionIds: Schema.Array(CriterionIdSchema),
  producedAt: TimestampSchema,
})
export type ProductSpecEvidenceReceipt = typeof ProductSpecEvidenceReceiptSchema.Type

export const ProductSpecVerificationReceiptSchema = Schema.Struct({
  receiptRef: RefSchema,
  evidenceReceiptRefs: Schema.Array(RefSchema),
  outputRef: RefSchema,
  verifierRef: RefSchema,
  spec: ProductSpecIdentitySchema,
  criterionIds: Schema.Array(CriterionIdSchema),
  verdict: Schema.Literal("passed"),
  verifiedAt: TimestampSchema,
})
export type ProductSpecVerificationReceipt = typeof ProductSpecVerificationReceiptSchema.Type

export const ProductSpecWorkPacketSchema = Schema.Struct({
  packetRef: RefSchema,
  title: NonEmptyTextSchema,
  criterionIds: Schema.Array(CriterionIdSchema),
  criterionRefs: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(800))),
  dependencyRefs: Schema.Array(RefSchema),
  allocation: Schema.Literals(["root", "child"]),
  state: ProductSpecPacketStateSchema,
  evidenceRefs: Schema.Array(RefSchema),
  evidenceReceipts: Schema.Array(ProductSpecEvidenceReceiptSchema),
  evidenceProducerRef: Schema.optional(RefSchema),
  verifierRefs: Schema.Array(RefSchema),
  verificationReceipts: Schema.Array(ProductSpecVerificationReceiptSchema),
  activeLease: Schema.optional(Schema.NullOr(Schema.Struct({
    leaseRef: RefSchema,
    executorRef: RefSchema,
    executionMode: Schema.Literals(["owner-present", "afk"]),
    admittedAt: TimestampSchema,
  }))),
  blockedReason: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000))),
})
export type ProductSpecWorkPacket = typeof ProductSpecWorkPacketSchema.Type

export const ProductSpecPlanSchema = Schema.Struct({
  planRef: RefSchema,
  spec: ProductSpecIdentitySchema,
  workContextRef: RefSchema,
  state: Schema.Literals(["proposed", "accepted", "revision_mismatch", "superseded", "cancelled"]),
  packets: Schema.Array(ProductSpecWorkPacketSchema),
  deferredCriterionIds: Schema.Array(CriterionIdSchema),
  proposedAt: TimestampSchema,
  acceptedAt: Schema.optional(TimestampSchema),
})
export type ProductSpecPlan = typeof ProductSpecPlanSchema.Type

export const ProductSpecRunSchema = Schema.Struct({
  runRef: RefSchema,
  spec: ProductSpecIdentitySchema,
  workContextRef: RefSchema,
  plan: ProductSpecPlanSchema,
  createdAt: TimestampSchema,
  updatedAt: TimestampSchema,
})
export type ProductSpecRun = typeof ProductSpecRunSchema.Type

export const ProductSpecReconciliationSchema = Schema.Struct({
  retainedCriterionIds: Schema.Array(CriterionIdSchema),
  changedCriterionIds: Schema.Array(CriterionIdSchema),
  addedCriterionIds: Schema.Array(CriterionIdSchema),
  removedCriterionIds: Schema.Array(CriterionIdSchema),
})
export type ProductSpecReconciliation = typeof ProductSpecReconciliationSchema.Type

export const ProductSpecEditProposalSchema = Schema.Struct({
  proposalRef: RefSchema,
  workContextRef: RefSchema,
  previous: ProductSpecIdentitySchema,
  next: ProductSpecIdentitySchema,
  reconciliation: ProductSpecReconciliationSchema,
  diff: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_100_000)),
  proposedAt: TimestampSchema,
  state: Schema.Literals(["proposed", "confirmed", "superseded"]),
  confirmedAt: Schema.optional(TimestampSchema),
})
export type ProductSpecEditProposal = typeof ProductSpecEditProposalSchema.Type

export const ProductSpecEditConfirmationSchema = Schema.Struct({
  proposal: ProductSpecEditProposalSchema,
  projection: ProductSpecProjectionSchema,
  reconciled: Schema.Boolean,
  criterionDisposition: Schema.Literal("supersede_affected_packets"),
})
export type ProductSpecEditConfirmation = typeof ProductSpecEditConfirmationSchema.Type

export const ProductSpecOperationErrorSchema = Schema.Struct({
  ok: Schema.Literal(false),
  reason: Schema.Literals([
    "invalid_request",
    "not_found",
    "read_failed",
    "write_failed",
    "not_executable",
    "revision_mismatch",
    "invalid_plan",
    "plan_not_accepted",
    "packet_not_found",
    "dependency_not_verified",
    "lease_conflict",
    "invalid_transition",
    "evidence_required",
    "verifier_required",
    "revision_not_incremented",
    "proposal_stale",
  ]),
  message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
})
export type ProductSpecOperationError = typeof ProductSpecOperationErrorSchema.Type

export const ProductSpecOpenRequestSchema = Schema.Struct({
  workContextRef: RefSchema,
  relativePath: RelativeSpecPathSchema,
})
export type ProductSpecOpenRequest = typeof ProductSpecOpenRequestSchema.Type

export const ProductSpecCreateRequestSchema = Schema.Struct({
  workContextRef: RefSchema,
  relativePath: RelativeSpecPathSchema,
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  author: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200))),
})
export type ProductSpecCreateRequest = typeof ProductSpecCreateRequestSchema.Type

export const ProductSpecPlanPacketInputSchema = Schema.Struct({
  packetRef: RefSchema,
  title: NonEmptyTextSchema,
  criterionIds: Schema.Array(CriterionIdSchema),
  dependencyRefs: Schema.Array(RefSchema),
  allocation: Schema.Literals(["root", "child"]),
})
export type ProductSpecPlanPacketInput = typeof ProductSpecPlanPacketInputSchema.Type

export const ProductSpecPlanProposalRequestSchema = Schema.Struct({
  workContextRef: RefSchema,
  spec: ProductSpecIdentitySchema,
  packets: Schema.Array(ProductSpecPlanPacketInputSchema),
  deferredCriterionIds: Schema.Array(CriterionIdSchema),
})
export type ProductSpecPlanProposalRequest = typeof ProductSpecPlanProposalRequestSchema.Type

export const ProductSpecPlanAcceptRequestSchema = Schema.Struct({
  planRef: RefSchema,
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecPlanAcceptRequest = typeof ProductSpecPlanAcceptRequestSchema.Type

export const ProductSpecEditProposalRequestSchema = Schema.Struct({
  workContextRef: RefSchema,
  expectedCurrent: ProductSpecIdentitySchema,
  proposedMarkdown: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000_000)),
})
export type ProductSpecEditProposalRequest = typeof ProductSpecEditProposalRequestSchema.Type

export const ProductSpecEditConfirmRequestSchema = Schema.Struct({
  proposalRef: RefSchema,
  expectedCurrent: ProductSpecIdentitySchema,
  criterionDisposition: Schema.Literal("supersede_affected_packets"),
})
export type ProductSpecEditConfirmRequest = typeof ProductSpecEditConfirmRequestSchema.Type

export const ProductSpecPacketAdmitRequestSchema = Schema.Struct({
  runRef: RefSchema,
  packetRef: RefSchema,
  leaseRef: RefSchema,
  executorRef: RefSchema,
  executionMode: Schema.Literals(["owner-present", "afk"]),
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecPacketAdmitRequest = typeof ProductSpecPacketAdmitRequestSchema.Type

export const ProductSpecPacketBlockRequestSchema = Schema.Struct({
  runRef: RefSchema,
  packetRef: RefSchema,
  leaseRef: RefSchema,
  reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecPacketBlockRequest = typeof ProductSpecPacketBlockRequestSchema.Type

export const ProductSpecPacketDispositionRequestSchema = Schema.Struct({
  runRef: RefSchema,
  packetRef: RefSchema,
  disposition: Schema.Literals(["failed", "cancelled", "superseded"]),
  reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecPacketDispositionRequest = typeof ProductSpecPacketDispositionRequestSchema.Type

export const ProductSpecEvidenceRequestSchema = Schema.Struct({
  runRef: RefSchema,
  packetRef: RefSchema,
  leaseRef: RefSchema,
  evidenceRef: RefSchema,
  evidenceKind: ProductSpecEvidenceKindSchema,
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecEvidenceRequest = typeof ProductSpecEvidenceRequestSchema.Type

export const ProductSpecVerificationRequestSchema = Schema.Struct({
  runRef: RefSchema,
  packetRef: RefSchema,
  verifierRef: RefSchema,
  outputRef: RefSchema,
  evidenceReceiptRefs: Schema.Array(RefSchema).check(Schema.isMinLength(1)),
  expectedSpec: ProductSpecIdentitySchema,
})
export type ProductSpecVerificationRequest = typeof ProductSpecVerificationRequestSchema.Type

export const ProductSpecRunGetRequestSchema = Schema.Struct({ runRef: RefSchema })
export type ProductSpecRunGetRequest = typeof ProductSpecRunGetRequestSchema.Type

const operationResult = <A>(schema: Schema.Schema<A>) => Schema.Union([
  Schema.Struct({ ok: Schema.Literal(true), value: schema, reconciled: Schema.optional(Schema.Boolean) }),
  ProductSpecOperationErrorSchema,
])

export const ProductSpecProjectionResultSchema = operationResult(ProductSpecProjectionSchema)
export const ProductSpecPlanResultSchema = operationResult(ProductSpecPlanSchema)
export const ProductSpecRunResultSchema = operationResult(ProductSpecRunSchema)
export const ProductSpecEditProposalResultSchema = operationResult(ProductSpecEditProposalSchema)
export const ProductSpecEditConfirmationResultSchema = operationResult(ProductSpecEditConfirmationSchema)

const decode = <A>(schema: any, value: unknown): A | null => {
  try {
    return Schema.decodeUnknownSync(schema)(value) as A
  } catch {
    return null
  }
}

export const decodeProductSpecOpenRequest = (value: unknown) => decode<ProductSpecOpenRequest>(ProductSpecOpenRequestSchema, value)
export const decodeProductSpecCreateRequest = (value: unknown) => decode<ProductSpecCreateRequest>(ProductSpecCreateRequestSchema, value)
export const decodeProductSpecPlanProposalRequest = (value: unknown) => decode<ProductSpecPlanProposalRequest>(ProductSpecPlanProposalRequestSchema, value)
export const decodeProductSpecPlanAcceptRequest = (value: unknown) => decode<ProductSpecPlanAcceptRequest>(ProductSpecPlanAcceptRequestSchema, value)
export const decodeProductSpecEditProposalRequest = (value: unknown) => decode<ProductSpecEditProposalRequest>(ProductSpecEditProposalRequestSchema, value)
export const decodeProductSpecEditConfirmRequest = (value: unknown) => decode<ProductSpecEditConfirmRequest>(ProductSpecEditConfirmRequestSchema, value)
export const decodeProductSpecPacketAdmitRequest = (value: unknown) => decode<ProductSpecPacketAdmitRequest>(ProductSpecPacketAdmitRequestSchema, value)
export const decodeProductSpecPacketBlockRequest = (value: unknown) => decode<ProductSpecPacketBlockRequest>(ProductSpecPacketBlockRequestSchema, value)
export const decodeProductSpecPacketDispositionRequest = (value: unknown) => decode<ProductSpecPacketDispositionRequest>(ProductSpecPacketDispositionRequestSchema, value)
export const decodeProductSpecEvidenceRequest = (value: unknown) => decode<ProductSpecEvidenceRequest>(ProductSpecEvidenceRequestSchema, value)
export const decodeProductSpecVerificationRequest = (value: unknown) => decode<ProductSpecVerificationRequest>(ProductSpecVerificationRequestSchema, value)
export const decodeProductSpecRunGetRequest = (value: unknown) => decode<ProductSpecRunGetRequest>(ProductSpecRunGetRequestSchema, value)
export const decodeProductSpecProjectionResult = (value: unknown) => decode(ProductSpecProjectionResultSchema, value)
export const decodeProductSpecPlanResult = (value: unknown) => decode(ProductSpecPlanResultSchema, value)
export const decodeProductSpecRunResult = (value: unknown) => decode(ProductSpecRunResultSchema, value)
export const decodeProductSpecEditProposalResult = (value: unknown) => decode(ProductSpecEditProposalResultSchema, value)
export const decodeProductSpecEditConfirmationResult = (value: unknown) => decode(ProductSpecEditConfirmationResultSchema, value)
