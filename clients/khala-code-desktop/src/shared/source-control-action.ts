import { Schema as S } from "effect"

export const KHALA_CODE_SOURCE_CONTROL_ACTION_SCHEMA =
  "openagents.khala_code.source_control_action_prompt.v1" as const
export const KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT =
  "khala-code-source-control-action-submit" as const

export const KhalaCodeSourceControlActionKindSchema = S.Literals([
  "commit_message",
  "fix_checks",
  "pr_body",
])
export type KhalaCodeSourceControlActionKind =
  typeof KhalaCodeSourceControlActionKindSchema.Type

export const KhalaCodeSourceControlActionSubmitDetailSchema = S.Struct({
  action: KhalaCodeSourceControlActionKindSchema,
  filePath: S.optional(S.String),
  sourceRef: S.String,
})
export type KhalaCodeSourceControlActionSubmitDetail =
  typeof KhalaCodeSourceControlActionSubmitDetailSchema.Type

export const KhalaCodeSourceControlActionPromptSchema = S.Struct({
  action: KhalaCodeSourceControlActionKindSchema,
  actionRef: S.String,
  filePath: S.optional(S.String),
  schema: S.Literal(KHALA_CODE_SOURCE_CONTROL_ACTION_SCHEMA),
  sourceRef: S.String,
})
export type KhalaCodeSourceControlActionPrompt =
  typeof KhalaCodeSourceControlActionPromptSchema.Type

export const khalaCodeSourceControlActionLabel = (
  action: KhalaCodeSourceControlActionKind,
): string => {
  switch (action) {
    case "commit_message":
      return "commit message"
    case "fix_checks":
      return "fix checks"
    case "pr_body":
      return "PR body"
  }
}

export const khalaCodeSourceControlActionPrompt = (
  input: KhalaCodeSourceControlActionSubmitDetail & { readonly actionRef: string },
): KhalaCodeSourceControlActionPrompt => ({
  action: input.action,
  actionRef: input.actionRef,
  ...(input.filePath === undefined ? {} : { filePath: input.filePath }),
  schema: KHALA_CODE_SOURCE_CONTROL_ACTION_SCHEMA,
  sourceRef: input.sourceRef,
})

export const khalaCodeSourceControlActionInstructions = (
  action: KhalaCodeSourceControlActionKind,
): string => {
  switch (action) {
    case "commit_message":
      return [
        "Draft a concise commit message for the currently reviewed changes.",
        "Use imperative mood, keep it public-safe, and do not run git commit.",
      ].join(" ")
    case "fix_checks":
      return [
        "Review the failing check output or validation context in this thread.",
        "Identify the smallest scoped fix, run the relevant local/owned checks, and report evidence before any source-control writeback.",
      ].join(" ")
    case "pr_body":
      return [
        "Draft a pull request body for the currently reviewed changes.",
        "Include Summary and Validation sections, keep it public-safe, and do not create or publish a PR.",
      ].join(" ")
  }
}

export const khalaCodeSourceControlActionPromptText = (
  prompt: KhalaCodeSourceControlActionPrompt,
): string => [
  `Source-control AI action (${prompt.schema})`,
  `Ref: ${prompt.actionRef}`,
  `Action: ${khalaCodeSourceControlActionLabel(prompt.action)}`,
  `Source: ${prompt.sourceRef}`,
  ...(prompt.filePath === undefined ? [] : [`File: ${prompt.filePath}`]),
  "",
  khalaCodeSourceControlActionInstructions(prompt.action),
].join("\n")
