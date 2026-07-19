import { Schema } from "effect"

export const IdeBasicIdeAcceptanceSchemaVersion = Schema.Literal(
  "openagents.desktop.ide-basic-ide-acceptance.v1",
)

export const IdeBasicIdeTimestampSchema = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
)

export const IdeBasicIdeCommitShaSchema = Schema.String.check(
  Schema.isPattern(/^[0-9a-f]{40}$/u),
)

export const IdeBasicIdeSha256Schema = Schema.String.check(
  Schema.isPattern(/^[0-9a-f]{64}$/u),
)

export const IdeBasicIdeMatrixIdSchema = Schema.Literals([
  "finder_cold_open",
  "explorer_at_scale",
  "rapid_switching",
  "editing_and_recovery",
  "conflict",
  "search_and_navigation",
  "versioned_review",
  "language_bursts",
  "vim_on_off",
  "keyboard_and_assistive_tech",
  "visual_and_accessibility",
  "offline_and_failure",
  "resource_disposal",
  "rollback",
  "chat_only_launch",
])
export type IdeBasicIdeMatrixId = typeof IdeBasicIdeMatrixIdSchema.Type

export const IdeBasicIdeMatrixEvidenceSchema = Schema.Struct({
  matrixId: IdeBasicIdeMatrixIdSchema,
  passed: Schema.Literal(true),
  evidenceRefs: Schema.Array(
    Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(320)),
  ).check(Schema.isMinLength(1), Schema.isMaxLength(20)),
  disposition: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
})
export type IdeBasicIdeMatrixEvidence = typeof IdeBasicIdeMatrixEvidenceSchema.Type

export const IdeBasicIdeMetricSchema = Schema.Struct({
  metric: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180)),
  unit: Schema.Literals(["milliseconds", "bytes", "count"]),
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  p50: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p95: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  p99: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  thresholdP95: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  thresholdP99: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  baselineP95: Schema.NullOr(Schema.Number.check(Schema.isGreaterThanOrEqualTo(0))),
  method: Schema.Literal("linear interpolation over ascending samples"),
  noise: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500)),
  passed: Schema.Literal(true),
})
export type IdeBasicIdeMetric = typeof IdeBasicIdeMetricSchema.Type

export const IdeBasicIdeArtifactSchema = Schema.Struct({
  target: Schema.Literal("darwin-arm64"),
  candidateCommitSha: IdeBasicIdeCommitShaSchema,
  packageVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  electronVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  artifactRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(320)),
  artifactTreeSha256: IdeBasicIdeSha256Schema,
  artifactFiles: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  artifactBytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  platform: Schema.Literal("darwin"),
  architecture: Schema.Literal("arm64"),
  osRelease: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  hardwareClass: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  packaged: Schema.Literal(true),
})
export type IdeBasicIdeArtifact = typeof IdeBasicIdeArtifactSchema.Type

export const IdeBasicIdeChatOnlyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-chat-only-packaged.v1"),
  capturedAt: IdeBasicIdeTimestampSchema,
  candidateCommitSha: IdeBasicIdeCommitShaSchema,
  artifactTreeSha256: IdeBasicIdeSha256Schema,
  repetitions: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(3)),
  shellReadyMs: Schema.Struct({
    p50: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    p95: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
    p99: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0)),
  }),
  editorAssetsRequested: Schema.Literal(0),
  rendererWorkers: Schema.Literal(0),
  monacoHosts: Schema.Literal(0),
  pierreTrees: Schema.Literal(0),
  languagePlacements: Schema.Literal(0),
  projectIndexSurfaces: Schema.Literal(0),
  rootWithheld: Schema.Literal(true),
  appProcessesAfter: Schema.Literal(0),
})
export type IdeBasicIdeChatOnlyReceipt = typeof IdeBasicIdeChatOnlyReceiptSchema.Type

export const IdeBasicIdePackagedJourneyReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-basic-ide-packaged-journey.v1"),
  capturedAt: IdeBasicIdeTimestampSchema,
  candidateCommitSha: IdeBasicIdeCommitShaSchema,
  artifactTreeSha256: IdeBasicIdeSha256Schema,
  target: Schema.Literal("darwin-arm64"),
  finderColdOpen: Schema.Literal(true),
  tokyoNightBeforeEditorReady: Schema.Literal(true),
  editorReady: Schema.Literal(true),
  explorerReady: Schema.Literal(true),
  quickOpenReady: Schema.Literal(true),
  edited: Schema.Literal(true),
  recoveryReloaded: Schema.Literal(true),
  vimToggled: Schema.Literal(true),
  splitViews: Schema.Literal(2),
  reviewReady: Schema.Literal(true),
  documentLocalWorkerReady: Schema.Literal(true),
  projectLanguageReady: Schema.Literal(true),
  problemsReady: Schema.Literal(true),
  outlineReady: Schema.Literal(true),
  offlinePrivateScheme: Schema.Literal(true),
  rootWithheld: Schema.Literal(true),
  legacyTextareaAbsent: Schema.Literal(true),
  resourcesAfterClose: Schema.Struct({
    models: Schema.Literal(0),
    views: Schema.Literal(0),
    workers: Schema.Literal(0),
    listeners: Schema.Literal(0),
  }),
  screenshotRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(320)),
})
export type IdeBasicIdePackagedJourneyReceipt = typeof IdeBasicIdePackagedJourneyReceiptSchema.Type

export const IdeBasicIdeArchitectureAuditSchema = Schema.Struct({
  oneSchemaSourcePerBoundary: Schema.Literal(true),
  contextServiceAndLayerEffect: Schema.Literal(true),
  namedEffectFunctions: Schema.Literal(true),
  taggedSchemaErrors: Schema.Literal(true),
  decodedRendererInputs: Schema.Literal(true),
  scopedLifecycle: Schema.Literal(true),
  rendererOwnsNoAuthority: Schema.Literal(true),
  nativeOwnsNoAuthority: Schema.Literal(true),
  tokyoNightOnly: Schema.Literal(true),
  vimFirstPartyOffByDefaultDisposable: Schema.Literal(true),
  noRemoteIndexOrUploadOrTelemetryExpansion: Schema.Literal(true),
  publicSafeEvidence: Schema.Literal(true),
})

export const IdeBasicIdeChildEvidenceSchema = Schema.Struct({
  packet: Schema.Literals(["IDE-00", "IDE-01", "IDE-02", "IDE-03", "IDE-04", "IDE-05", "IDE-06"]),
  issue: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 9015, maximum: 9021 })),
  state: Schema.Literal("CLOSED"),
  evidenceRefs: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(320))).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(20),
  ),
})

export const IdeBasicIdeAcceptanceReceiptSchema = Schema.Struct({
  schemaVersion: IdeBasicIdeAcceptanceSchemaVersion,
  generatedAt: IdeBasicIdeTimestampSchema,
  claim: Schema.Literal("OpenAgents basic IDE"),
  candidate: IdeBasicIdeArtifactSchema,
  childEvidence: Schema.Array(IdeBasicIdeChildEvidenceSchema).check(Schema.isLengthBetween(7, 7)),
  matrix: Schema.Array(IdeBasicIdeMatrixEvidenceSchema).check(Schema.isLengthBetween(15, 15)),
  metrics: Schema.Array(IdeBasicIdeMetricSchema).check(Schema.isMinLength(10), Schema.isMaxLength(100)),
  chatOnly: IdeBasicIdeChatOnlyReceiptSchema,
  architecture: IdeBasicIdeArchitectureAuditSchema,
  rollback: Schema.Struct({
    targetCommitSha: IdeBasicIdeCommitShaSchema,
    settingsSchemaCompatible: Schema.Literal(true),
    recoverySchemaCompatible: Schema.Literal(true),
    dependencyRemovalDocumented: Schema.Literal(true),
    updateRollbackCorpusPassed: Schema.Literal(true),
  }),
  targets: Schema.Struct({
    claimed: Schema.Tuple([Schema.Literal("darwin-arm64")]),
    unavailable: Schema.Array(Schema.Struct({
      target: Schema.Literals(["darwin-x64", "win32-arm64", "win32-x64", "linux-arm64", "linux-x64"]),
      reason: Schema.Literal("No IDE-07 packaged candidate was evaluated for this target."),
    })).check(Schema.isLengthBetween(5, 5)),
  }),
  review: Schema.Struct({
    reviewerClass: Schema.Literal("deterministic_repository_oracle"),
    oracleRef: Schema.Literal("apps/openagents-desktop/scripts/ide-basic-ide-acceptance.ts"),
    producerCanOverride: Schema.Literal(false),
    disposition: Schema.Literal("pass"),
  }),
  laterGaps: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240))).check(
    Schema.isMinLength(12),
    Schema.isMaxLength(30),
  ),
  assertions: Schema.Array(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(500))).check(
    Schema.isMinLength(5),
    Schema.isMaxLength(30),
  ),
})
export type IdeBasicIdeAcceptanceReceipt = typeof IdeBasicIdeAcceptanceReceiptSchema.Type

export const decodeIdeBasicIdeAcceptanceReceipt = Schema.decodeUnknownEffect(
  IdeBasicIdeAcceptanceReceiptSchema,
)
