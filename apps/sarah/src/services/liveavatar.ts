/**
 * LiveAvatar (HeyGen) session service for the Sarah avatar surface
 * (#8598 AV-1/AV-4; assessment: docs/sarah/2026-07-09-liveavatar-integration-assessment.md).
 *
 * FULL Mode: LiveAvatar runs ASR/TTS/video; the brain is ours via the Custom
 * LLM add-on (see llm-openai-compat.ts). This module owns:
 *  - guarded session minting (origin check, active-session cap, daily cap,
 *    usage JSONL — avatar minutes are credit-metered, S-3 discipline applies)
 *  - per-session context creation: the base dashboard context (KB + opener)
 *    cloned with a `[conversation_ref: …]` marker so the brain endpoint can
 *    correlate LLM calls to the browser session
 *  - stop proxy + usage recording
 *
 * The LiveAvatar API key never reaches the frontend; the page only receives
 * the session token (safe — scoped to one session).
 */

import { appendFile, mkdir } from "node:fs/promises"

import { persistSarahAvatarSession } from "./turn-store.ts"
import { dirname, join } from "node:path"

const API_BASE = "https://api.liveavatar.com"
const SANDBOX_AVATAR_ID = "dd73ea75-1218-4ef3-92ce-606d5f7fbc0a"
const DEFAULT_BASE_CONTEXT_ID = "52e544bc-4207-4deb-b42f-fd028cf6b4cd"

export function liveAvatarArmed(): boolean {
  return Boolean(process.env.LIVEAVATAR_API_KEY?.trim())
}

export function sarahAvatarConfig() {
  const configuredAvatarId = process.env.SARAH_AVATAR_ID?.trim()
  return {
    avatarId: configuredAvatarId || SANDBOX_AVATAR_ID,
    // Sandbox until a real avatar id is configured (sandbox sessions are free
    // and ~1 minute). Production avatar: June HR preset via SARAH_AVATAR_ID
    // (owner selection 2026-07-09, #8598).
    sandbox: !configuredAvatarId || process.env.SARAH_AVATAR_SANDBOX === "1",
    voiceId: process.env.SARAH_AVATAR_VOICE_ID?.trim() || undefined,
    baseContextId:
      process.env.SARAH_AVATAR_BASE_CONTEXT_ID?.trim() || DEFAULT_BASE_CONTEXT_ID,
    llmConfigurationId: process.env.SARAH_AVATAR_LLM_CONFIG_ID?.trim() || undefined,
    maxActiveSessions: Number(process.env.SARAH_AVATAR_MAX_ACTIVE_SESSIONS ?? 3),
    dailySessionCap: Number(process.env.SARAH_AVATAR_DAILY_SESSION_CAP ?? 100),
  }
}

function usageFilePath() {
  return join(
    process.cwd(),
    ".sarah",
    process.env.SARAH_AVATAR_USAGE_FILE ?? "avatar-usage.jsonl",
  )
}

async function recordUsage(entry: Record<string, unknown>) {
  try {
    const path = usageFilePath()
    await mkdir(dirname(path), { recursive: true })
    await appendFile(
      path,
      `${JSON.stringify({ type: "sarah.avatar_usage.v1", at: new Date().toISOString(), ...entry })}\n`,
    )
  } catch {
    // Usage projection is fail-soft; the LiveAvatar dashboard stays authoritative for credits.
  }
  if (typeof entry.event === "string" && typeof entry.sessionId === "string") {
    await persistSarahAvatarSession({
      event: entry.event,
      sessionId: entry.sessionId,
      conversationRef: String(entry.conversationRef ?? ""),
      ...(typeof entry.sandbox === "boolean" ? { sandbox: entry.sandbox } : {}),
      ...(typeof entry.minutes === "number" ? { minutes: entry.minutes } : {}),
    })
  }
}

async function liveAvatarFetch(path: string, init: RequestInit): Promise<Response> {
  const key = process.env.LIVEAVATAR_API_KEY?.trim()
  if (!key) throw new Error("liveavatar_not_armed")
  return fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      "X-API-KEY": key,
      "content-type": "application/json",
      ...(init.headers ?? {}),
    },
  })
}

type ActiveSession = {
  sessionId: string
  sessionToken: string
  conversationRef: string
  contextId: string
  startedAt: number
}

const activeSessions = new Map<string, ActiveSession>()
let dailyCount = 0
let dailyKey = ""

function todayKey() {
  return new Date().toISOString().slice(0, 10)
}

let cachedBaseContext: { prompt: string; openingText: string; at: number } | null = null

async function baseContext(): Promise<{ prompt: string; openingText: string }> {
  // 5-minute TTL so dashboard copy edits (like the opener) propagate without
  // an instance roll.
  if (cachedBaseContext && Date.now() - cachedBaseContext.at < 5 * 60_000) {
    return cachedBaseContext
  }
  const config = sarahAvatarConfig()
  const response = await liveAvatarFetch(`/v1/contexts/${config.baseContextId}`, {
    method: "GET",
  })
  if (!response.ok) throw new Error(`liveavatar_context_http_${response.status}`)
  const data = (await response.json()) as {
    data?: { prompt?: string; opening_text?: string }
  }
  cachedBaseContext = {
    prompt: data.data?.prompt ?? "You are Sarah, OpenAgents' AI sales employee. Disclose you are an AI.",
    openingText: data.data?.opening_text ?? "Hello! I'm Sarah. What's on your mind today?",
    at: Date.now(),
  }
  return cachedBaseContext
}

export type MintResult =
  | {
      ok: true
      sessionToken: string
      sessionId: string
      conversationRef: string
      sandbox: boolean
      brainConfigured: boolean
    }
  | { ok: false; error: string; status: number; detail?: string }

export async function mintSarahAvatarSession({
  prospectRef,
}: {
  prospectRef?: string | undefined
}): Promise<MintResult> {
  if (!liveAvatarArmed()) {
    return { ok: false, error: "avatar_not_armed", status: 503 }
  }
  const config = sarahAvatarConfig()

  if (dailyKey !== todayKey()) {
    dailyKey = todayKey()
    dailyCount = 0
  }
  if (activeSessions.size >= config.maxActiveSessions) {
    return { ok: false, error: "avatar_session_cap_exceeded", status: 429 }
  }
  if (dailyCount >= config.dailySessionCap) {
    return { ok: false, error: "avatar_daily_cap_exceeded", status: 429 }
  }

  const conversationRef = prospectRef
    ? `prospect:${prospectRef}`
    : `visitor:${crypto.randomUUID()}`

  try {
    const base = await baseContext()
    // Context names must be unique account-wide (LiveAvatar rejects
    // duplicates — the 2026-07-09 returning-prospect 502). Identity lives in
    // the prompt marker, so the name just gets a per-mint suffix; contexts
    // are deleted at session stop and GC'd by the reaper.
    const contextResponse = await liveAvatarFetch("/v1/contexts", {
      method: "POST",
      body: JSON.stringify({
        name: `sarah-session-${crypto.randomUUID().slice(0, 8)}-${conversationRef.slice(0, 48)}`,
        prompt: `${base.prompt}\n\n[conversation_ref: ${conversationRef}]`,
        opening_text: base.openingText,
      }),
    })
    if (!contextResponse.ok) {
      const detail = (await contextResponse.text().catch(() => "")).slice(0, 200)
      return {
        ok: false,
        error: `avatar_context_http_${contextResponse.status}`,
        detail,
        status: 502,
      }
    }
    const contextData = (await contextResponse.json()) as { data?: { id?: string } }
    const contextId = contextData.data?.id
    if (!contextId) return { ok: false, error: "avatar_context_missing_id", status: 502 }

    const mintToken = () =>
      liveAvatarFetch("/v1/sessions/token", {
        method: "POST",
        body: JSON.stringify({
          mode: "FULL",
          avatar_id: config.avatarId,
          ...(config.sandbox ? { is_sandbox: true } : {}),
          ...(config.llmConfigurationId
            ? { llm_configuration_id: config.llmConfigurationId }
            : {}),
          avatar_persona: {
            context_id: contextId,
            language: "en",
            ...(config.voiceId ? { voice_id: config.voiceId } : {}),
          },
        }),
      })
    let tokenResponse = await mintToken()
    if (!tokenResponse.ok && tokenResponse.status < 500) {
      // Most 4xx here are LiveAvatar-side concurrency/plan limits (their
      // sessions self-expire after ~5 idle minutes — the 2026-07-09 owner
      // 502). Force-stop anything stale we still track, then retry once.
      await Promise.all(
        [...activeSessions.values()]
          .filter((session) => Date.now() - session.startedAt > 3 * 60_000)
          .map((session) => stopSarahAvatarSession(session.sessionId)),
      )
      tokenResponse = await mintToken()
    }
    if (!tokenResponse.ok) {
      const detail = (await tokenResponse.text().catch(() => "")).slice(0, 200)
      const busy = tokenResponse.status < 500
      return {
        ok: false,
        error: busy ? "avatar_upstream_busy" : `avatar_token_http_${tokenResponse.status}`,
        detail,
        status: busy ? 429 : 502,
      }
    }
    const tokenData = (await tokenResponse.json()) as {
      data?: { session_id?: string; session_token?: string }
    }
    const sessionId = tokenData.data?.session_id
    const sessionToken = tokenData.data?.session_token
    if (!sessionId || !sessionToken) {
      return { ok: false, error: "avatar_token_missing", status: 502 }
    }

    activeSessions.set(sessionId, {
      sessionId,
      sessionToken,
      conversationRef,
      contextId,
      startedAt: Date.now(),
    })
    dailyCount += 1
    await recordUsage({
      event: "session_minted",
      sessionId,
      conversationRef,
      sandbox: config.sandbox,
      dailyCount,
    })

    return {
      ok: true,
      sessionToken,
      sessionId,
      conversationRef,
      sandbox: config.sandbox,
      brainConfigured: Boolean(config.llmConfigurationId),
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "avatar_mint_failed",
      status: 502,
    }
  }
}

export async function stopSarahAvatarSession(sessionId: string): Promise<{
  ok: boolean
  minutes?: number
}> {
  const session = activeSessions.get(sessionId)
  if (!session) return { ok: true }
  activeSessions.delete(sessionId)
  const minutes = Math.max(1, Math.round((Date.now() - session.startedAt) / 60_000))
  try {
    await fetch(`${API_BASE}/v1/sessions/stop`, {
      method: "POST",
      headers: { Authorization: `Bearer ${session.sessionToken}` },
    })
  } catch {
    // The 5-minute inactivity timeout is the backstop.
  }
  // Per-session contexts are disposable — delete to keep the account tidy.
  try {
    await liveAvatarFetch("/v1/contexts/" + session.contextId, { method: "DELETE" })
  } catch {
    // Context GC is hygiene, never a failure.
  }
  await recordUsage({
    event: "session_stopped",
    sessionId,
    conversationRef: session.conversationRef,
    minutes,
  })
  return { ok: true, minutes }
}

/** Stop stale sessions for real (upstream + registry) so caps never wedge. */
export function reapStaleAvatarSessions(maxAgeMs = 30 * 60_000): void {
  const cutoff = Date.now() - maxAgeMs
  for (const [, session] of activeSessions) {
    if (session.startedAt < cutoff) void stopSarahAvatarSession(session.sessionId)
  }
  void gcOrphanedSessionContexts()
}

/** Delete leaked sarah-session-* contexts (crashed instances, pre-GC code). */
let lastContextGc = 0
async function gcOrphanedSessionContexts(): Promise<void> {
  if (Date.now() - lastContextGc < 10 * 60_000) return
  lastContextGc = Date.now()
  try {
    const response = await liveAvatarFetch("/v1/contexts?page_size=100", { method: "GET" })
    if (!response.ok) return
    const data = (await response.json()) as {
      data?: { results?: Array<{ id: string; name?: string; created_at?: string }> }
    }
    const active = new Set([...activeSessions.values()].map((s) => s.contextId))
    const cutoff = Date.now() - 60 * 60_000
    for (const context of data.data?.results ?? []) {
      if (!context.name?.startsWith("sarah-session-")) continue
      if (active.has(context.id)) continue
      const created = context.created_at ? Date.parse(`${context.created_at}Z`) : 0
      if (created > cutoff) continue
      await liveAvatarFetch(`/v1/contexts/${context.id}`, { method: "DELETE" }).catch(() => {})
    }
  } catch {
    // GC is hygiene, never a failure.
  }
}

export function sarahAvatarStatus() {
  const config = sarahAvatarConfig()
  return {
    armed: liveAvatarArmed(),
    sandbox: config.sandbox,
    brainConfigured: Boolean(config.llmConfigurationId),
    activeSessions: activeSessions.size,
  }
}
