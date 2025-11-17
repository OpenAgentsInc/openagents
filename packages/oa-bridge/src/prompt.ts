export type PromptSegment = { type: 'text'; text: string }

export interface ReadOnlyPromptOptions {
  maxToolCalls?: number
}

export function buildReadOnlyPrompt(paths: string[], opts: ReadOnlyPromptOptions = {}): PromptSegment[] {
  const { maxToolCalls = 8 } = opts
  const list = paths.map((p, i) => `${i + 1}) ${p}`).join('\n')
  const text =
    `Explore this repository using read-only tools to gather context. Start by scanning and searching, then open files to verify details.\n\n` +
    `Priority targets:\n${list}\n\n` +
    `Rules:\n` +
    `- You may use up to ${maxToolCalls} read-only tool calls total.\n` +
    `- Prefer search/scan tools first (e.g., Find/Glob, Grep/Search).\n` +
    `- If you use a shell/terminal, restrict to read-only commands only (e.g., cat, ls, grep, sed -n, find).\n` +
    `- Never write or modify files; never run builds, installs, or network commands.\n` +
    `- Do not execute code or start servers.\n` +
    `- When you reach the limit, stop and summarize.\n\n` +
    `Deliverable:\n` +
    `- Summarize the project structure and key configuration in 6-10 concise bullet points, including build tooling, package managers, frameworks, and any notable conventions or scripts.`
  return [{ type: 'text', text }]
}
