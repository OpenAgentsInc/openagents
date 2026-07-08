// The Pylon agent surface (issue #4713; sprint
// docs/pylon/2026-06-10-v03-sprint-agent-economy.md): forum commands
// carrying the registered local identity, a small inspectable local
// memory store, and model adapters (a local model endpoint or the
// user's own Gemini key - the user's identity AND the user's inference;
// never a platform key from the device).

import { existsSync } from "node:fs"
import { appendFile, mkdir, readFile } from "node:fs/promises"
import { dirname, join } from "node:path"

import type { TipsNetworkOptions } from "./tips.js"

// ---------------------------------------------------------------------------
// Forum surface

async function forumRequest(
  options: TipsNetworkOptions,
  input: { path: string; method: "GET" | "POST"; body?: Record<string, unknown>; idempotencyKey?: string },
): Promise<Record<string, unknown>> {
  const headers: Record<string, string> = { "Content-Type": "application/json" }
  const token = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (input.method === "POST") {
    if (!token) throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for forum writes")
    headers.Authorization = `Bearer ${token}`
    if (input.idempotencyKey) headers["Idempotency-Key"] = input.idempotencyKey
  }
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers,
    method: input.method,
  })
  const text = await response.text()
  const payload = text.trim() ? JSON.parse(text) as Record<string, unknown> : {}
  if (!response.ok) {
    const reason = typeof payload.reason === "string" ? payload.reason : typeof payload.error === "string" ? payload.error : String(response.status)
    throw new Error(`pylon forum request failed (${response.status}): ${reason}`)
  }
  return payload
}

export async function forumPostTopic(
  options: TipsNetworkOptions,
  input: { forumSlug: string; title: string; bodyText: string },
): Promise<Record<string, unknown>> {
  const now = options.now?.() ?? new Date()
  return forumRequest(options, {
    body: { bodyText: input.bodyText, title: input.title },
    idempotencyKey: `pylon-forum-topic:${input.title.slice(0, 40)}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    path: `/api/forum/forums/${encodeURIComponent(input.forumSlug)}/topics`,
  })
}

export async function forumReply(
  options: TipsNetworkOptions,
  input: { topicId: string; bodyText: string },
): Promise<Record<string, unknown>> {
  const now = options.now?.() ?? new Date()
  return forumRequest(options, {
    body: { bodyText: input.bodyText },
    idempotencyKey: `pylon-forum-reply:${input.topicId.slice(0, 12)}:${now.toISOString().slice(0, 16)}`,
    method: "POST",
    path: `/api/forum/topics/${encodeURIComponent(input.topicId)}/posts`,
  })
}

export async function forumReadTopic(
  options: TipsNetworkOptions,
  topicId: string,
): Promise<Record<string, unknown>> {
  return forumRequest(options, {
    method: "GET",
    path: `/api/forum/topics/${encodeURIComponent(topicId)}`,
  })
}

// ---------------------------------------------------------------------------
// Local memory store: JSONL, file-backed, inspectable, never synced
// off-device by Pylon itself.

export type PylonMemoryEntry = {
  at: string
  kind: string
  summary: string
  refs?: Record<string, unknown>
}

export function memoryPathFor(pylonHome: string): string {
  return join(pylonHome, "memories.jsonl")
}

export async function appendMemory(
  pylonHome: string,
  entry: PylonMemoryEntry,
): Promise<void> {
  const path = memoryPathFor(pylonHome)
  await mkdir(dirname(path), { recursive: true })
  await appendFile(path, `${JSON.stringify(entry)}\n`, "utf8")
}

export async function readMemories(
  pylonHome: string,
  limit = 50,
): Promise<PylonMemoryEntry[]> {
  const path = memoryPathFor(pylonHome)
  if (!existsSync(path)) return []
  const lines = (await readFile(path, "utf8")).split("\n").filter((line) => line.trim() !== "")
  const entries: PylonMemoryEntry[] = []
  for (const line of lines.slice(-limit)) {
    try {
      entries.push(JSON.parse(line) as PylonMemoryEntry)
    } catch {
      // skip malformed lines; the store stays inspectable and forgiving
    }
  }
  return entries
}

// ---------------------------------------------------------------------------
// Model adapters: runtime config, not authority. Either a local
// OpenAI-compatible endpoint (PYLON_LOCAL_MODEL_URL [+ PYLON_LOCAL_MODEL])
// or the user's own Gemini key (GEMINI_API_KEY).

export type ModelAdapter = {
  kind: "local" | "gemini" | "none"
  complete: (input: { system: string; prompt: string }) => Promise<string>
}

export function resolveModelAdapter(env: Record<string, string | undefined>): ModelAdapter {
  const localUrl = env.PYLON_LOCAL_MODEL_URL?.trim()
  if (localUrl) {
    const model = env.PYLON_LOCAL_MODEL?.trim() || "default"
    return {
      complete: async ({ prompt, system }) => {
        const response = await fetch(new URL("/v1/chat/completions", localUrl), {
          body: JSON.stringify({
            messages: [
              { content: system, role: "system" },
              { content: prompt, role: "user" },
            ],
            model,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        })
        if (!response.ok) throw new Error(`local model request failed (${response.status})`)
        const payload = await response.json() as { choices?: Array<{ message?: { content?: string } }> }
        const text = payload.choices?.[0]?.message?.content?.trim()
        if (!text) throw new Error("local model returned no content")
        return text
      },
      kind: "local",
    }
  }

  const geminiKey = env.GEMINI_API_KEY?.trim()
  if (geminiKey) {
    const model = env.PYLON_GEMINI_MODEL?.trim() || "gemini-3.5-flash"
    return {
      complete: async ({ prompt, system }) => {
        const response = await fetch(
          `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent`,
          {
            body: JSON.stringify({
              contents: [{ parts: [{ text: prompt }] }],
              systemInstruction: { parts: [{ text: system }] },
            }),
            headers: {
              "Content-Type": "application/json",
              "x-goog-api-key": geminiKey,
            },
            method: "POST",
          },
        )
        if (!response.ok) throw new Error(`gemini request failed (${response.status})`)
        const payload = await response.json() as {
          candidates?: Array<{ content?: { parts?: Array<{ text?: string }> } }>
        }
        const text = payload.candidates?.[0]?.content?.parts?.map((part) => part.text ?? "").join("").trim()
        if (!text) throw new Error("gemini returned no content")
        return text
      },
      kind: "gemini",
    }
  }

  return {
    complete: async () => {
      throw new Error("no model adapter configured (set PYLON_LOCAL_MODEL_URL or GEMINI_API_KEY)")
    },
    kind: "none",
  }
}

// ---------------------------------------------------------------------------
// ask-artanis: the guided flow. Formats the device question with real
// device capability context plus recent memories into a Forum topic in
// the Artanis forum, where the responder loop (issues #4714-#4715)
// answers. The post is composed by the configured model adapter when one
// exists, otherwise a clean structured template - never a fabricated
// device claim either way.

export const ARTANIS_FORUM_SLUG = "artanis"

export type AskArtanisInput = {
  question: string
  pylonRef: string
  deviceContext: Record<string, unknown>
  memories: PylonMemoryEntry[]
}

export function askArtanisFallbackBody(input: AskArtanisInput): string {
  const memoryLines = input.memories
    .slice(-5)
    .map((entry) => `- ${entry.at}: ${entry.summary}`)
    .join("\n")
  return [
    `Question for Artanis from a Pylon device (${input.pylonRef}):`,
    "",
    input.question,
    "",
    "Device context (reported by the Pylon itself):",
    "```json",
    JSON.stringify(input.deviceContext, null, 2),
    "```",
    ...(memoryLines === "" ? [] : ["", "Recent device activity:", memoryLines]),
    "",
    "Posted via `pylon ask-artanis` (v0.3 agent surface).",
  ].join("\n")
}

export async function composeAskArtanisBody(
  input: AskArtanisInput,
  adapter: ModelAdapter,
): Promise<{ bodyText: string; composedBy: ModelAdapter["kind"] }> {
  if (adapter.kind === "none") {
    return { bodyText: askArtanisFallbackBody(input), composedBy: "none" }
  }
  try {
    const bodyText = await adapter.complete({
      prompt: [
        `The user's question: ${input.question}`,
        `Device context JSON (only source of device facts; do not invent any): ${JSON.stringify(input.deviceContext)}`,
        `Recent device memories: ${JSON.stringify(input.memories.slice(-5))}`,
      ].join("\n"),
      system:
        "You compose a concise Forum post from a Pylon device asking the network administrator (Artanis) a question. Include the user's question, the relevant device facts from the provided context verbatim (never invent hardware claims), and end with: Posted via pylon ask-artanis (v0.3 agent surface). Plain text, no markdown headers.",
    })
    return { bodyText, composedBy: adapter.kind }
  } catch {
    return { bodyText: askArtanisFallbackBody(input), composedBy: "none" }
  }
}
