import readline from "node:readline"

export async function readPromptFromTerminal(prompt = "You: "): Promise<string | null> {
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
