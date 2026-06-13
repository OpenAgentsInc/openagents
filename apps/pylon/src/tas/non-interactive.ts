export type PromptRef = string

export type InteractivePrompt = Readonly<{
  promptRef: PromptRef
  interactive?: boolean
}>

export type PromptBlocked = Readonly<{
  kind: "prompt_blocked"
  promptRef: PromptRef
  reason: "interactive_prompt_unavailable"
}>

export type PromptResolution<Prompt extends InteractivePrompt> =
  | Prompt
  | PromptBlocked

export type InteractionMode = Readonly<{
  interactive: boolean
}>

export function resolvePrompt<Prompt extends InteractivePrompt>(
  prompt: Prompt,
  mode: InteractionMode,
): PromptResolution<Prompt> {
  if (mode.interactive || prompt.interactive === false) {
    return prompt
  }

  return {
    kind: "prompt_blocked",
    promptRef: prompt.promptRef,
    reason: "interactive_prompt_unavailable",
  }
}

export type TerminalStatusState =
  | "queued"
  | "running"
  | "waiting"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled"

export type TerminalStatus = Readonly<{
  state: TerminalStatusState
  label: string
  subjectRef?: string
}>

export type StatusLineOptions = Readonly<{
  color: boolean
}>

const stateColors = {
  queued: "36",
  running: "34",
  waiting: "33",
  blocked: "33",
  completed: "32",
  failed: "31",
  cancelled: "90",
} as const satisfies Record<TerminalStatusState, string>

export function statusLine(
  status: TerminalStatus,
  options: StatusLineOptions,
): string {
  if (!options.color) {
    return [
      `state=${status.state}`,
      `label=${quoteField(status.label)}`,
      ...(status.subjectRef ? [`subject_ref=${status.subjectRef}`] : []),
    ].join(" ")
  }

  const state = `\u001b[${stateColors[status.state]}m${status.state}\u001b[0m`
  return [
    `[${state}]`,
    status.label,
    ...(status.subjectRef ? [`(${status.subjectRef})`] : []),
  ].join(" ")
}

function quoteField(value: string): string {
  return JSON.stringify(value)
}
