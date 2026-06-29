export type CommandArgType = "string" | "number"

export type CommandArgSpec = Readonly<{
  name: string
  type: CommandArgType
  required?: boolean
}>

export type CommandDescriptor = Readonly<{
  name: string
  args: readonly CommandArgSpec[]
}>

export type CommandRegistry = Readonly<{
  commands: ReadonlyMap<string, CommandDescriptor>
}>

export type ParsedCommandArgs = Readonly<Record<string, string | number>>

export type CommandParseError =
  | Readonly<{ code: "not_command"; message: string }>
  | Readonly<{ code: "unknown_command"; commandName: string; message: string }>
  | Readonly<{ code: "missing_required_arg"; argName: string; message: string }>
  | Readonly<{
      code: "invalid_arg_type"
      argName: string
      expectedType: CommandArgType
      value: string
      message: string
    }>
  | Readonly<{ code: "too_many_args"; message: string }>
  | Readonly<{ code: "unterminated_quote"; message: string }>

export type CommandParseResult =
  | Readonly<{ ok: true; name: string; args: ParsedCommandArgs }>
  | Readonly<{ ok: false; error: CommandParseError }>

export function createCommandRegistry(
  commands: readonly CommandDescriptor[] = [],
): CommandRegistry {
  return commands.reduce<CommandRegistry>(
    (registry, command) => registerCommand(registry, command),
    { commands: new Map<string, CommandDescriptor>() },
  )
}

export function registerCommand(
  registry: CommandRegistry,
  command: CommandDescriptor,
): CommandRegistry {
  assertCommandName(command.name)

  if (registry.commands.has(command.name)) {
    throw new Error(`Command already registered: ${command.name}`)
  }

  const commands = new Map(registry.commands)
  commands.set(command.name, command)

  return { commands }
}

export function parseCommand(
  registry: CommandRegistry,
  input: string,
): CommandParseResult {
  const trimmed = input.trim()

  if (!trimmed.startsWith("/")) {
    return {
      ok: false,
      error: {
        code: "not_command",
        message: "input must start with an explicit slash command",
      },
    }
  }

  const tokens = tokenize(trimmed)

  if (tokens.ok === false) {
    return tokens
  }

  const [commandToken, ...argTokens] = tokens.tokens

  if (!commandToken || commandToken === "/") {
    return {
      ok: false,
      error: {
        code: "not_command",
        message: "input must include a slash command name",
      },
    }
  }

  const commandName = commandToken.slice(1)
  const descriptor = registry.commands.get(commandName)

  if (!descriptor) {
    return {
      ok: false,
      error: {
        code: "unknown_command",
        commandName,
        message: `unknown command: /${commandName}`,
      },
    }
  }

  if (argTokens.length > descriptor.args.length) {
    return {
      ok: false,
      error: {
        code: "too_many_args",
        message: `/${commandName} received too many arguments`,
      },
    }
  }

  const args: Record<string, string | number> = {}

  for (const [index, spec] of descriptor.args.entries()) {
    const value = argTokens[index]

    if (value === undefined) {
      if (spec.required === true) {
        return {
          ok: false,
          error: {
            code: "missing_required_arg",
            argName: spec.name,
            message: `${spec.name} is required`,
          },
        }
      }

      continue
    }

    const parsed = parseArgValue(spec, value)

    if (parsed.ok === false) {
      return parsed
    }

    args[spec.name] = parsed.value
  }

  return {
    ok: true,
    name: commandName,
    args,
  }
}

function parseArgValue(
  spec: CommandArgSpec,
  value: string,
):
  | Readonly<{ ok: true; value: string | number }>
  | Readonly<{ ok: false; error: CommandParseError }> {
  if (spec.type === "string") {
    return { ok: true, value }
  }

  const parsed = Number(value)

  if (value.trim() === "" || !Number.isFinite(parsed)) {
    return {
      ok: false,
      error: {
        code: "invalid_arg_type",
        argName: spec.name,
        expectedType: spec.type,
        value,
        message: `${spec.name} must be ${spec.type}`,
      },
    }
  }

  return { ok: true, value: parsed }
}

function tokenize(
  input: string,
):
  | Readonly<{ ok: true; tokens: readonly string[] }>
  | Readonly<{ ok: false; error: CommandParseError }> {
  const tokens: string[] = []
  let current = ""
  let quote: '"' | "'" | undefined

  for (let index = 0; index < input.length; index += 1) {
    const char = input[index]

    if (quote) {
      if (char === quote) {
        quote = undefined
      } else {
        current += char
      }

      continue
    }

    if (char === '"' || char === "'") {
      quote = char
      continue
    }

    if (/\s/.test(char)) {
      if (current !== "") {
        tokens.push(current)
        current = ""
      }

      continue
    }

    current += char
  }

  if (quote) {
    return {
      ok: false,
      error: {
        code: "unterminated_quote",
        message: "command argument quote is unterminated",
      },
    }
  }

  if (current !== "") {
    tokens.push(current)
  }

  return { ok: true, tokens }
}

function assertCommandName(name: string): void {
  if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(name)) {
    throw new Error(`Invalid command name: ${name}`)
  }
}
