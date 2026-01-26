export type CodexMessageRole = "user" | "assistant" | "system"

export type CodexMessage = {
  id: string
  role: CodexMessageRole
  text: string
}

export type CodexReasoning = {
  id: string
  summary?: string
  content: string
}

export type CodexPlanStep = {
  step: string
  status: string
}

export type CodexPlan = {
  id: string
  explanation?: string
  steps: CodexPlanStep[]
}

export type CodexDiff = {
  id: string
  title: string
  diff: string
  status?: string | null
}

export type CodexReview = {
  id: string
  state: "started" | "completed"
  text: string
}

export type CodexToolChange = {
  path: string
  kind?: string
  diff?: string
}

export type CodexToolCall = {
  id: string
  toolType?: string
  title: string
  detail?: string
  status?: string
  output?: string
  durationMs?: number | null
  changes?: CodexToolChange[]
}

export type CodexConversationItem =
  | { kind: "message" } & CodexMessage
  | { kind: "reasoning" } & CodexReasoning
  | { kind: "plan" } & CodexPlan
  | { kind: "diff" } & CodexDiff
  | { kind: "review" } & CodexReview
  | { kind: "tool" } & CodexToolCall
