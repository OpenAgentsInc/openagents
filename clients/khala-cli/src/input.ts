import readline from "node:readline"

export interface PromptHistoryOptions {
  readonly history?: ReadonlyArray<string> | undefined
}

export function appendPromptHistory(
  history: ReadonlyArray<string>,
  value: string,
  limit = 100,
): ReadonlyArray<string> {
  const trimmed = value.trim()
  if (trimmed.length === 0) return history
  const withoutDuplicateTail = history[history.length - 1] === value
    ? history.slice(0, -1)
    : history
  return [...withoutDuplicateTail, value].slice(-limit)
}

export async function readPromptFromTerminal(
  prompt = "> ",
  options: PromptHistoryOptions = {},
): Promise<string | null> {
  if (process.stdin.isTTY && process.stdout.isTTY && typeof process.stdin.setRawMode === "function") {
    return await readPromptWithHistory(prompt, options.history ?? [])
  }

  return await new Promise<string | null>((resolve) => {
    let settled = false
    const input = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
      terminal: true,
    })

    const settle = (value: string | null) => {
      if (settled) return
      settled = true
      try {
        input.close()
      } finally {
        resolve(value)
      }
    }

    input.on("SIGINT", () => settle(null))
    input.on("close", () => settle(null))
    input.question(prompt, (answer) => settle(answer))
  })
}

async function readPromptWithHistory(
  prompt: string,
  history: ReadonlyArray<string>,
): Promise<string | null> {
  return await new Promise<string | null>((resolve) => {
    let settled = false
    let value = ""
    let cursor = history.length
    const stdin = process.stdin
    const stdout = process.stdout
    const wasRaw = stdin.isRaw

    readline.emitKeypressEvents(stdin)
    stdin.setRawMode(true)
    stdin.resume()
    stdout.write(prompt)

    const render = (): void => {
      readline.clearLine(stdout, 0)
      readline.cursorTo(stdout, 0)
      stdout.write(`${prompt}${value}`)
    }

    const settle = (answer: string | null): void => {
      if (settled) return
      settled = true
      stdin.off("keypress", onKeypress)
      stdin.setRawMode(wasRaw)
      if (answer !== null) stdout.write("\n")
      resolve(answer)
    }

    const setFromHistory = (nextCursor: number): void => {
      cursor = Math.max(0, Math.min(history.length, nextCursor))
      value = cursor === history.length ? "" : history[cursor] ?? ""
      render()
    }

    const onKeypress = (str: string, key: readline.Key): void => {
      if (key.ctrl === true && key.name === "c") {
        settle(null)
        return
      }
      if (key.ctrl === true && key.name === "l") {
        stdout.write("\x1b[H\x1b[2J\x1b[3J")
        render()
        return
      }
      if (key.name === "return" || key.name === "enter") {
        settle(value)
        return
      }
      if (key.name === "backspace" || key.name === "delete") {
        if (value.length > 0) {
          value = value.slice(0, -1)
          render()
        }
        return
      }
      if (key.name === "up") {
        if (history.length > 0) {
          setFromHistory(cursor - 1)
        }
        return
      }
      if (key.name === "down") {
        if (history.length > 0) {
          setFromHistory(cursor + 1)
        }
        return
      }
      if (key.name === "left" || key.name === "right") {
        return
      }
      if (str.length > 0 && !key.ctrl && !key.meta) {
        value = `${value}${str}`
        stdout.write(str)
      }
    }

    stdin.on("keypress", onKeypress)
  })
}
