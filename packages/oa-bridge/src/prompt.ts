export type PromptSegment = { type: 'text'; text: string }

export function buildReadOnlyPrompt(paths: string[]): PromptSegment[] {
  const list = paths.map((p, i) => `${i + 1}) ${p}`).join('\n')
  const text =
    `Explore the project using read-only tools only. Read these absolute paths:\n` +
    `${list}\n\n` +
    `Rules:\n` +
    `- Use only the Read file tool.\n` +
    `- Do not run commands or write files.\n` +
    `- After reading, summarize the project structure and key configuration in 6-10 bullet points.`
  return [{ type: 'text', text }]
}

