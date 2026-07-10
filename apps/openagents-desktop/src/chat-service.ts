import type { DesktopMessage } from "./chat-contract.ts"

const endpoint = (): string => `${(process.env.OPENAGENTS_INFERENCE_GATEWAY_BASE_URL ?? process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com").replace(/\/+$/, "")}/api/v1/chat/completions`

export const completeChatTurn = async (history: readonly DesktopMessage[]): Promise<string> => {
  const token = process.env.OPENAGENTS_AGENT_TOKEN?.trim() || process.env.OPENAGENTS_INFERENCE_API_KEY?.trim()
  if (!token) throw new Error("No OpenAgents model token is configured on this desktop host.")
  const response = await fetch(endpoint(), {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({
      model: process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL ?? "openagents-gateway-default",
      stream: false,
      messages: [
        { role: "system", content: "You are OpenAgents Desktop. Answer directly and accurately. Never claim a Fleet run started without an authority receipt." },
        ...history.filter((message) => message.role !== "system").slice(-24).map((message) => ({ role: message.role, content: message.text })),
      ],
    }),
  })
  if (!response.ok) throw new Error(`The model gateway returned ${response.status}.`)
  const data = await response.json() as { choices?: Array<{ message?: { content?: unknown } }> }
  const content = data.choices?.[0]?.message?.content
  if (typeof content !== "string" || content.trim() === "") throw new Error("The model gateway returned no assistant message.")
  return content.trim()
}
