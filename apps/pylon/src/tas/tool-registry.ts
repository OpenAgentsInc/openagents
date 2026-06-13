export type ToolPrimitiveType = "string" | "number" | "boolean" | "object"

export type ToolFieldSpec = {
  readonly type: ToolPrimitiveType
  readonly required?: boolean
}

export type ToolInputSchema = Readonly<Record<string, ToolFieldSpec>>

export type ToolContract = {
  readonly name: string
  readonly inputSchema: ToolInputSchema
  readonly readOnly: boolean
}

export type ToolValidationResult = {
  readonly ok: boolean
  readonly errors: string[]
}

export type ToolRegistry = {
  readonly registerTool: (contract: ToolContract) => ToolContract
  readonly getTool: (name: string) => ToolContract | undefined
}

export function validateToolCall(
  contract: ToolContract,
  args: unknown,
): ToolValidationResult {
  const errors: string[] = []
  const input =
    typeof args === "object" && args !== null && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : undefined

  if (!input) {
    errors.push("args must be an object")
    return { ok: false, errors }
  }

  for (const [field, spec] of Object.entries(contract.inputSchema)) {
    const value = input[field]

    if (spec.required === true && value === undefined) {
      errors.push(`${field} is required`)
      continue
    }

    if (value !== undefined && !matchesPrimitiveType(value, spec.type)) {
      errors.push(`${field} must be ${spec.type}`)
    }
  }

  return {
    ok: errors.length === 0,
    errors,
  }
}

export function createToolRegistry(
  initialContracts: readonly ToolContract[] = [],
): ToolRegistry {
  const tools = new Map<string, ToolContract>()

  const registry: ToolRegistry = {
    registerTool(contract) {
      if (tools.has(contract.name)) {
        throw new Error(`Tool already registered: ${contract.name}`)
      }

      tools.set(contract.name, contract)
      return contract
    },

    getTool(name) {
      return tools.get(name)
    },
  }

  for (const contract of initialContracts) {
    registry.registerTool(contract)
  }

  return registry
}

function matchesPrimitiveType(value: unknown, type: ToolPrimitiveType): boolean {
  if (type === "object") {
    return typeof value === "object" && value !== null && !Array.isArray(value)
  }

  return typeof value === type
}
