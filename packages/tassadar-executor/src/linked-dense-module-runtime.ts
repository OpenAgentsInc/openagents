import {
  digestTassadarNumericTraceRows,
  type TassadarNumericTrace,
} from "./numeric-executor.js"
import {
  executeTassadarDenseWeightModule,
  type TassadarDenseWeightModule,
} from "./dense-weight-module-runtime.js"

export const TASSADAR_ALM_LINKED_DENSE_MODULE_KIND =
  "tassadar_alm_linked_dense_module.v1"
export const TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST =
  "cc1403674fc0d38892610d9e9c6c9230075494061f720c45bfa4f7b5a961756a"
export const TASSADAR_ALM_LINKED_DENSE_COMPOSED_MODULE_DIGEST =
  "2f3fa15120f0a078d4ede4e074e288fed24533ffa46f2d4b8aa4ca418c876602"
export const TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST =
  "0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2"
export const TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF =
  "listing.public.tassadar_compiled_weight_module.cc1403674fc0d388"

export type TassadarLinkedDenseDependencyNode = Readonly<{
  module_ref: string
  module_id: string
  trust_posture: string
  claim_class: string
  compatibility_digest: string
}>

export type TassadarLinkedDenseDependencyEdge = Readonly<{
  importer_module_ref: string
  import_symbol: string
  provider_module_ref: string
  provider_export_symbol: string
}>

export type TassadarLinkedDenseDependencyGraph = Readonly<{
  consumer_family: string
  nodes: ReadonlyArray<TassadarLinkedDenseDependencyNode>
  edges: ReadonlyArray<TassadarLinkedDenseDependencyEdge>
  graph_digest: string
}>

export type TassadarLinkedDenseLinkResolution = Readonly<{
  consumer_family: string
  requested_module_refs: ReadonlyArray<string>
  selected_module_refs: ReadonlyArray<string>
  dependency_graph: TassadarLinkedDenseDependencyGraph
  posture: string
  resolution_digest: string
}>

export type TassadarLinkedDenseBank = Readonly<{
  bankId: string
  moduleRef: string
  programId: string
  workloadKind: string
  profileId: string
  denseModuleDigest: string
  numericModelDigest: string
  expectedTraceDigest: string
  slotOffset: number
  channelOffset: number
  projectedOutputStart: number
  projectedOutputEnd: number
  compileReceiptRefs: ReadonlyArray<string>
  denseModule: TassadarDenseWeightModule
}>

export type TassadarLinkedDenseConformanceCase = Readonly<{
  caseId: string
  programId: string
  denseModuleDigest: string
  sourceTraceDigest: string
  projectedTraceDigest: string
  projectedRowsMatchSource: boolean
  comparedStepCount: number
  projectedOutputStart: number
  projectedOutputEnd: number
}>

export type TassadarLinkedDenseModule = Readonly<{
  schemaVersion: number
  moduleKind: typeof TASSADAR_ALM_LINKED_DENSE_MODULE_KIND
  moduleId: string
  banks: ReadonlyArray<TassadarLinkedDenseBank>
  linkResolution: TassadarLinkedDenseLinkResolution
  composedDenseModule: TassadarDenseWeightModule
  claimBoundary: string
}>

export type TassadarLinkedDenseProgramFixture = Readonly<{
  schemaVersion: number
  fixtureId: string
  generatedBy: string
  claimBoundary: string
  linkedModuleDigest: string
  composedDenseModuleDigest: string
  composedModelDigest: string
  composedTraceDigest: string
  linkedModule: TassadarLinkedDenseModule
  steps: ReadonlyArray<ReadonlyArray<number>>
  expectedFinalRow: ReadonlyArray<number> | null
  conformanceCases: ReadonlyArray<TassadarLinkedDenseConformanceCase>
  compileReceiptRefs: ReadonlyArray<string>
  marketplaceArtifactRefs: ReadonlyArray<string>
}>

export type TassadarLinkedDenseConformanceVerdict = Readonly<{
  caseId: string
  programId: string
  denseModuleDigest: string
  sourceTraceDigest: string
  projectedTraceDigest: string
  projectedRowsMatchSource: boolean
  verified: boolean
  blockerRefs: ReadonlyArray<string>
}>

export type TassadarLinkedDenseReplayVerification = Readonly<{
  verifierId: "tassadar.linked_dense.replay_verifier.ts.v1"
  linkedModuleDigest: string
  composedTraceDigest: string | null
  expectedComposedTraceDigest: string
  replayVerificationCleared: boolean
  conformanceCases: ReadonlyArray<TassadarLinkedDenseConformanceVerdict>
  blockerRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
}>

export type TassadarCompiledWeightModuleListingState =
  | "blocked"
  | "replay_verified_listed"
  | "purchased_pending_settlement"
  | "settled"

export type TassadarCompiledWeightModuleListing = Readonly<{
  schemaVersion: 1
  listingKind: "tassadar_compiled_weight_module_listing.v1"
  listingRef: typeof TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF
  moduleKind: typeof TASSADAR_ALM_LINKED_DENSE_MODULE_KIND
  moduleId: string
  linkedModuleDigest: string
  composedDenseModuleDigest: string
  composedTraceDigest: string
  sourceBankCount: number
  sourceBanks: ReadonlyArray<
    Readonly<{
      programId: string
      moduleRef: string
      denseModuleDigest: string
      sourceTraceDigest: string
    }>
  >
  linkResolutionDigest: string
  dependencyEdgeCount: number
  replayVerificationCleared: boolean
  settlementClaimAllowed: boolean
  purchaseSettlementAllowed: boolean
  state: TassadarCompiledWeightModuleListingState
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  compileReceiptRefs: ReadonlyArray<string>
  replayReceiptRefs: ReadonlyArray<string>
  marketplaceArtifactRefs: ReadonlyArray<string>
  purchaseReceiptRefs: ReadonlyArray<string>
  settlementReceiptRefs: ReadonlyArray<string>
  claimBoundary: string
}>

export type TassadarCompiledWeightModuleListingInput = Readonly<{
  fixture: TassadarLinkedDenseProgramFixture
  purchaseReceiptRefs?: ReadonlyArray<string>
  settlementReceiptRefs?: ReadonlyArray<string>
}>

export class TassadarCompiledWeightModuleListingUnsafe extends Error {
  readonly field: string

  constructor(field: string, detail: string) {
    super(`${field}: ${detail}`)
    this.field = field
  }
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const rawMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|full[_-]?(prompt|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|dataset|key|repo|source|trace|wallet)|prompt[_-]?(raw|text|full)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|invoice|log|payment|payload|prompt|provider|repo|runner|run[_-]?log|source|telemetry|text|trace|usage|webhook)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|trace[_-]?(raw|full|private|payload)|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [...new Set((refs ?? []).map((ref) => ref.trim()).filter(Boolean))].sort()

const safeRefs = (
  field: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(
    (ref) =>
      !safeRefPattern.test(ref) ||
      rawMaterialPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )
  if (unsafe !== undefined) {
    throw new TassadarCompiledWeightModuleListingUnsafe(
      field,
      "refs must be public-safe and must not contain raw traces, prompts, private repos, provider payloads, customer data, wallet/payment material, secrets, or timestamps",
    )
  }
  return normalized
}

const rowsEqual = (
  left: ReadonlyArray<ReadonlyArray<bigint>>,
  right: ReadonlyArray<ReadonlyArray<bigint>>,
): boolean =>
  left.length === right.length &&
  left.every((leftRow, rowIndex) => {
    const rightRow = right[rowIndex] ?? []
    return (
      leftRow.length === rightRow.length &&
      leftRow.every((value, columnIndex) => value === rightRow[columnIndex])
    )
  })

const projectedRows = (
  trace: TassadarNumericTrace,
  start: number,
  end: number,
): ReadonlyArray<ReadonlyArray<bigint>> =>
  trace.stepOutputs.map((row) => row.slice(start, end))

const publicModuleRef = (moduleRef: string): string =>
  moduleRef.replaceAll("@", ".version.")

const findFixtureCase = (
  fixture: TassadarLinkedDenseProgramFixture,
  bank: TassadarLinkedDenseBank,
): TassadarLinkedDenseConformanceCase | undefined =>
  fixture.conformanceCases.find(
    (item) =>
      item.programId === bank.programId &&
      item.denseModuleDigest === bank.denseModuleDigest,
  )

const structureBlockerRefs = (
  fixture: TassadarLinkedDenseProgramFixture,
): ReadonlyArray<string> => [
  ...(fixture.linkedModule.moduleKind !== TASSADAR_ALM_LINKED_DENSE_MODULE_KIND
    ? ["blocker.public.tassadar_compiled_module.unsupported_module_kind"]
    : []),
  ...(fixture.linkedModuleDigest !== TASSADAR_ALM_LINKED_DENSE_MODULE_DIGEST
    ? ["blocker.public.tassadar_compiled_module.linked_digest_mismatch"]
    : []),
  ...(fixture.composedDenseModuleDigest !==
  TASSADAR_ALM_LINKED_DENSE_COMPOSED_MODULE_DIGEST
    ? ["blocker.public.tassadar_compiled_module.composed_module_digest_mismatch"]
    : []),
  ...(fixture.linkedModule.banks.length < 2
    ? ["blocker.public.tassadar_compiled_module.needs_two_dense_banks"]
    : []),
  ...(fixture.linkedModule.linkResolution.dependency_graph.edges.length < 1
    ? ["blocker.public.tassadar_compiled_module.dependency_edge_missing"]
    : []),
]

export const verifyTassadarLinkedDenseComposition = async (
  fixture: TassadarLinkedDenseProgramFixture,
): Promise<TassadarLinkedDenseReplayVerification> => {
  const structureBlockers = structureBlockerRefs(fixture)
  let composedTrace: TassadarNumericTrace
  try {
    composedTrace = await executeTassadarDenseWeightModule(
      fixture.linkedModule.composedDenseModule,
      fixture.steps,
    )
  } catch {
    return {
      blockerRefs: [
        ...structureBlockers,
        "blocker.public.tassadar_compiled_module.composed_execution_refused",
      ],
      composedTraceDigest: null,
      conformanceCases: [],
      expectedComposedTraceDigest: fixture.composedTraceDigest,
      linkedModuleDigest: fixture.linkedModuleDigest,
      receiptRefs: [],
      replayVerificationCleared: false,
      verifierId: "tassadar.linked_dense.replay_verifier.ts.v1",
    }
  }

  const composedBlockers =
    composedTrace.traceDigest === fixture.composedTraceDigest &&
    composedTrace.traceDigest === TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST
      ? []
      : ["blocker.public.tassadar_compiled_module.composed_trace_mismatch"]

  const verdicts = await Promise.all(
    fixture.linkedModule.banks.map(async (bank) => {
      const sourceTrace = await executeTassadarDenseWeightModule(
        bank.denseModule,
        fixture.steps,
      )
      const expectedCase = findFixtureCase(fixture, bank)
      const projection = projectedRows(
        composedTrace,
        bank.projectedOutputStart,
        bank.projectedOutputEnd,
      )
      const projectedTraceDigest = await digestTassadarNumericTraceRows(
        sourceTrace.graphDigest,
        projection,
      )
      const projectedRowsMatchSource = rowsEqual(
        projection,
        sourceTrace.stepOutputs,
      )
      const blockerRefs = [
        ...(expectedCase === undefined
          ? ["blocker.public.tassadar_compiled_module.conformance_case_missing"]
          : []),
        ...(sourceTrace.traceDigest !== bank.expectedTraceDigest
          ? ["blocker.public.tassadar_compiled_module.source_trace_mismatch"]
          : []),
        ...(expectedCase !== undefined &&
        sourceTrace.traceDigest !== expectedCase.sourceTraceDigest
          ? [
              "blocker.public.tassadar_compiled_module.fixture_source_trace_mismatch",
            ]
          : []),
        ...(expectedCase !== undefined &&
        projectedTraceDigest !== expectedCase.projectedTraceDigest
          ? [
              "blocker.public.tassadar_compiled_module.fixture_projected_trace_mismatch",
            ]
          : []),
        ...(!projectedRowsMatchSource
          ? [
              "blocker.public.tassadar_compiled_module.projected_rows_diverged",
            ]
          : []),
        ...(projectedTraceDigest !== sourceTrace.traceDigest
          ? [
              "blocker.public.tassadar_compiled_module.projected_trace_diverged",
            ]
          : []),
        ...(expectedCase !== undefined && !expectedCase.projectedRowsMatchSource
          ? [
              "blocker.public.tassadar_compiled_module.fixture_projection_not_verified",
            ]
          : []),
      ]

      return {
        blockerRefs,
        caseId:
          expectedCase?.caseId ??
          `conformance.linked_dense.${bank.programId}.missing`,
        denseModuleDigest: bank.denseModuleDigest,
        programId: bank.programId,
        projectedRowsMatchSource,
        projectedTraceDigest,
        sourceTraceDigest: sourceTrace.traceDigest,
        verified: blockerRefs.length === 0,
      } satisfies TassadarLinkedDenseConformanceVerdict
    }),
  )

  const blockerRefs = [
    ...structureBlockers,
    ...composedBlockers,
    ...verdicts.flatMap((verdict) => verdict.blockerRefs),
  ].sort()
  const replayVerificationCleared =
    blockerRefs.length === 0 && verdicts.every((verdict) => verdict.verified)

  return {
    blockerRefs,
    composedTraceDigest: composedTrace.traceDigest,
    conformanceCases: verdicts,
    expectedComposedTraceDigest: fixture.composedTraceDigest,
    linkedModuleDigest: fixture.linkedModuleDigest,
    receiptRefs: replayVerificationCleared
      ? [
          `receipt.openagents.tassadar_linked_dense_replay.${fixture.linkedModuleDigest.slice(0, 16)}`,
        ]
      : [],
    replayVerificationCleared,
    verifierId: "tassadar.linked_dense.replay_verifier.ts.v1",
  }
}

export const projectTassadarCompiledWeightModuleListing = async (
  input: TassadarCompiledWeightModuleListingInput,
): Promise<TassadarCompiledWeightModuleListing> => {
  const purchaseReceiptRefs = safeRefs(
    "purchaseReceiptRefs",
    input.purchaseReceiptRefs,
  )
  const settlementReceiptRefs = safeRefs(
    "settlementReceiptRefs",
    input.settlementReceiptRefs,
  )
  const verification = await verifyTassadarLinkedDenseComposition(input.fixture)
  const purchased = purchaseReceiptRefs.length > 0
  const settlementReceiptPresent = settlementReceiptRefs.length > 0
  const purchaseSettlementAllowed =
    verification.replayVerificationCleared && purchased && settlementReceiptPresent
  const state: TassadarCompiledWeightModuleListingState =
    !verification.replayVerificationCleared
      ? "blocked"
      : purchaseSettlementAllowed
        ? "settled"
        : purchased
          ? "purchased_pending_settlement"
          : "replay_verified_listed"
  const blockerRefs = [
    ...verification.blockerRefs,
    ...(verification.replayVerificationCleared
      ? []
      : ["blocker.public.tassadar_compiled_module.replay_verification_missing"]),
    ...(purchased
      ? []
      : ["blocker.public.tassadar_compiled_module.purchase_receipt_missing"]),
    ...(settlementReceiptPresent
      ? []
      : ["blocker.public.tassadar_compiled_module.settlement_receipt_missing"]),
  ].sort()

  return {
    blockerRefs,
    caveatRefs: [
      "caveat.public.tassadar_compiled_module.listing_is_not_serving",
      "caveat.public.tassadar_compiled_module.purchase_is_not_settlement",
      "caveat.public.tassadar_compiled_module.replay_verification_required_before_settlement",
      "caveat.public.tassadar_compiled_module.no_real_money_moved_by_listing",
    ],
    claimBoundary: input.fixture.claimBoundary,
    compileReceiptRefs: safeRefs(
      "compileReceiptRefs",
      input.fixture.compileReceiptRefs,
    ),
    composedDenseModuleDigest: input.fixture.composedDenseModuleDigest,
    composedTraceDigest: input.fixture.composedTraceDigest,
    dependencyEdgeCount:
      input.fixture.linkedModule.linkResolution.dependency_graph.edges.length,
    linkResolutionDigest:
      input.fixture.linkedModule.linkResolution.resolution_digest,
    linkedModuleDigest: input.fixture.linkedModuleDigest,
    listingKind: "tassadar_compiled_weight_module_listing.v1",
    listingRef: TASSADAR_COMPILED_WEIGHT_MODULE_LISTING_REF,
    marketplaceArtifactRefs: safeRefs(
      "marketplaceArtifactRefs",
      input.fixture.marketplaceArtifactRefs,
    ),
    moduleId: input.fixture.linkedModule.moduleId,
    moduleKind: TASSADAR_ALM_LINKED_DENSE_MODULE_KIND,
    purchaseReceiptRefs,
    purchaseSettlementAllowed,
    replayReceiptRefs: verification.receiptRefs,
    replayVerificationCleared: verification.replayVerificationCleared,
    schemaVersion: 1,
    settlementClaimAllowed: purchaseSettlementAllowed,
    settlementReceiptRefs,
    sourceBankCount: input.fixture.linkedModule.banks.length,
    sourceBanks: input.fixture.linkedModule.banks.map((bank) => ({
      denseModuleDigest: bank.denseModuleDigest,
      moduleRef: publicModuleRef(bank.moduleRef),
      programId: bank.programId,
      sourceTraceDigest: bank.expectedTraceDigest,
    })),
    state,
  }
}
