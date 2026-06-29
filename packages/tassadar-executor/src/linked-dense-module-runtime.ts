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
export const TASSADAR_ALM_LINKED_DENSE_CONSUMER_FAMILY =
  "tassadar_linked_dense_marketplace_v1"
export const TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS =
  "compiled dense ALM module composition / exact replay gate"
export const TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE =
  "benchmark_gated_internal"
export const TASSADAR_ALM_LINKED_DENSE_EXPECTED_COMPATIBILITY_DIGESTS = {
  "tassadar_dense_memory_roundtrip_v1@1.0.0":
    "7873bf9e8f60675c7fcae4bf077f240514a8f2a14733d29c800531f59c6a2389",
  "tassadar_dense_mul_add_v1@1.0.0":
    "7383efa5fc20908b610c46cd015fe56a4bf7e793ac76ecddfaa7bf3e4ca72ad7",
} as const

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
  rollback_detail?: string
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
  bankId: string
  caseId: string
  moduleRef: string
  programId: string
  denseModuleDigest: string
  expectedTraceDigest: string
  sourceTraceDigest: string
  projectedTraceDigest: string | null
  projectedRowsMatchSource: boolean
  verified: boolean
  blockerRefs: ReadonlyArray<string>
}>

export type TassadarLinkedDenseLinkCompatibilityVerification = Readonly<{
  consumerFamily: string
  requestedModuleRefs: ReadonlyArray<string>
  selectedModuleRefs: ReadonlyArray<string>
  dependencyGraphDigest: string
  recomputedDependencyGraphDigest: string
  linkResolutionDigest: string
  recomputedLinkResolutionDigest: string
  nodeCount: number
  edgeCount: number
  verified: boolean
  blockerRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
}>

export type TassadarLinkedDenseComposedVerification = Readonly<{
  expectedTraceDigest: string
  replayedTraceDigest: string | null
  verified: boolean
  blockerRefs: ReadonlyArray<string>
}>

export type TassadarLinkedDenseReplayVerification = Readonly<{
  verifierId: "tassadar.linked_dense.replay_verifier.ts.v1"
  linkedModuleDigest: string
  composedTraceDigest: string | null
  expectedComposedTraceDigest: string
  replayVerificationCleared: boolean
  compositionVerificationCleared: boolean
  composedVerification: TassadarLinkedDenseComposedVerification
  linkCompatibility: TassadarLinkedDenseLinkCompatibilityVerification
  constituentVerifications: ReadonlyArray<TassadarLinkedDenseConformanceVerdict>
  conformanceCases: ReadonlyArray<TassadarLinkedDenseConformanceVerdict>
  blockerRefs: ReadonlyArray<string>
  compositionReceiptRefs: ReadonlyArray<string>
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
  compositionVerificationCleared: boolean
  constituentVerificationCount: number
  linkCompatibilityVerified: boolean
  settlementClaimAllowed: boolean
  purchaseSettlementAllowed: boolean
  state: TassadarCompiledWeightModuleListingState
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  compileReceiptRefs: ReadonlyArray<string>
  replayReceiptRefs: ReadonlyArray<string>
  compositionReceiptRefs: ReadonlyArray<string>
  linkCompatibilityReceiptRefs: ReadonlyArray<string>
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
const hexDigestPattern = /^[0-9a-f]{64}$/

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

const moduleIdFromRef = (moduleRef: string): string =>
  moduleRef.split("@", 1)[0] ?? moduleRef

const textBytes = (value: string): Uint8Array => new TextEncoder().encode(value)

const sha256Hex = async (chunks: ReadonlyArray<Uint8Array>): Promise<string> => {
  let total = 0
  for (const chunk of chunks) total += chunk.length
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }
  const digest = await crypto.subtle.digest("SHA-256", joined)
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
}

const stableJsonDigest = async (
  prefix: string,
  value: unknown,
): Promise<string> => sha256Hex([textBytes(prefix), textBytes(JSON.stringify(value))])

const orderedDependencyGraphForDigest = (
  graph: TassadarLinkedDenseDependencyGraph,
  graphDigest: string,
) => ({
  consumer_family: graph.consumer_family,
  nodes: graph.nodes.map((node) => ({
    module_ref: node.module_ref,
    module_id: node.module_id,
    trust_posture: node.trust_posture,
    claim_class: node.claim_class,
    compatibility_digest: node.compatibility_digest,
  })),
  edges: graph.edges.map((edge) => ({
    importer_module_ref: edge.importer_module_ref,
    import_symbol: edge.import_symbol,
    provider_module_ref: edge.provider_module_ref,
    provider_export_symbol: edge.provider_export_symbol,
  })),
  graph_digest: graphDigest,
})

const recomputeDependencyGraphDigest = (
  graph: TassadarLinkedDenseDependencyGraph,
): Promise<string> =>
  stableJsonDigest(
    "psionic_tassadar_module_dependency_graph|",
    orderedDependencyGraphForDigest(graph, ""),
  )

const orderedLinkResolutionForDigest = (
  resolution: TassadarLinkedDenseLinkResolution,
) => {
  const base = {
    consumer_family: resolution.consumer_family,
    requested_module_refs: resolution.requested_module_refs,
    selected_module_refs: resolution.selected_module_refs,
    posture: resolution.posture,
    dependency_graph: orderedDependencyGraphForDigest(
      resolution.dependency_graph,
      resolution.dependency_graph.graph_digest,
    ),
  }
  return resolution.rollback_detail === undefined
    ? { ...base, resolution_digest: "" }
    : {
        ...base,
        rollback_detail: resolution.rollback_detail,
        resolution_digest: "",
      }
}

const recomputeLinkResolutionDigest = (
  resolution: TassadarLinkedDenseLinkResolution,
): Promise<string> =>
  stableJsonDigest(
    "psionic_tassadar_module_link_resolution|",
    orderedLinkResolutionForDigest(resolution),
  )

const arraysEqual = (
  left: ReadonlyArray<string>,
  right: ReadonlyArray<string>,
): boolean =>
  left.length === right.length &&
  left.every((value, index) => value === right[index])

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].sort()

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

const verifyTassadarLinkedDenseLinkCompatibility = async (
  fixture: TassadarLinkedDenseProgramFixture,
): Promise<TassadarLinkedDenseLinkCompatibilityVerification> => {
  const resolution = fixture.linkedModule.linkResolution
  const graph = resolution.dependency_graph
  const bankRefs = fixture.linkedModule.banks.map((bank) => bank.moduleRef)
  const graphNodeRefs = graph.nodes.map((node) => node.module_ref)
  const graphEdgeRefs = graph.edges.flatMap((edge) => [
    edge.importer_module_ref,
    edge.provider_module_ref,
  ])
  const recomputedDependencyGraphDigest =
    await recomputeDependencyGraphDigest(graph)
  const recomputedLinkResolutionDigest =
    await recomputeLinkResolutionDigest(resolution)
  const missingNodeRefs = bankRefs.filter(
    (moduleRef) => !graphNodeRefs.includes(moduleRef),
  )
  const extraNodeRefs = graphNodeRefs.filter(
    (moduleRef) => !bankRefs.includes(moduleRef),
  )
  const unknownEdgeRefs = uniqueSorted(
    graphEdgeRefs.filter((moduleRef) => !bankRefs.includes(moduleRef)),
  )
  const duplicateNodeRefs =
    new Set(graphNodeRefs).size === graphNodeRefs.length
      ? []
      : ["blocker.public.tassadar_compiled_module.link_duplicate_node_ref"]
  const firstBankRef = bankRefs[0]
  const laterBankRefs = bankRefs.slice(1)
  const hasDependencyFromLaterBankToFirst =
    firstBankRef !== undefined &&
    graph.edges.some(
      (edge) =>
        edge.provider_module_ref === firstBankRef &&
        laterBankRefs.includes(edge.importer_module_ref),
    )

  const nodeBlockers = graph.nodes.flatMap((node) => {
    const expectedCompatibilityDigest =
      TASSADAR_ALM_LINKED_DENSE_EXPECTED_COMPATIBILITY_DIGESTS[
        node.module_ref as keyof typeof TASSADAR_ALM_LINKED_DENSE_EXPECTED_COMPATIBILITY_DIGESTS
      ]
    return [
      ...(node.module_id === moduleIdFromRef(node.module_ref)
        ? []
        : ["blocker.public.tassadar_compiled_module.link_node_id_mismatch"]),
      ...(node.trust_posture ===
      TASSADAR_ALM_LINKED_DENSE_REQUIRED_TRUST_POSTURE
        ? []
        : [
            "blocker.public.tassadar_compiled_module.link_trust_posture_refused",
          ]),
      ...(node.claim_class === TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS
        ? []
        : ["blocker.public.tassadar_compiled_module.link_claim_class_refused"]),
      ...(hexDigestPattern.test(node.compatibility_digest)
        ? []
        : [
            "blocker.public.tassadar_compiled_module.link_compatibility_digest_invalid",
          ]),
      ...(expectedCompatibilityDigest !== undefined &&
      node.compatibility_digest === expectedCompatibilityDigest
        ? []
        : [
            "blocker.public.tassadar_compiled_module.link_compatibility_digest_mismatch",
          ]),
    ]
  })
  const edgeBlockers = graph.edges.flatMap((edge) => [
    ...(edge.import_symbol.length > 0 &&
    edge.provider_export_symbol.length > 0
      ? []
      : ["blocker.public.tassadar_compiled_module.link_edge_symbol_missing"]),
    ...(edge.import_symbol === edge.provider_export_symbol
      ? []
      : ["blocker.public.tassadar_compiled_module.link_edge_symbol_mismatch"]),
  ])
  const blockerRefs = [
    ...(resolution.consumer_family === TASSADAR_ALM_LINKED_DENSE_CONSUMER_FAMILY
      ? []
      : ["blocker.public.tassadar_compiled_module.link_consumer_mismatch"]),
    ...(graph.consumer_family === TASSADAR_ALM_LINKED_DENSE_CONSUMER_FAMILY
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_graph_consumer_mismatch",
        ]),
    ...(resolution.posture === "exact"
      ? []
      : ["blocker.public.tassadar_compiled_module.link_posture_not_exact"]),
    ...(arraysEqual(resolution.requested_module_refs, bankRefs)
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_requested_refs_mismatch",
        ]),
    ...(arraysEqual(resolution.selected_module_refs, bankRefs)
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_selected_refs_mismatch",
        ]),
    ...(graph.nodes.length === bankRefs.length
      ? []
      : ["blocker.public.tassadar_compiled_module.link_node_count_mismatch"]),
    ...missingNodeRefs.map(
      () => "blocker.public.tassadar_compiled_module.link_node_missing",
    ),
    ...extraNodeRefs.map(
      () => "blocker.public.tassadar_compiled_module.link_node_unknown",
    ),
    ...duplicateNodeRefs,
    ...(unknownEdgeRefs.length === 0
      ? []
      : ["blocker.public.tassadar_compiled_module.link_edge_unknown_module"]),
    ...(hasDependencyFromLaterBankToFirst
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_dependency_edge_not_conformant",
        ]),
    ...nodeBlockers,
    ...edgeBlockers,
    ...(graph.graph_digest === recomputedDependencyGraphDigest
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_dependency_graph_digest_mismatch",
        ]),
    ...(resolution.resolution_digest === recomputedLinkResolutionDigest
      ? []
      : [
          "blocker.public.tassadar_compiled_module.link_resolution_digest_mismatch",
        ]),
  ].sort()
  const verified = blockerRefs.length === 0

  return {
    blockerRefs,
    consumerFamily: resolution.consumer_family,
    dependencyGraphDigest: graph.graph_digest,
    edgeCount: graph.edges.length,
    linkResolutionDigest: resolution.resolution_digest,
    nodeCount: graph.nodes.length,
    receiptRefs: verified
      ? [
          `receipt.openagents.tassadar_link_compatibility.${resolution.resolution_digest.slice(0, 16)}`,
        ]
      : [],
    recomputedDependencyGraphDigest,
    recomputedLinkResolutionDigest,
    requestedModuleRefs: resolution.requested_module_refs,
    selectedModuleRefs: resolution.selected_module_refs,
    verified,
  }
}

export const verifyTassadarLinkedDenseComposition = async (
  fixture: TassadarLinkedDenseProgramFixture,
): Promise<TassadarLinkedDenseReplayVerification> => {
  const structureBlockers = structureBlockerRefs(fixture)
  const linkCompatibility =
    await verifyTassadarLinkedDenseLinkCompatibility(fixture)
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
      compositionReceiptRefs: [],
      compositionVerificationCleared: false,
      composedVerification: {
        blockerRefs: [
          "blocker.public.tassadar_compiled_module.composed_execution_refused",
        ],
        expectedTraceDigest: fixture.composedTraceDigest,
        replayedTraceDigest: null,
        verified: false,
      },
      constituentVerifications: [],
      expectedComposedTraceDigest: fixture.composedTraceDigest,
      linkCompatibility,
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
  const composedVerification = {
    blockerRefs: composedBlockers,
    expectedTraceDigest: fixture.composedTraceDigest,
    replayedTraceDigest: composedTrace.traceDigest,
    verified: composedBlockers.length === 0,
  } satisfies TassadarLinkedDenseComposedVerification

  const verdicts = await Promise.all(
    fixture.linkedModule.banks.map(async (bank) => {
      const expectedCase = findFixtureCase(fixture, bank)
      let sourceTrace: TassadarNumericTrace
      try {
        sourceTrace = await executeTassadarDenseWeightModule(
          bank.denseModule,
          fixture.steps,
        )
      } catch {
        const blockerRefs = [
          ...(expectedCase === undefined
            ? [
                "blocker.public.tassadar_compiled_module.conformance_case_missing",
              ]
            : []),
          "blocker.public.tassadar_compiled_module.constituent_execution_refused",
        ]
        return {
          bankId: bank.bankId,
          blockerRefs,
          caseId:
            expectedCase?.caseId ??
            `conformance.linked_dense.${bank.programId}.missing`,
          denseModuleDigest: bank.denseModuleDigest,
          expectedTraceDigest: bank.expectedTraceDigest,
          moduleRef: publicModuleRef(bank.moduleRef),
          programId: bank.programId,
          projectedRowsMatchSource: false,
          projectedTraceDigest: null,
          sourceTraceDigest: "",
          verified: false,
        } satisfies TassadarLinkedDenseConformanceVerdict
      }
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
        bankId: bank.bankId,
        blockerRefs,
        caseId:
          expectedCase?.caseId ??
          `conformance.linked_dense.${bank.programId}.missing`,
        denseModuleDigest: bank.denseModuleDigest,
        expectedTraceDigest: bank.expectedTraceDigest,
        moduleRef: publicModuleRef(bank.moduleRef),
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
    ...linkCompatibility.blockerRefs,
    ...verdicts.flatMap((verdict) => verdict.blockerRefs),
  ].sort()
  const compositionVerificationCleared =
    blockerRefs.length === 0 &&
    composedVerification.verified &&
    linkCompatibility.verified &&
    verdicts.every((verdict) => verdict.verified)
  const replayVerificationCleared = compositionVerificationCleared

  return {
    blockerRefs,
    composedTraceDigest: composedTrace.traceDigest,
    composedVerification,
    conformanceCases: verdicts,
    compositionReceiptRefs: compositionVerificationCleared
      ? [
          `receipt.openagents.tassadar_linked_dense_composition.${fixture.linkedModuleDigest.slice(0, 16)}`,
        ]
      : [],
    compositionVerificationCleared,
    constituentVerifications: verdicts,
    expectedComposedTraceDigest: fixture.composedTraceDigest,
    linkCompatibility,
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
    verification.compositionVerificationCleared &&
    purchased &&
    settlementReceiptPresent
  const state: TassadarCompiledWeightModuleListingState =
    !verification.compositionVerificationCleared
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
    ...(verification.compositionVerificationCleared
      ? []
      : [
          "blocker.public.tassadar_compiled_module.composition_verification_missing",
        ]),
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
      "caveat.public.tassadar_compiled_module.composition_verification_required_before_settlement",
      "caveat.public.tassadar_compiled_module.no_real_money_moved_by_listing",
    ],
    claimBoundary: input.fixture.claimBoundary,
    compileReceiptRefs: safeRefs(
      "compileReceiptRefs",
      input.fixture.compileReceiptRefs,
    ),
    composedDenseModuleDigest: input.fixture.composedDenseModuleDigest,
    composedTraceDigest: input.fixture.composedTraceDigest,
    compositionReceiptRefs: verification.compositionReceiptRefs,
    compositionVerificationCleared:
      verification.compositionVerificationCleared,
    constituentVerificationCount: verification.constituentVerifications.length,
    dependencyEdgeCount:
      input.fixture.linkedModule.linkResolution.dependency_graph.edges.length,
    linkCompatibilityReceiptRefs: verification.linkCompatibility.receiptRefs,
    linkCompatibilityVerified: verification.linkCompatibility.verified,
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
