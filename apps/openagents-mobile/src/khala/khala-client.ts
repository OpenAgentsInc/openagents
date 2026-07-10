import { fetch as expoFetch } from "expo/fetch"

import type { KhalaTurnClient } from "../screens/khala-core"

export const KHALA_CHAT_URL = "https://openagents.com/api/khala/chat"

const readSseReply = async (response: Response): Promise<string> => {
  if (response.body === null) throw new Error("khala_stream_missing")
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ""
  let reply = ""
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    let boundary = buffer.indexOf("\n\n")
    while (boundary >= 0) {
      const frame = buffer.slice(0, boundary)
      buffer = buffer.slice(boundary + 2)
      const event = frame.match(/^event:\s*(.+)$/m)?.[1]?.trim()
      const data = frame
        .split("\n")
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice(5).trimStart())
        .join("\n")
      if (event === "delta" && data !== "") {
        const payload = JSON.parse(data) as { text?: unknown }
        if (typeof payload.text === "string") reply += payload.text
      }
      if (event === "error") throw new Error("khala_stream_error")
      boundary = buffer.indexOf("\n\n")
    }
  }
  if (reply === "") throw new Error("khala_empty_reply")
  return reply
}

/** Public, unauthenticated generic Khala chat. Routing stays server-owned. */
export const sendKhalaTurn: KhalaTurnClient["sendTurn"] = async ({ messages }) => {
  const response = await expoFetch(KHALA_CHAT_URL, {
    method: "POST",
    headers: { accept: "text/event-stream", "content-type": "application/json" },
    body: JSON.stringify({ messages }),
  })
  if (!response.ok) throw new Error(`khala_http_${response.status}`)
  return { reply: await readSseReply(response) }
}
