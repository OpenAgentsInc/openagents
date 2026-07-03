import { Schema as S } from "effect"

export const KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA =
  "openagents.khala_code.diff_review_comment.v1" as const
export const KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT =
  "khala-code-diff-review-submit" as const
export const KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA =
  "openagents.khala_code.judge_diff_verdict.v1" as const

export const KhalaCodeDiffReviewLineKindSchema = S.Literals([
  "add",
  "context",
  "remove",
])
export type KhalaCodeDiffReviewLineKind =
  typeof KhalaCodeDiffReviewLineKindSchema.Type

export const KhalaCodeDiffReviewLineSideSchema = S.Literals(["new", "old"])
export type KhalaCodeDiffReviewLineSide =
  typeof KhalaCodeDiffReviewLineSideSchema.Type

export const KhalaCodeDiffReviewCommentSchema = S.Struct({
  body: S.String,
  commentRef: S.String,
  filePath: S.String,
  lineKind: KhalaCodeDiffReviewLineKindSchema,
  lineNo: S.Number,
  lineSide: KhalaCodeDiffReviewLineSideSchema,
  patchRef: S.String,
  schema: S.Literal(KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA),
})
export type KhalaCodeDiffReviewComment =
  typeof KhalaCodeDiffReviewCommentSchema.Type

export const KhalaCodeDiffReviewSubmitDetailSchema = S.Struct({
  body: S.String,
  filePath: S.String,
  lineKind: KhalaCodeDiffReviewLineKindSchema,
  lineNo: S.Number,
  lineSide: KhalaCodeDiffReviewLineSideSchema,
  patchRef: S.String,
})
export type KhalaCodeDiffReviewSubmitDetail =
  typeof KhalaCodeDiffReviewSubmitDetailSchema.Type

export const KhalaCodeJudgeDiffVerdictKindSchema = S.Literals([
  "accept",
  "request_changes",
  "replan",
])
export type KhalaCodeJudgeDiffVerdictKind =
  typeof KhalaCodeJudgeDiffVerdictKindSchema.Type

export const KhalaCodeJudgeDiffFindingPrioritySchema = S.Literals([
  "P0",
  "P1",
  "P2",
  "P3",
])
export type KhalaCodeJudgeDiffFindingPriority =
  typeof KhalaCodeJudgeDiffFindingPrioritySchema.Type

export const KhalaCodeJudgeDiffVerdictFindingSchema = S.Struct({
  body: S.String,
  confidence: S.Number,
  filePath: S.String,
  findingRef: S.String,
  lineEnd: S.optional(S.Number),
  lineStart: S.Number,
  priority: KhalaCodeJudgeDiffFindingPrioritySchema,
  title: S.String,
})
export type KhalaCodeJudgeDiffVerdictFinding =
  typeof KhalaCodeJudgeDiffVerdictFindingSchema.Type

export const KhalaCodeJudgeDiffVerdictSchema = S.Struct({
  confidence: S.Number,
  diffRef: S.optional(S.String),
  findings: S.Array(KhalaCodeJudgeDiffVerdictFindingSchema),
  schema: S.Literal(KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA),
  summary: S.String,
  verdict: KhalaCodeJudgeDiffVerdictKindSchema,
  verifyAuthority: S.Literal("verify_command_required"),
})
export type KhalaCodeJudgeDiffVerdict =
  typeof KhalaCodeJudgeDiffVerdictSchema.Type

export const khalaCodeDiffReviewLineLabel = (
  input: Pick<KhalaCodeDiffReviewComment, "filePath" | "lineNo" | "lineSide">,
): string => `${input.filePath}:${input.lineSide === "new" ? "+" : "-"}${input.lineNo}`

export const khalaCodeDiffReviewComment = (
  input: KhalaCodeDiffReviewSubmitDetail & { readonly commentRef: string },
): KhalaCodeDiffReviewComment => ({
  body: input.body.trim(),
  commentRef: input.commentRef,
  filePath: input.filePath,
  lineKind: input.lineKind,
  lineNo: input.lineNo,
  lineSide: input.lineSide,
  patchRef: input.patchRef,
  schema: KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA,
})

export const khalaCodeDiffReviewSteeringNote = (
  comment: KhalaCodeDiffReviewComment,
): string => [
  `Diff review comment (${comment.schema})`,
  "Role: judge",
  `Ref: ${comment.commentRef}`,
  `Line: ${khalaCodeDiffReviewLineLabel(comment)}`,
  `Kind: ${comment.lineKind}`,
  "",
  comment.body,
].join("\n")

export const khalaCodeJudgeDiffFindingToReviewDetail = (
  finding: KhalaCodeJudgeDiffVerdictFinding,
): Omit<KhalaCodeDiffReviewSubmitDetail, "body"> & { readonly body: string } => ({
  body: [
    `Judge finding ${finding.findingRef} (${finding.priority}, confidence ${finding.confidence.toFixed(2)})`,
    finding.title,
    "",
    finding.body,
  ].join("\n"),
  filePath: finding.filePath,
  lineKind: "context",
  lineNo: finding.lineStart,
  lineSide: "new",
  patchRef: `judge.${finding.findingRef}.${finding.filePath}.${finding.lineStart}`,
})
