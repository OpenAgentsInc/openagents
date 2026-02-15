// Shared page data contracts between Effect atoms/loaders and UI templates.
// Keep this file UI-free so `src/effect/**` can depend on it without pulling in `effuse-pages/**`.

export type DseOpsRunItem = {
  readonly runId: string
  readonly status: "running" | "finished" | "failed"
  readonly startedAtMs: number
  readonly endedAtMs: number | null
  readonly commitSha: string | null
  readonly baseUrl: string | null
  readonly actorUserId: string | null
  readonly signatureIds: ReadonlyArray<string> | null
  readonly updatedAtMs: number
  readonly createdAtMs: number
}

export type DseOpsRunsPageData = {
  readonly errorText: string | null
  readonly runs: ReadonlyArray<DseOpsRunItem> | null
}

export type DseOpsRunEventItem = {
  readonly tsMs: number
  readonly level: "info" | "warn" | "error"
  readonly phase: string | null
  readonly message: string
  readonly jsonPreview: string | null
}

export type DseOpsRunDetail = {
  readonly runId: string
  readonly status: "running" | "finished" | "failed"
  readonly startedAtMs: number
  readonly endedAtMs: number | null
  readonly commitSha: string | null
  readonly baseUrl: string | null
  readonly actorUserId: string | null
  readonly signatureIds: ReadonlyArray<string> | null
  readonly notes: string | null
  readonly linksJson: string | null
  readonly summaryJson: string | null
  readonly updatedAtMs: number
}

export type DseOpsRunDetailPageData = {
  readonly runId: string
  readonly errorText: string | null
  readonly run: DseOpsRunDetail | null
  readonly events: ReadonlyArray<DseOpsRunEventItem> | null
}

export type DseCompileReportDetail = {
  readonly signatureId: string
  readonly jobHash: string
  readonly datasetHash: string
  readonly datasetId: string
  readonly compiled_id: string
  readonly createdAtMs: number
  readonly jsonPretty: string
}

export type DseCompileReportPageData = {
  readonly signatureId: string
  readonly jobHash: string
  readonly datasetHash: string
  readonly errorText: string | null
  readonly report: DseCompileReportDetail | null
}

export type DseEvalReportDetail = {
  readonly signatureId: string
  readonly evalHash: string
  readonly compiled_id: string
  readonly datasetId: string
  readonly datasetHash: string
  readonly rewardId: string
  readonly rewardVersion: number
  readonly split: string | null
  readonly n: number | null
  readonly createdAtMs: number
  readonly jsonPretty: string
}

export type DseEvalReportPageData = {
  readonly signatureId: string
  readonly evalHash: string
  readonly errorText: string | null
  readonly report: DseEvalReportDetail | null
}

export type DseActivePointer = {
  readonly compiled_id: string | null
  readonly updatedAtMs: number | null
}

export type DseActiveHistoryItem = {
  readonly action: "set" | "clear" | "rollback"
  readonly fromCompiledId: string | null
  readonly toCompiledId: string | null
  readonly reason: string | null
  readonly actorUserId: string | null
  readonly createdAtMs: number
}

export type DseCanaryConfig = {
  readonly enabled: boolean
  readonly control_compiled_id: string
  readonly canary_compiled_id: string
  readonly rolloutPct: number
  readonly okCount: number
  readonly errorCount: number
  readonly minSamples: number
  readonly maxErrorRate: number
  readonly updatedAtMs: number
}

export type DseCanaryHistoryItem = {
  readonly action: "start" | "stop" | "auto_stop" | "update"
  readonly control_compiled_id: string | null
  readonly canary_compiled_id: string | null
  readonly rolloutPct: number | null
  readonly okCount: number | null
  readonly errorCount: number | null
  readonly reason: string | null
  readonly actorUserId: string | null
  readonly createdAtMs: number
}

export type DseCompileReportListItem = {
  readonly jobHash: string
  readonly datasetHash: string
  readonly compiled_id: string
  readonly createdAtMs: number
}

export type DseEvalReportListItem = {
  readonly evalHash: string
  readonly compiled_id: string
  readonly datasetHash: string
  readonly rewardId: string
  readonly split: string | null
  readonly n: number | null
  readonly createdAtMs: number
}

export type DseExampleListItem = {
  readonly exampleId: string
  readonly split: "train" | "dev" | "holdout" | "test" | null
  readonly tags: ReadonlyArray<string> | null
  readonly inputJson: string
  readonly expectedJson: string
}

export type DseReceiptListItem = {
  readonly receiptId: string
  readonly compiled_id: string
  readonly createdAtMs: number
  readonly strategyId: string | null
  readonly resultTag: "Ok" | "Error" | null
  readonly rlmTraceBlobId: string | null
  readonly rlmTraceEventCount: number | null
}

export type DseSignaturePageData = {
  readonly signatureId: string
  readonly errorText: string | null
  readonly active: DseActivePointer | null
  readonly activeHistory: ReadonlyArray<DseActiveHistoryItem> | null
  readonly canary: DseCanaryConfig | null
  readonly canaryHistory: ReadonlyArray<DseCanaryHistoryItem> | null
  readonly compileReports: ReadonlyArray<DseCompileReportListItem> | null
  readonly evalReports: ReadonlyArray<DseEvalReportListItem> | null
  readonly examples: ReadonlyArray<DseExampleListItem> | null
  readonly receipts: ReadonlyArray<DseReceiptListItem> | null
}

