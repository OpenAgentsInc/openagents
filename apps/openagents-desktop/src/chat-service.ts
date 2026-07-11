import type { DesktopMessage } from "./chat-contract.ts"

const endpoint = (): string => `${(process.env.OPENAGENTS_INFERENCE_GATEWAY_BASE_URL ?? process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com").replace(/\/+$/, "")}/api/v1/chat/completions`

/**
 * Legacy non-streaming gateway turn. Explicit laneless fallback ONLY (#8712):
 * harness sends ("fable"/"codex") never reach this path — the local Fable
 * lane and the runtime host own those. The model slug is the single public
 * Khala model `openagents/khala` (khala-mini/pro/code and the old
 * "openagents-gateway-default" placeholder are dead slugs the gateway 400s).
 */
export const completeChatTurn = async (
  history: readonly DesktopMessage[],
  fetchImpl: typeof fetch = fetch,
): Promise<string> => {
  const token = process.env.OPENAGENTS_AGENT_TOKEN?.trim() || process.env.OPENAGENTS_INFERENCE_API_KEY?.trim()
  if (!token) throw new Error("No OpenAgents model token is configured on this desktop host.")
  const response = await fetchImpl(endpoint(), {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({
      model: process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL ?? "openagents/khala",
      stream: false,
      messages: [
        { role: "system", content: "You are OpenAgents Desktop. Answer directly and accurately. Never claim a Fleet run started without an authority receipt." },
        ...history.filter((message) => message.role !== "system").slice(-24).map((message) => ({ role: message.role, content: message.text })),
      ],
    }),
  })
  if (!response.ok) {
    // Bounded response-body detail: a bare status hid the actionable reason
    // (the live 400 that motivated #8712's fix carried one).
    const detail = (await response.text().catch(() => "")).replace(/\s+/g, " ").trim().slice(0, 300)
    throw new Error(`The model gateway returned ${response.status}.${detail === "" ? "" : ` ${detail}`}`)
  }
  const data = await response.json() as { choices?: Array<{ message?: { content?: unknown } }> }
  const content = data.choices?.[0]?.message?.content
  if (typeof content !== "string" || content.trim() === "") throw new Error("The model gateway returned no assistant message.")
  return content.trim()
}
