// Shared OpenCode CLI streaming helper. Spawns `opencode run` with JSON
// event output and surfaces progress through plain callbacks, so callers
// (e.g. the agent-runtime adapter) reuse one parser without touching
// renderables or Effect (issue #4737).

export const OPENCODE_DEFAULT_MODEL = "opencode/deepseek-v4-flash-free"

export function summarizeOpenCodeEvent(event: any): string {
  const type = typeof event?.type === "string" ? event.type : "event"
  const partType = typeof event?.part?.type === "string" ? event.part.type : undefined
  const tool = event?.part?.tool ?? event?.tool ?? event?.name
  const title = event?.part?.title ?? event?.title
  const path = event?.part?.path ?? event?.path
  const detail = [partType, tool, title, path].filter(Boolean).join(" ")
  return detail ? `${type}: ${detail}` : type
}

export interface OpencodeStreamCallbacks {
  // Called with the accumulated response text after each text chunk.
  onText?: (fullText: string) => void
  // Called for every parsed non-text event with a one-line summary.
  onEvent?: (summary: string, eventCount: number) => void
  // Called for unparseable stdout lines.
  onRaw?: (line: string) => void
  onStderr?: (line: string) => void
  // Called when a step_finish event reports usage.
  onUsage?: (usage: { cost: number; tokens: number }) => void
}

export interface OpencodeStreamResult {
  text: string
  cost: number
  tokens: number
  eventCount: number
  byteCount: number
}

export async function runOpencodeStream(
  opencodePath: string,
  prompt: string,
  callbacks: OpencodeStreamCallbacks = {},
  model: string = OPENCODE_DEFAULT_MODEL,
): Promise<OpencodeStreamResult> {
  const proc = Bun.spawn([opencodePath, "run", prompt, "--model", model, "--format", "json"], {
    stdout: "pipe",
    stderr: "pipe",
  })

  let textResult = ""
  let finalCost = 0
  let totalTokens = 0
  let eventCount = 0
  let byteCount = 0

  const stderrTask = (async () => {
    const reader = proc.stderr.getReader()
    const decoder = new TextDecoder()
    let buffer = ""
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split("\n")
      buffer = lines.pop() ?? ""
      for (const line of lines) {
        if (line.trim()) callbacks.onStderr?.(line.trim())
      }
    }
    const trailing = buffer.trim()
    if (trailing) callbacks.onStderr?.(trailing)
  })()

  const handleLine = (line: string) => {
    if (!line.trim()) return
    eventCount += 1
    try {
      const event = JSON.parse(line)
      if (event.type === "text" && event.part && event.part.text) {
        textResult += event.part.text
        callbacks.onText?.(textResult)
        return
      }
      callbacks.onEvent?.(summarizeOpenCodeEvent(event), eventCount)
      if (event.type === "step_finish" && event.part && event.part.tokens) {
        finalCost = event.part.cost ?? 0
        totalTokens = event.part.tokens.total ?? 0
        callbacks.onUsage?.({ cost: finalCost, tokens: totalTokens })
      }
    } catch {
      callbacks.onRaw?.(line)
    }
  }

  const reader = proc.stdout.getReader()
  const decoder = new TextDecoder()
  let buffer = ""
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    byteCount += value.byteLength
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split("\n")
    buffer = lines.pop() ?? ""
    for (const line of lines) handleLine(line)
  }
  handleLine(buffer)

  const exitCode = await proc.exited
  await stderrTask
  if (exitCode !== 0) {
    throw new Error(`OpenCode exited with code ${exitCode}`)
  }

  return {
    text: textResult.trim(),
    cost: finalCost,
    tokens: totalTokens,
    eventCount,
    byteCount,
  }
}
