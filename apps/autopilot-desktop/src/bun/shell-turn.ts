// Bun-host shell-turn: a REAL model response for the zero-base shell text bar
// (HUD H5, #5503).
//
// The dead-simple default surface (a black screen with one text bar) sends each
// submitted prompt here via the `shellTurn` RPC verb. This module calls the
// now-LIVE OpenAgents inference gateway — the OpenAI-compatible
// `POST /v1/chat/completions` surface on openagents.com (Gemini 3.5 Flash on the
// free per-agent allowance) — authenticated with the desktop's configured
// OpenAgents agent token, and projects ONLY the model text back to the
// webview.
//
// SECRET BOUNDARY: the agent token lives ONLY in the Bun host env
// (`OPENAGENTS_AGENT_TOKEN`, with an optional shell-specific override). It is
// NEVER returned to the webview and is only ever placed in the outbound
// Authorization header here. Never hardcode or log a token.
//
// HONEST, NEVER-FAKE: if no token is configured (or the gateway/network is
// unavailable), this returns a clean, in-conversation message telling the owner
// how to configure it. It never invents a model answer. Jargon-free: the text
// the webview renders carries no session-ref / program-step / verdict /
// node-state vocabulary — just a plain answer or a plain "how to configure" note.

import {
  inferenceGatewayChatCompletionsUrl,
  resolveInferenceGatewaySettings,
} from "../shared/inference-gateway.js"
import type { ShellTurnResponse } from "../shared/rpc.js"

// The free-tier default model id the gateway routes to the first-party Vertex
// Gemini lane (mirrors the Worker's DEFAULT_CHAT_MODEL). An explicit
// OPENAGENTS_SHELL_MODEL / OPENAGENTS_INFERENCE_GATEWAY_MODEL override wins.
const DEFAULT_SHELL_MODEL = "gemini-3.5-flash"

// A short, neutral system steer so the shell answers as Autopilot and
// keeps out of internal-jargon territory.
export const SHELL_SYSTEM_PROMPT =
  "You are Autopilot, the OpenAgents desktop agent. Answer the user's message directly and concisely in plain language."

const NO_TOKEN_MESSAGE =
  "I can't reach a model yet — no OpenAgents account token is configured. " +
  "Set OPENAGENTS_AGENT_TOKEN in the desktop app's environment (the same token " +
  "the app uses to talk to openagents.com) and submit again. " +
  "You can create or copy a token from your account at https://openagents.com."

type ShellTurnEnv = Readonly<Record<string, string | undefined>>

export type BuildShellTurnInput = Readonly<{
  prompt: string
  env: ShellTurnEnv
  // The OpenAgents agent token (kept in the Bun host). Injected separately so
  // the builder is testable without reading process env and so the token is
  // never threaded through anything the webview can see.
  agentToken: string | null
  fetchFn?: typeof fetch
}>

// Resolve the agent token the shell turn authenticates with.
//
// Order (#5503 live-gateway fix):
//   1. An explicit env override — OPENAGENTS_SHELL_AGENT_TOKEN, then the same
//      OPENAGENTS_AGENT_TOKEN the desktop uses for openagents.com elsewhere
//      (promise-surfacing, node-launcher).
//   2. The persisted agent credential the desktop already mints + stores during
//      auto-onboarding (AO-1, `<PYLON_HOME>/agent-credential.json`). On a normal
//      install nothing sets the env var, so WITHOUT this fallback every shell
//      turn hit the no-token path. This makes the chat authenticate as the
//      owner's real agent identity with zero manual configuration.
//
// `readPersistedToken` is injected (and optional) so this stays a pure,
// env-only function for unit tests; the Bun host wires it to
// `loadPersistedCredential(onboardingHome())?.token`. The token is returned to
// the host only — it is never logged and never crosses to the webview.
export const resolveShellAgentToken = (
  env: ShellTurnEnv,
  readPersistedToken?: () => string | null,
): string | null => {
  const envToken =
    env.OPENAGENTS_SHELL_AGENT_TOKEN ?? env.OPENAGENTS_AGENT_TOKEN ?? null
  const trimmedEnv = envToken?.trim()
  if (trimmedEnv && trimmedEnv.length > 0) return trimmedEnv

  const persisted = readPersistedToken?.() ?? null
  const trimmedPersisted = persisted?.trim()
  return trimmedPersisted && trimmedPersisted.length > 0
    ? trimmedPersisted
    : null
}

// Pull the assistant text out of an OpenAI-compatible chat-completions body.
const parseAssistantText = (body: unknown): string | null => {
  if (typeof body !== "object" || body === null) return null
  const choices = (body as { choices?: unknown }).choices
  if (!Array.isArray(choices) || choices.length === 0) return null
  const first = choices[0] as { message?: { content?: unknown } } | undefined
  const content = first?.message?.content
  return typeof content === "string" && content.trim().length > 0
    ? content
    : null
}

// Pull a human-readable error out of the gateway's error body (`{ error, message }`).
const parseErrorMessage = (body: unknown, status: number): string => {
  if (typeof body === "object" && body !== null) {
    const record = body as { error?: unknown; message?: unknown }
    if (typeof record.message === "string" && record.message.trim().length > 0) {
      return record.message
    }
    if (typeof record.error === "string" && record.error.trim().length > 0) {
      return record.error
    }
  }
  return `request failed (${status})`
}

export const buildShellTurn = async (
  input: BuildShellTurnInput,
): Promise<ShellTurnResponse> => {
  const fetchFn = input.fetchFn ?? fetch
  const settings = resolveInferenceGatewaySettings(input.env)
  const model =
    input.env.OPENAGENTS_SHELL_MODEL?.trim() ||
    input.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL?.trim() ||
    DEFAULT_SHELL_MODEL
  const url = inferenceGatewayChatCompletionsUrl(settings.baseUrl)
  const prompt = input.prompt.trim()

  // Empty prompt: the reducer already no-ops an empty submit, but stay honest.
  if (prompt === "") {
    return { ok: false, text: "Type a message and submit to get a response." }
  }

  // NO-TOKEN honest path: never fake a model answer. Tell the owner how to wire
  // it up, in plain language, and do NO network call.
  const token =
    input.agentToken !== null && input.agentToken.trim().length > 0
      ? input.agentToken.trim()
      : null
  if (token === null) {
    return { ok: false, text: NO_TOKEN_MESSAGE }
  }

  try {
    const res = await fetchFn(url, {
      method: "POST",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        model,
        stream: false,
        messages: [
          { role: "system", content: SHELL_SYSTEM_PROMPT },
          { role: "user", content: prompt },
        ],
      }),
    })

    let body: unknown = null
    try {
      body = await res.json()
    } catch {
      body = null
    }

    if (!res.ok) {
      const reason = parseErrorMessage(body, res.status)
      // Map the well-known gateway errors to clean, actionable plain language.
      if (res.status === 401 || res.status === 403) {
        return {
          ok: false,
          text:
            "I couldn't authenticate with openagents.com. The configured " +
            "account token may be invalid or expired — refresh it and try again.",
        }
      }
      if (res.status === 402) {
        return {
          ok: false,
          text:
            "Your free model allowance is used up and there's no credit balance " +
            "left. Add credit at https://openagents.com to keep chatting.",
        }
      }
      if (res.status === 404) {
        return {
          ok: false,
          text:
            "The model gateway isn't available right now. Please try again in a " +
            "moment.",
        }
      }
      return {
        ok: false,
        text: `I couldn't get a response right now: ${reason}. Please try again.`,
      }
    }

    const text = parseAssistantText(body)
    if (text === null) {
      return {
        ok: false,
        text: "The model returned an empty response. Please try again.",
      }
    }
    return { ok: true, text }
  } catch (error) {
    const reason = error instanceof Error ? error.message : "network error"
    return {
      ok: false,
      text: `I couldn't reach the model (${reason}). Check your connection and try again.`,
    }
  }
}
