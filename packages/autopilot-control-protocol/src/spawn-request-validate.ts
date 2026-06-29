export type SpawnRequestAdapter = "codex" | "claude_agent"

export type SpawnRequestValidationResult = {
  ok: boolean
  adapter: SpawnRequestAdapter | null
  objective: string
  accountRef: string | null
  errors: string[]
}

export function validateSpawnRequest(input: {
  adapter: unknown
  objective: unknown
  accountRef?: unknown
}): SpawnRequestValidationResult {
  const errors: string[] = []
  const adapter = parseAdapter(input.adapter, errors)
  const objective = parseObjective(input.objective, errors)
  const accountRef = parseAccountRef(input.accountRef)

  return {
    ok: errors.length === 0,
    adapter,
    objective,
    accountRef,
    errors,
  }
}

function parseAdapter(
  value: unknown,
  errors: string[],
): SpawnRequestAdapter | null {
  if (value === "codex" || value === "claude_agent") {
    return value
  }

  errors.push("adapter must be one of codex|claude_agent")
  return null
}

function parseObjective(value: unknown, errors: string[]): string {
  if (typeof value !== "string") {
    errors.push("objective must be a string")
    return ""
  }

  const objective = value.trim()
  if (objective.length === 0) {
    errors.push("objective must be non-empty")
  }

  if (objective.length > 4000) {
    errors.push("objective must be <=4000 characters")
  }

  return objective
}

function parseAccountRef(value: unknown): string | null {
  if (typeof value !== "string") {
    return null
  }

  const accountRef = value.trim()
  return accountRef.length > 0 ? accountRef : null
}
