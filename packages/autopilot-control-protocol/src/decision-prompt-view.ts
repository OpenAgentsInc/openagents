import type { DecisionRecord, DecisionVerb } from "./decision.js"

export type DecisionPromptKind = "approval" | "question" | "choice" | "unknown"

export type DecisionPromptView = {
  ref: string
  kind: DecisionPromptKind
  prompt: string
  options: string[]
  requiresAnswer: boolean
}

type RawRecord = Record<string, unknown>
type DecisionRecordLike = Partial<DecisionRecord> & RawRecord

const APPROVAL_OPTIONS: readonly DecisionVerb[] = ["approve", "deny"]

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value.trim() : undefined
}

function readBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined
}

function firstString(record: RawRecord, keys: readonly string[]): string {
  for (const key of keys) {
    const value = readString(record[key])
    if (value !== undefined && value.length > 0) return value
  }

  return ""
}

function readOptions(value: unknown): string[] {
  if (!Array.isArray(value)) return []

  const options: string[] = []
  for (const option of value) {
    if (typeof option === "string") {
      const label = option.trim()
      if (label.length > 0) options.push(label)
      continue
    }

    if (!isRecord(option)) continue

    const label = firstString(option, ["label", "title", "value", "id"])
    if (label.length > 0) options.push(label)
  }

  return options
}

function readKind(rawKind: unknown, options: readonly string[]): DecisionPromptKind {
  switch (readString(rawKind)?.toLowerCase()) {
    case "approval":
    case "approve":
    case "permission":
      return "approval"
    case "question":
    case "answer":
      return "question"
    case "choice":
    case "select":
    case "selection":
      return "choice"
    default:
      return options.length > 0 ? "choice" : "unknown"
  }
}

function hasApprovalVerbs(record: RawRecord): boolean {
  const verbs = readOptions(record.availableVerbs)
  return verbs.includes("approve") && verbs.includes("deny")
}

function projectRecord(record: DecisionRecordLike): DecisionPromptView {
  const explicitOptions = readOptions(record.options)
  const kind = hasApprovalVerbs(record) ? "approval" : readKind(record.kind ?? record.type, explicitOptions)
  const options = kind === "approval" && explicitOptions.length === 0 ? [...APPROVAL_OPTIONS] : explicitOptions
  const prompt = firstString(record, ["prompt", "question", "message", "title", "actionRef"])
  const explicitRequiresAnswer = readBoolean(record.requiresAnswer)

  return {
    ref: firstString(record, ["ref", "requestId", "decisionRef", "id"]),
    kind,
    prompt,
    options,
    requiresAnswer: explicitRequiresAnswer ?? kind !== "unknown",
  }
}

export function projectDecisionPrompt(raw: unknown): DecisionPromptView {
  if (!isRecord(raw)) {
    return {
      ref: "",
      kind: "unknown",
      prompt: "",
      options: [],
      requiresAnswer: false,
    }
  }

  return projectRecord(raw)
}
