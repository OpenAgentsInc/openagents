const defaultTimeoutMs = 120_000
const terminateGraceMs = 5_000

type ParsedArgs = {
  readonly command: string[]
  readonly label: string
  readonly timeoutMs: number
}

const usage = (): string =>
  [
    "Usage: bun scripts/run-with-timeout.ts [--timeout-ms <ms>] [--label <name>] -- <command> [args...]",
    "Example: bun scripts/run-with-timeout.ts --timeout-ms 120000 --label verse-launch-smoke -- bun scripts/verse-launch-smoke.ts",
  ].join("\n")

const parsePositiveInteger = (name: string, value: string): number => {
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer, received ${value}`)
  }
  return parsed
}

const readOptionValue = (
  args: readonly string[],
  index: number,
  option: string,
): readonly [string, number] => {
  const value = args[index + 1]
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value\n${usage()}`)
  }
  return [value, index + 2]
}

const parseArgs = (argv: readonly string[]): ParsedArgs => {
  const delimiterIndex = argv.indexOf("--")
  if (delimiterIndex === -1) {
    throw new Error(`missing -- command delimiter\n${usage()}`)
  }

  let label = "timed command"
  let timeoutMs = defaultTimeoutMs
  const options = argv.slice(0, delimiterIndex)
  const command = argv.slice(delimiterIndex + 1)

  if (command.length === 0) {
    throw new Error(`missing command after --\n${usage()}`)
  }

  for (let index = 0; index < options.length; ) {
    const option = options[index]

    if (option === "--timeout-ms") {
      const [value, nextIndex] = readOptionValue(options, index, option)
      timeoutMs = parsePositiveInteger(option, value)
      index = nextIndex
      continue
    }

    if (option.startsWith("--timeout-ms=")) {
      timeoutMs = parsePositiveInteger(
        "--timeout-ms",
        option.slice("--timeout-ms=".length),
      )
      index += 1
      continue
    }

    if (option === "--label") {
      const [value, nextIndex] = readOptionValue(options, index, option)
      label = value
      index = nextIndex
      continue
    }

    if (option.startsWith("--label=")) {
      label = option.slice("--label=".length)
      index += 1
      continue
    }

    throw new Error(`unknown option: ${option}\n${usage()}`)
  }

  if (label.trim().length === 0) {
    throw new Error("--label must not be empty")
  }

  return { command, label, timeoutMs }
}

const run = async (): Promise<number> => {
  const { command, label, timeoutMs } = parseArgs(process.argv.slice(2))
  let timedOut = false
  let settled = false
  let killTimer: ReturnType<typeof setTimeout> | undefined
  const commandName = command[0] ?? "command"

  const child = Bun.spawn({
    cmd: command,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })

  const timeoutTimer = setTimeout(() => {
    timedOut = true
    console.error(
      `${label} exceeded ${timeoutMs}ms; sending SIGTERM to ${commandName}.`,
    )
    child.kill("SIGTERM")

    killTimer = setTimeout(() => {
      if (settled) return
      console.error(
        `${label} did not exit within ${terminateGraceMs}ms after SIGTERM; sending SIGKILL to ${commandName}.`,
      )
      child.kill("SIGKILL")
    }, terminateGraceMs)
  }, timeoutMs)

  const exitCode = await child.exited
  settled = true
  clearTimeout(timeoutTimer)
  if (killTimer !== undefined) clearTimeout(killTimer)

  if (timedOut) return 124
  return exitCode
}

run().then(
  exitCode => process.exit(exitCode),
  error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  },
)
