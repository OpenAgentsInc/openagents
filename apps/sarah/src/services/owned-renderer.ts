/**
 * Owned avatar render-service client for the Sarah avatar surface
 * (OAV-4 #8614, epic #8610; spec: docs/sarah/2026-07-09-owned-avatar-video-pipeline-spec.md §6/§8).
 *
 * The render service itself (idle/listen/speak state machine over our clip
 * library, MuseTalk lip sync, WebRTC egress) is OAV-2 and lives in the
 * hydralisk repo. This module codes to its CONTRACT:
 *
 *   POST   {SARAH_RENDER_SERVICE_URL}/sessions            (bearer) -> { session_id, webrtc: { offer_url | join info } }
 *   POST   {SARAH_RENDER_SERVICE_URL}/sessions/{id}/control (bearer)
 *          { type: "speak", event_id, audio_b64 }  — base64 PCM s16le 24k mono chunks, shared event_id per utterance
 *          { type: "speak_end", event_id }
 *          { type: "interrupt" | "start_listening" | "stop_listening" | "keepalive" }
 *   DELETE {SARAH_RENDER_SERVICE_URL}/sessions/{id}       (bearer)
 *
 * Renderer selection is flag-gated: SARAH_AVATAR_RENDERER=owned arms this
 * path; anything else keeps LiveAvatar as the default renderer (byte-equal
 * behavior on that lane). Session registry, caps, and usage JSONL mirror
 * liveavatar.ts mint/stop/reap symmetry so ops discipline is identical.
 *
 * The speaking bridge INVERTS the LiveAvatar flow: instead of the vendor
 * calling our brain (custom-LLM hook), our server runs the owned brain and
 * pushes synthesized PCM to the render service's speak API. TTS is the OAV-3
 * seam (SARAH_TTS_SERVICE_URL, same bearer pattern):
 *
 *   POST {SARAH_TTS_SERVICE_URL}/synthesize (bearer SARAH_TTS_SERVICE_TOKEN)
 *        { text, format: "pcm_s16le", sample_rate_hz: 24000 } -> binary PCM body
 *
 * Honest v1 scope: the owned path speaks TEXT-DRIVEN turns only (surface text
 * composer / server-side turn loop). User-mic ASR is out of scope until a
 * later lane adds it — the mic path remains LiveAvatar-only.
 */

import { appendFile, mkdir } from "node:fs/promises"
import { dirname, join } from "node:path"

import { runOwnedSarahTurn } from "../agent-runtime/owned-runtime.ts"
import { publishSarahAvatarEvent } from "./avatar-event-bus.ts"
import { persistSarahAvatarSession } from "./turn-store.ts"

export type SarahAvatarRenderer = "liveavatar" | "owned"

/** Flag-off default is LiveAvatar; SARAH_AVATAR_RENDERER=owned arms this path. */
export function sarahAvatarRenderer(): SarahAvatarRenderer {
  return process.env.SARAH_AVATAR_RENDERER?.trim() === "owned"
    ? "owned"
    : "liveavatar"
}

/**
 * SQ-4 (#8621): when the owned renderer is preferred but mint fails upstream
 * (busy/502), fall back to LiveAvatar if armed — so a cold GPU or full slot
 * never takes the surface offline. Local policy caps (our session/daily caps)
 * do NOT fall through: those are intentional load-shedding.
 *
 * Default ON when LiveAvatar is armed (`SARAH_AVATAR_OWNED_FALLBACK` unset).
 * Set `SARAH_AVATAR_OWNED_FALLBACK=off` to fail hard on owned mint errors.
 */
export function ownedMintFallsBackToLiveAvatar(): boolean {
  const raw = process.env.SARAH_AVATAR_OWNED_FALLBACK?.trim().toLowerCase()
  if (raw === "off" || raw === "0" || raw === "false" || raw === "none") {
    return false
  }
  if (raw === "liveavatar" || raw === "on" || raw === "1" || raw === "true") {
    return true
  }
  // Default: fall back when LiveAvatar can actually mint.
  return Boolean(process.env.LIVEAVATAR_API_KEY?.trim())
}

/** Errors where falling back to LiveAvatar is honest and useful. */
export function ownedMintErrorIsFallbackable(error: string): boolean {
  if (
    error === "avatar_session_cap_exceeded" ||
    error === "avatar_daily_cap_exceeded"
  ) {
    return false
  }
  return (
    error === "avatar_upstream_busy" ||
    error === "avatar_not_armed" ||
    error.startsWith("avatar_render_http_") ||
    error === "avatar_render_unreachable" ||
    error === "avatar_mint_failed"
  )
}

function renderServiceUrl(): string | null {
  const url = process.env.SARAH_RENDER_SERVICE_URL?.trim()
  return url ? url.replace(/\/+$/, "") : null
}

function ttsServiceUrl(): string | null {
  const url = process.env.SARAH_TTS_SERVICE_URL?.trim()
  return url ? url.replace(/\/+$/, "") : null
}

export function ownedRendererArmed(): boolean {
  return Boolean(renderServiceUrl() && process.env.SARAH_RENDER_SERVICE_TOKEN?.trim())
}

export function ownedTtsArmed(): boolean {
  return Boolean(ttsServiceUrl() && process.env.SARAH_TTS_SERVICE_TOKEN?.trim())
}

/** Same cap envs as the LiveAvatar lane — GPU sessions carry the same S-3 discipline. */
function ownedRendererCaps() {
  return {
    maxActiveSessions: Number(process.env.SARAH_AVATAR_MAX_ACTIVE_SESSIONS ?? 3),
    dailySessionCap: Number(process.env.SARAH_AVATAR_DAILY_SESSION_CAP ?? 100),
  }
}

// --- usage projection (same JSONL file + row type as liveavatar.ts, with a
// renderer discriminator so the two lanes stay distinguishable) --------------

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
      `${JSON.stringify({ type: "sarah.avatar_usage.v1", renderer: "owned", at: new Date().toISOString(), ...entry })}\n`,
    )
  } catch {
    // Usage projection is fail-soft.
  }
  if (typeof entry.event === "string" && typeof entry.sessionId === "string") {
    await persistSarahAvatarSession({
      event: entry.event,
      sessionId: entry.sessionId,
      conversationRef: String(entry.conversationRef ?? ""),
      ...(typeof entry.minutes === "number" ? { minutes: entry.minutes } : {}),
    })
  }
}

// --- render-service HTTP client ---------------------------------------------

async function renderServiceFetch(path: string, init: RequestInit): Promise<Response> {
  const base = renderServiceUrl()
  const token = process.env.SARAH_RENDER_SERVICE_TOKEN?.trim()
  if (!base || !token) throw new Error("owned_renderer_not_armed")
  return fetch(`${base}${path}`, {
    ...init,
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
      ...(init.headers ?? {}),
    },
  })
}

type OwnedActiveSession = {
  sessionId: string
  conversationRef: string
  prospectRef: string | null
  startedAt: number
  /** event_id of the utterance currently streaming to the speak API, if any. */
  speakingEventId: string | null
}

const activeSessions = new Map<string, OwnedActiveSession>()
let dailyCount = 0
let dailyKey = ""

function todayKey() {
  return new Date().toISOString().slice(0, 10)
}

export function isOwnedAvatarSession(sessionId: string): boolean {
  return activeSessions.has(sessionId)
}

export type OwnedMintResult =
  | {
      ok: true
      renderer: "owned"
      sessionId: string
      conversationRef: string
      /** Render-service WebRTC join info, passed through to the browser. */
      webrtc: Record<string, unknown>
      sandbox: boolean
      brainConfigured: boolean
    }
  | { ok: false; error: string; status: number; detail?: string }

export async function mintOwnedAvatarSession({
  prospectRef,
}: {
  prospectRef?: string | undefined
}): Promise<OwnedMintResult> {
  if (!ownedRendererArmed()) {
    return { ok: false, error: "avatar_not_armed", status: 503 }
  }
  const caps = ownedRendererCaps()
  if (dailyKey !== todayKey()) {
    dailyKey = todayKey()
    dailyCount = 0
  }
  if (activeSessions.size >= caps.maxActiveSessions) {
    return { ok: false, error: "avatar_session_cap_exceeded", status: 429 }
  }
  if (dailyCount >= caps.dailySessionCap) {
    return { ok: false, error: "avatar_daily_cap_exceeded", status: 429 }
  }

  const conversationRef = prospectRef
    ? `prospect:${prospectRef}`
    : `visitor:${crypto.randomUUID()}`

  try {
    const response = await renderServiceFetch("/sessions", {
      method: "POST",
      body: JSON.stringify({ conversation_ref: conversationRef }),
    })
    if (!response.ok) {
      const detail = (await response.text().catch(() => "")).slice(0, 200)
      const busy = response.status === 429 || response.status === 409
      return {
        ok: false,
        error: busy ? "avatar_upstream_busy" : `avatar_render_http_${response.status}`,
        detail,
        status: busy ? 429 : 502,
      }
    }
    const data = (await response.json()) as {
      session_id?: string
      webrtc?: Record<string, unknown>
    }
    if (!data.session_id || !data.webrtc) {
      return { ok: false, error: "avatar_render_missing_session", status: 502 }
    }

    activeSessions.set(data.session_id, {
      sessionId: data.session_id,
      conversationRef,
      prospectRef: prospectRef ?? null,
      startedAt: Date.now(),
      speakingEventId: null,
    })
    dailyCount += 1
    await recordUsage({
      event: "session_minted",
      sessionId: data.session_id,
      conversationRef,
      dailyCount,
    })

    return {
      ok: true,
      renderer: "owned",
      sessionId: data.session_id,
      conversationRef,
      webrtc: data.webrtc,
      // The owned renderer has no vendor sandbox tier; the field stays for
      // response-shape symmetry with the LiveAvatar mint.
      sandbox: false,
      brainConfigured: true,
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "avatar_mint_failed",
      status: 502,
    }
  }
}

export async function stopOwnedAvatarSession(sessionId: string): Promise<{
  ok: boolean
  minutes?: number
}> {
  const session = activeSessions.get(sessionId)
  if (!session) return { ok: true }
  activeSessions.delete(sessionId)
  const minutes = Math.max(1, Math.round((Date.now() - session.startedAt) / 60_000))
  try {
    await renderServiceFetch(`/sessions/${encodeURIComponent(sessionId)}`, {
      method: "DELETE",
    })
  } catch {
    // The render service's own idle timeout is the backstop.
  }
  await recordUsage({
    event: "session_stopped",
    sessionId,
    conversationRef: session.conversationRef,
    minutes,
  })
  return { ok: true, minutes }
}

/** Stop stale owned sessions (registry + render service) so caps never wedge. */
export function reapStaleOwnedSessions(maxAgeMs = 30 * 60_000): void {
  const cutoff = Date.now() - maxAgeMs
  for (const [, session] of activeSessions) {
    if (session.startedAt < cutoff) void stopOwnedAvatarSession(session.sessionId)
  }
}

export function ownedRendererStatus() {
  return {
    armed: ownedRendererArmed(),
    ttsArmed: ownedTtsArmed(),
    // Shape symmetry with the LiveAvatar status consumed by the surface.
    sandbox: false,
    brainConfigured: true,
    activeSessions: activeSessions.size,
    speechScope: "text_driven_v1_no_mic_asr",
  }
}

// --- PCM chunking (LITE guidance: ~600 ms first chunk, 1 s after) -----------

const PCM_SAMPLE_RATE_HZ = 24_000
const PCM_BYTES_PER_SAMPLE = 2 // s16le mono
const FIRST_CHUNK_MS = 600
const NEXT_CHUNK_MS = 1_000

function pcmBytesForMs(ms: number): number {
  return Math.floor((PCM_SAMPLE_RATE_HZ * ms) / 1000) * PCM_BYTES_PER_SAMPLE
}

/**
 * Split raw PCM (s16le 24k mono) into base64 speak chunks: ~600 ms first (fast
 * time-to-first-frame), 1 s after. Exported for tests.
 */
export function chunkPcmBase64(pcm: Uint8Array): string[] {
  if (pcm.length === 0) return []
  const chunks: string[] = []
  let offset = 0
  let size = pcmBytesForMs(FIRST_CHUNK_MS)
  while (offset < pcm.length) {
    chunks.push(Buffer.from(pcm.subarray(offset, offset + size)).toString("base64"))
    offset += size
    size = pcmBytesForMs(NEXT_CHUNK_MS)
  }
  return chunks
}

// --- TTS seam (OAV-3) --------------------------------------------------------

async function synthesizeSpeechPcm(text: string): Promise<
  { ok: true; pcm: Uint8Array } | { ok: false; error: string }
> {
  const base = ttsServiceUrl()
  const token = process.env.SARAH_TTS_SERVICE_TOKEN?.trim()
  if (!base || !token) return { ok: false, error: "tts_not_armed" }
  try {
    const response = await fetch(`${base}/synthesize`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        text,
        format: "pcm_s16le",
        sample_rate_hz: PCM_SAMPLE_RATE_HZ,
      }),
    })
    if (!response.ok) return { ok: false, error: `tts_http_${response.status}` }
    return { ok: true, pcm: new Uint8Array(await response.arrayBuffer()) }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "tts_failed",
    }
  }
}

// --- speech text shaping (SQ-4 #8621) -----------------------------------------

/**
 * The brain replies in Markdown; TTS must never vocalize its syntax
 * ("asterisk asterisk", "dash dash"). Strip structure, keep the words.
 * Exported for tests.
 */
export function toSpeakableText(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/(\*\*|__)(.*?)\1/g, "$2")
    .replace(/(\*|_)(.*?)\1/g, "$2")
    .replace(/^\s*>\s?/gm, "")
    .replace(/\|/g, " ")
    .replace(/\s+/g, " ")
    .trim()
}

/**
 * Split a reply into sentence groups of at least `minChars` so TTS can
 * stream per group: first audio lands after the FIRST group synthesizes,
 * not after the whole reply. Exported for tests.
 */
export function splitSpeakableSentences(text: string, minChars = 40): string[] {
  const sentences = text
    .split(/(?<=[.!?])\s+/)
    .map((sentence) => sentence.trim())
    .filter(Boolean)
  const groups: string[] = []
  let current = ""
  for (const sentence of sentences) {
    current = current ? `${current} ${sentence}` : sentence
    if (current.length >= minChars) {
      groups.push(current)
      current = ""
    }
  }
  if (current) {
    if (groups.length > 0) groups[groups.length - 1] += ` ${current}`
    else groups.push(current)
  }
  return groups
}

// --- the spoken greeting (owner requirement 2026-07-09: the session must
// never sit silent after connect — Sarah greets first, audibly) ---------------

export const SARAH_OWNED_GREETING = "Hello! I'm Sarah. What's on your mind today?"

/**
 * Speak the fixed greeting on a fresh owned session — no brain turn, just
 * TTS + the speak API — and feed the SSE transcript so the surface shows it.
 * Fail-soft: a TTS/render hiccup must not break the session; the greeting
 * still lands as a transcript line.
 */
export async function speakOwnedGreeting(sessionId: string): Promise<void> {
  const session = activeSessions.get(sessionId)
  if (!session) return
  publishSarahAvatarEvent(session.conversationRef, {
    type: "transcript",
    role: "assistant",
    text: SARAH_OWNED_GREETING,
  })
  const speech = await synthesizeSpeechPcm(SARAH_OWNED_GREETING)
  if (!speech.ok) return
  const eventId = crypto.randomUUID()
  session.speakingEventId = eventId
  try {
    for (const audioB64 of chunkPcmBase64(speech.pcm)) {
      if (session.speakingEventId !== eventId) return
      await sendControl(sessionId, {
        type: "speak",
        event_id: eventId,
        audio_b64: audioB64,
      })
    }
    await sendControl(sessionId, { type: "speak_end", event_id: eventId })
  } catch {
    // Fail-soft: greeting audio is best-effort; the transcript line landed.
  } finally {
    if (session.speakingEventId === eventId) session.speakingEventId = null
  }
}

// --- the speaking bridge (owned-path turn loop) -------------------------------

async function sendControl(
  sessionId: string,
  payload: Record<string, unknown>,
): Promise<void> {
  const response = await renderServiceFetch(
    `/sessions/${encodeURIComponent(sessionId)}/control`,
    { method: "POST", body: JSON.stringify(payload) },
  )
  if (!response.ok) throw new Error(`render_control_http_${response.status}`)
}

export type OwnedSpeakResult =
  | {
      ok: true
      reply: string
      /** false when the reply landed as text only (TTS/render degraded). */
      spoken: boolean
      speechError?: string
    }
  | { ok: false; error: string; status: number }

/**
 * One text-driven turn on the owned avatar path: run the owned brain, feed the
 * SSE bus (the surface's transcript source on this lane), synthesize via the
 * OAV-3 TTS seam, and stream PCM chunks to the render service speak API.
 *
 * Degradation is honest and layered: brain reply always reaches the SSE
 * transcript; when TTS or the speak API fails, the turn returns spoken=false
 * with the reason instead of dropping the reply.
 */
export async function speakOwnedAvatarTurn({
  sessionId,
  message,
}: {
  sessionId: string
  message: string
}): Promise<OwnedSpeakResult> {
  const session = activeSessions.get(sessionId)
  if (!session) {
    return { ok: false, error: "owned_session_not_found", status: 404 }
  }
  const trimmed = message.trim()
  if (!trimmed) return { ok: false, error: "empty_message", status: 400 }

  publishSarahAvatarEvent(session.conversationRef, {
    type: "transcript",
    role: "user",
    text: trimmed,
  })

  const turn = await runOwnedSarahTurn({
    message: trimmed,
    ...(session.prospectRef ? { prospectRef: session.prospectRef } : {}),
  })

  publishSarahAvatarEvent(session.conversationRef, {
    type: "transcript",
    role: "assistant",
    text: turn.reply,
  })

  // Barge-in: a new turn interrupts any utterance still streaming.
  if (session.speakingEventId) {
    session.speakingEventId = null
    await sendControl(sessionId, { type: "interrupt" }).catch(() => {})
  }

  // Sentence-streamed synthesis (SQ-4 #8621): synthesize and push per
  // sentence group under one event_id, so first audio lands after the first
  // group's TTS instead of the whole reply's.
  const speakable = toSpeakableText(turn.reply)
  const groups = splitSpeakableSentences(speakable)
  if (groups.length === 0) {
    return { ok: true, reply: turn.reply, spoken: false, speechError: "empty_speakable_text" }
  }

  const eventId = crypto.randomUUID()
  session.speakingEventId = eventId
  let spokeAnything = false
  try {
    for (const group of groups) {
      if (session.speakingEventId !== eventId) {
        return { ok: true, reply: turn.reply, spoken: spokeAnything, speechError: "interrupted" }
      }
      const speech = await synthesizeSpeechPcm(group)
      if (!speech.ok) {
        // First-group failure means nothing was spoken; later failures are
        // an honest partial with the reason attached.
        if (spokeAnything) {
          await sendControl(sessionId, { type: "speak_end", event_id: eventId }).catch(() => {})
        }
        return {
          ok: true,
          reply: turn.reply,
          spoken: spokeAnything,
          speechError: spokeAnything ? `partial:${speech.error}` : speech.error,
        }
      }
      for (const audioB64 of chunkPcmBase64(speech.pcm)) {
        // A newer turn interrupted this utterance — stop pushing stale audio.
        if (session.speakingEventId !== eventId) {
          return { ok: true, reply: turn.reply, spoken: spokeAnything, speechError: "interrupted" }
        }
        await sendControl(sessionId, {
          type: "speak",
          event_id: eventId,
          audio_b64: audioB64,
        })
        spokeAnything = true
      }
    }
    await sendControl(sessionId, { type: "speak_end", event_id: eventId })
    return { ok: true, reply: turn.reply, spoken: true }
  } catch (error) {
    return {
      ok: true,
      reply: turn.reply,
      spoken: false,
      speechError: error instanceof Error ? error.message : "speak_failed",
    }
  } finally {
    if (session.speakingEventId === eventId) session.speakingEventId = null
  }
}
