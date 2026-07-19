import { Schema } from "effect"

const nonNegative = Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))
const positive = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1))
const text = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_000))

export const IdeCursorBenchmarkMetricSchema = Schema.Struct({
  name: text,
  unit: Schema.Literals(["milliseconds", "bytes", "count"]),
  repetitions: positive,
  warmup: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  p50: nonNegative,
  p95: nonNegative,
  p99: nonNegative,
  thresholdP95: nonNegative,
  thresholdP99: nonNegative,
  method: text,
  noise: text,
  passed: Schema.Boolean,
}).annotate({ identifier: "IdeCursorBenchmarkMetric" })

export const IdeCursorBenchmarkReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.ide-cursor-benchmark.v1"),
  issue: Schema.Literal("IDE-09"),
  measuredAt: Schema.String,
  commitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  environment: Schema.Struct({
    platform: Schema.String,
    architecture: Schema.String,
    node: Schema.String,
    provider: text,
    model: text,
    harness: text,
    placement: text,
    indexPosture: Schema.Literals(["disabled", "local", "remote"]),
    cacheState: Schema.Literals(["cold", "warm", "mixed"]),
    cohort: Schema.Literals(["deterministic_fixture", "real_provider"]),
  }),
  corpus: Schema.Struct({
    fixtureRef: text,
    cases: positive,
    languages: Schema.Array(text).check(Schema.isMinLength(3)),
    intents: Schema.Array(Schema.Literals(["completion", "next_edit", "ask", "edit", "proposal"]))
      .check(Schema.isMinLength(5)),
    adversarialClasses: Schema.Array(text).check(Schema.isMinLength(10)),
  }),
  metrics: Schema.Array(IdeCursorBenchmarkMetricSchema).check(Schema.isMinLength(10)),
  quality: Schema.Struct({
    scoredCases: positive,
    exactMatch: nonNegative,
    acceptedSemanticMatch: nonNegative,
    syntaxPreserved: nonNegative,
    diagnosticsPreserved: nonNegative,
    deliberateNoSuggestionCorrect: nonNegative,
    hallucinatedPaths: Schema.Literal(0),
    secretUnsafeSuggestions: Schema.Literal(0),
    stalePublished: Schema.Literal(0),
    wrongIdentityPublished: Schema.Literal(0),
    unauthorizedExternalRequests: Schema.Literal(0),
  }),
  resourcesAfter: Schema.Struct({
    activeRequests: Schema.Literal(0),
    candidateModels: Schema.Literal(0),
    subscriptions: Schema.Literal(0),
    activeHandlesDelta: Schema.Number.check(Schema.isInt()),
    retainedHeapBytes: Schema.Number,
  }),
  dataFlow: Schema.Struct({
    remoteEmbeddingsRequired: Schema.Literal(false),
    providerRequests: nonNegative,
    providerBytes: nonNegative,
    otherNetworkRequests: Schema.Literal(0),
    secretsSent: Schema.Literal(false),
    publicReceiptPrivateMaterial: Schema.Literal(false),
  }),
  usage: Schema.Struct({
    inputTokens: nonNegative,
    outputTokens: nonNegative,
    costUsdMicros: nonNegative,
  }),
  budgetsPassed: Schema.Boolean,
}).annotate({ identifier: "IdeCursorBenchmarkReceipt" })

export type IdeCursorBenchmarkReceipt = typeof IdeCursorBenchmarkReceiptSchema.Type

export const IdeCursorPackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.ide-cursor-packaged-journey.v1"),
  issue: Schema.Literal("IDE-09"),
  capturedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
  target: Schema.Literal("darwin-arm64"),
  journey: Schema.Struct({
    completionRendered: Schema.Literal(true),
    partialAcceptApplied: Schema.Literal(true),
    undoRestored: Schema.Literal(true),
    nextEditRendered: Schema.Literal(true),
    askRendered: Schema.Literal(true),
    proposalSubmittedToIde08: Schema.Literal(true),
    compareAndRetryReceipted: Schema.Literal(true),
    identityDisclosed: Schema.Literal(true),
    noRemoteIndexDependencyDisclosed: Schema.Literal(true),
    keyboardOperable: Schema.Literal(true),
    focusAndEscape: Schema.Literal(true),
    vimAndTokyoNightPresent: Schema.Literal(true),
  }),
  screenshotRef: text,
  traceRef: text,
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeCursorPackagedJourneyReceipt" })
export type IdeCursorPackagedJourneyReceipt = typeof IdeCursorPackagedJourneyReceiptSchema.Type

export const IdeCursorAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.ide-cursor-acceptance.v1"),
  issue: Schema.Literal("IDE-09"),
  generatedAt: Schema.String,
  candidateCommitSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  mainEvaluationSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  artifactTreeSha256: Schema.String.check(Schema.isPattern(/^sha256:[0-9a-f]{64}$/u)),
  evidenceRefs: Schema.Array(text).check(Schema.isMinLength(5), Schema.isMaxLength(80)),
  faultMatrix: Schema.Array(Schema.Struct({
    fault: text,
    passed: Schema.Literal(true),
    evidenceRef: text,
  })).check(Schema.isMinLength(15), Schema.isMaxLength(120)),
  architecture: Schema.Struct({
    oneSchemaGraph: Schema.Literal(true),
    effectServices: Schema.Literal(true),
    rendererAuthority: Schema.Literal(false),
    providerAuthority: Schema.Literal(false),
    harnessAuthority: Schema.Literal(false),
    monacoAuthority: Schema.Literal(false),
    embeddingsRequired: Schema.Literal(false),
    silentFallback: Schema.Literal(false),
  }),
  accessibility: Schema.Struct({
    keyboard: Schema.Literal(true),
    screenReaderLabels: Schema.Literal(true),
    focusEscape: Schema.Literal(true),
    vimOnOff: Schema.Literal(true),
    imeComposition: Schema.Literal(true),
    reducedMotion: Schema.Literal(true),
    zoomAndMinimumWindow: Schema.Literal(true),
    tokyoNightNonColorCues: Schema.Literal(true),
  }),
  assuranceLifecycle: Schema.Literal("proposed"),
  ownerDisposition: Schema.Literal("unreviewed"),
  realProviderCohort: Schema.Struct({
    disposition: Schema.Literals(["passed", "not_run"]),
    reason: text,
  }),
  rollbackTargetSha: Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u)),
  claimedTargets: Schema.Tuple([Schema.Literal("darwin-arm64")]),
  laterGaps: Schema.Array(text).check(Schema.isMinLength(1), Schema.isMaxLength(30)),
  passed: Schema.Literal(true),
}).annotate({ identifier: "IdeCursorAcceptanceReceipt" })
export type IdeCursorAcceptanceReceipt = typeof IdeCursorAcceptanceReceiptSchema.Type

export const ideCursorBenchmarkThresholds = Object.freeze({
  requestToIdentityP95Ms: 25,
  requestToIdentityP99Ms: 50,
  completionFirstCandidateP95Ms: 120,
  completionFirstCandidateP99Ms: 250,
  nextEditFirstCandidateP95Ms: 175,
  nextEditFirstCandidateP99Ms: 350,
  acceptP95Ms: 16,
  acceptP99Ms: 32,
  cancelP95Ms: 16,
  cancelP99Ms: 32,
  supersedeP95Ms: 16,
  supersedeP99Ms: 32,
})
