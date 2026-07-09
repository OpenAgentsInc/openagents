import { createHash } from "node:crypto"

import { CLAUDE_AGENT_SDK_PACKAGE } from "./claude-agent.js"
import {
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "../custody/account-registry.js"

export const CLAUDE_SECOND_PASS_REVIEW_SCHEMA = "openagents.pylon.claude_second_pass_review.v1" as const

export type ClaudeSecondPassRecommendation = "approve" | "manual_review" | "request_changes"

export type ClaudeSecondPassVerdict = {
  schema: typeof CLAUDE_SECOND_PASS_REVIEW_SCHEMA
  recommendation: ClaudeSecondPassRecommendation
  confidence: "low" | "medium" | "high"
  summary: string
  riskRefs: string[]
}

export type ClaudeSecondPassReviewInput = {
  assignmentRef: string
  workspace: string
  diffText: string
  verifyCommandRef: string
  verifyCommand: string[]
  account?: ResolvedPylonAccountSelection | null
  env?: Record<string, string | undefined>
  sdkImporter?: (specifier: string) => Promise<{ query: (args: unknown) => AsyncIterable<unknown> }>
  timeoutMs?: number
  model?: string
}

export type ClaudeSecondPassReviewer = (input: ClaudeSecondPassReviewInput) => Promise<ClaudeSecondPassVerdict>

export type ClaudeSecondPassReviewOptions =
  | {
      enabled: true
      account: ResolvedPylonAccountSelection
      reviewer?: ClaudeSecondPassReviewer
      timeoutMs?: number
    }
  | {
      enabled: true
      reviewer: ClaudeSecondPassReviewer
      account?: ResolvedPylonAccountSelection | null
      timeoutMs?: number
    }

const DEFAULT_TIMEOUT_MS = 120_000
const MAX_DIFF_CHARS = 40_000
const REF_SHAPED_PATTERN = /^[a-z][a-z0-9_-]*(?:\.[a-z0-9][a-z0-9_-]*)+$/
const MAX_PUBLIC_REF_CHARS = 160

export function claudeSecondPassVerdictRef(verdict: ClaudeSecondPassVerdict): string {
  return `review.public.pylon.claude_second_pass.${createHash("sha256")
    .update(JSON.stringify(verdict))
    .digest("hex")
    .slice(0, 24)}`
}

export function parseClaudeSecondPassVerdict(value: unknown): ClaudeSecondPassVerdict | null {
  let parsed = value
  if (typeof parsed === "string") {
    const sliced = sliceFirstJsonObject(parsed)
    if (sliced === null) return null
    try {
      parsed = JSON.parse(sliced)
    } catch {
      return null
    }
  }
  if (parsed === null || typeof parsed !== "object") return null
  const record = parsed as Record<string, unknown>
  if (record.schema !== CLAUDE_SECOND_PASS_REVIEW_SCHEMA) return null
  if (
    record.recommendation !== "approve" &&
    record.recommendation !== "manual_review" &&
    record.recommendation !== "request_changes"
  ) return null
  if (record.confidence !== "low" && record.confidence !== "medium" && record.confidence !== "high") return null
  if (typeof record.summary !== "string" || record.summary.trim().length === 0) return null
  if (!Array.isArray(record.riskRefs)) return null
  const riskRefs: string[] = []
  for (const value of record.riskRefs) {
    const ref = normalizeClaudeSecondPassRiskRef(value)
    if (ref === null) return null
    riskRefs.push(ref)
  }
  return {
    schema: CLAUDE_SECOND_PASS_REVIEW_SCHEMA,
    recommendation: record.recommendation,
    confidence: record.confidence,
    summary: record.summary.trim().slice(0, 500),
    riskRefs: riskRefs.slice(0, 20),
  }
}

function normalizeClaudeSecondPassRiskRef(value: unknown): string | null {
  if (typeof value !== "string") return null
  const ref = value.trim()
  if (ref.length === 0 || ref.length > MAX_PUBLIC_REF_CHARS) return null
  if (!REF_SHAPED_PATTERN.test(ref)) return null
  return ref
}

function sliceFirstJsonObject(text: string): string | null {
  const start = text.indexOf("{")
  if (start === -1) return null
  let depth = 0
  let inString = false
  let escaped = false
  for (let index = start; index < text.length; index += 1) {
    const char = text[index]
    if (inString) {
      if (escaped) {
        escaped = false
      } else if (char === "\\") {
        escaped = true
      } else if (char === "\"") {
        inString = false
      }
      continue
    }
    if (char === "\"") {
      inString = true
      continue
    }
    if (char === "{") depth += 1
    if (char === "}") {
      depth -= 1
      if (depth === 0) return text.slice(start, index + 1)
    }
  }
  return null
}

export function buildClaudeSecondPassReviewPrompt(input: ClaudeSecondPassReviewInput): string {
  const diff =
    input.diffText.length > MAX_DIFF_CHARS
      ? `${input.diffText.slice(0, MAX_DIFF_CHARS)}\n...(diff truncated)`
      : input.diffText
  return [
    "You are a second-pass semantic reviewer for a verified Codex worker closeout.",
    "The verification command already passed and remains the authority.",
    "Review only the unified diff below for semantic risk, missed scope, and likely regressions.",
    "Do not approve execution or merge. Your verdict is advisory and may only request manual review.",
    "Return exactly one JSON object matching the requested schema.",
    "",
    `Assignment ref: ${input.assignmentRef}`,
    `Verification command ref: ${input.verifyCommandRef}`,
    `Verification command argv: ${input.verifyCommand.join(" ")}`,
    "",
    "Unified diff:",
    diff.length === 0 ? "(empty diff)" : diff,
  ].join("\n")
}

export async function runClaudeSecondPassReview(input: ClaudeSecondPassReviewInput): Promise<ClaudeSecondPassVerdict> {
  if (input.account === undefined || input.account === null || input.account.provider !== "claude_agent") {
    throw new Error("Claude second-pass reviewer requires an isolated claude_agent account")
  }
  const env = pylonAccountEnvironment(input.env ?? Bun.env, input.account)
  const sdk = input.sdkImporter === undefined
    ? await import(CLAUDE_AGENT_SDK_PACKAGE) as { query: (args: unknown) => AsyncIterable<unknown> }
    : await input.sdkImporter(CLAUDE_AGENT_SDK_PACKAGE)
  const abort = new AbortController()
  const timer = setTimeout(() => abort.abort(), input.timeoutMs ?? DEFAULT_TIMEOUT_MS)
  let text = ""
  try {
    const session = sdk.query({
      prompt: buildClaudeSecondPassReviewPrompt(input),
      options: {
        cwd: input.workspace,
        env,
        allowedTools: ["Read", "Grep", "Glob"],
        maxTurns: 2,
        permissionMode: "plan",
        settingSources: [],
        abortController: abort,
        outputFormat: {
          type: "json_schema",
          schema: {
            type: "object",
            additionalProperties: false,
            required: ["schema", "recommendation", "confidence", "summary", "riskRefs"],
            properties: {
              schema: { const: CLAUDE_SECOND_PASS_REVIEW_SCHEMA },
              recommendation: { enum: ["approve", "manual_review", "request_changes"] },
              confidence: { enum: ["low", "medium", "high"] },
              summary: { type: "string", minLength: 1, maxLength: 500 },
              riskRefs: {
                type: "array",
                maxItems: 20,
                items: { type: "string", minLength: 1, maxLength: MAX_PUBLIC_REF_CHARS, pattern: REF_SHAPED_PATTERN.source },
              },
            },
          },
        },
        ...(input.model === undefined ? {} : { model: input.model }),
      },
    })
    for await (const message of session) {
      const record = message as Record<string, unknown>
      if (typeof record.result === "string") text += record.result
      if (typeof record.text === "string") text += record.text
      if (typeof record.content === "string") text += record.content
      if (Array.isArray(record.content)) {
        for (const item of record.content) {
          const content = item as { text?: unknown }
          if (typeof content.text === "string") text += content.text
        }
      }
    }
  } finally {
    clearTimeout(timer)
  }
  const verdict = parseClaudeSecondPassVerdict(text)
  if (verdict === null) {
    throw new Error("Claude second-pass reviewer returned an invalid structured verdict")
  }
  return verdict
}
