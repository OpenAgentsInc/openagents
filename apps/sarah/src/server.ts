/**
 * Sarah Bun HTTP service — served at openagents.com/sarah/*
 *
 * Authority: openagents.com Worker APIs remain system of record.
 * No Next.js. Zero React. Routes are contract-compatible with the private
 * sarah app's /api/* paths, mounted under /sarah/api/*.
 */

import { join } from "node:path"
import { fileURLToPath } from "node:url"

import {
  checkSarahRealtimeTokenRequest,
  recordSarahRealtimeTokenMint,
  SARAH_PROSPECT_COOKIE,
  sarahRealtimeTokenTestMode,
} from "./services/realtime-token-guard.ts"
import { getSarahRealtimeToolDefinitions } from "./services/realtime-tools.ts"
import {
  sarahActiveModelId,
  sarahInferenceTransport,
} from "./services/google-inference.ts"
import {
  mintSarahProspectRef,
  readSarahProspectRef,
  setSarahProspectCookie,
  threadIdForProspectRef,
} from "./services/prospect-session.ts"
import { getSarahRealtimeInstructions } from "./services/sarah-instructions.ts"
import { listSarahProspectSessions } from "./services/session-index.ts"
import { processDueSarahFollowUps } from "./services/follow-up-scheduler.ts"
import { enqueueSarahEmailDraft } from "./services/crm-email-rail.ts"
import { runOwnedSarahTurn } from "./agent-runtime/owned-runtime.ts"
import { handleSarahChatCompletions } from "./llm-openai-compat.ts"
import { sarahAnswerCacheStatus } from "./services/semantic-answer-cache.ts"
import {
  approveLearningCandidate,
  distillLearningCandidates,
  listLearningCandidates,
  listLearningReceipts,
  rejectLearningCandidate,
  sarahCollectiveLearningStatus,
} from "./services/collective-learning.ts"
import { sarahTurnStoreStatus } from "./services/turn-store.ts"
import { sarahEcosystemStatus } from "./services/ecosystem-tools.ts"
import {
  getCurrentCustomerBlueprintMapSeed,
  listCustomerBlueprintsForOperator,
} from "./services/customer-blueprint.ts"
import {
  addSarahBlueprintFact,
  loadSarahBlueprint,
  promoteLearningToBlueprintFact,
  retireSarahBlueprintFact,
  sarahBlueprintStatus,
  type BlueprintFactFormat,
} from "./services/sarah-blueprint.ts"
import {
  getSarahAccountLinkStatus,
  linkSarahProspectAccount,
  resolveOpenAgentsSession,
} from "./services/account-link.ts"
import { sarahAvatarEventStream } from "./services/avatar-event-bus.ts"
import {
  mintSarahAvatarSession,
  reapStaleAvatarSessions,
  sarahAvatarStatus,
  stopSarahAvatarSession,
} from "./services/liveavatar.ts"
import {
  isOwnedAvatarSession,
  mintOwnedAvatarSession,
  ownedRendererStatus,
  reapStaleOwnedSessions,
  sarahAvatarRenderer,
  speakOwnedAvatarTurn,
  speakOwnedGreeting,
  stopOwnedAvatarSession,
} from "./services/owned-renderer.ts"

const __dirname = fileURLToPath(new URL(".", import.meta.url))
/** Overridable for Cloud Run monolith bundle (UI copied beside server.js). */
const UI_DIR =
  process.env.SARAH_UI_DIR?.trim() || join(__dirname, "ui")
const PREFIX = "/sarah"

function json(data: unknown, init: ResponseInit = {}) {
  const headers = new Headers(init.headers)
  headers.set("content-type", "application/json; charset=utf-8")
  return new Response(JSON.stringify(data), { ...init, headers })
}

function stripPrefix(pathname: string) {
  if (pathname === PREFIX || pathname.startsWith(`${PREFIX}/`)) {
    return pathname.slice(PREFIX.length) || "/"
  }
  return pathname
}

function setProspectCookieOn(
  response: Response,
  prospectRef: string,
  maxAgeSeconds: number,
) {
  const secure = process.env.NODE_ENV === "production" ? "; Secure" : ""
  response.headers.append(
    "set-cookie",
    `${SARAH_PROSPECT_COOKIE}=${encodeURIComponent(prospectRef)}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${maxAgeSeconds}${secure}`,
  )
}

async function handleRealtimeToken(request: Request): Promise<Response> {
  const guard = checkSarahRealtimeTokenRequest(request)
  if (!guard.ok) {
    const response = json(
      { error: guard.error },
      {
        status: guard.status,
        headers: guard.error.retryAfterMs
          ? { "retry-after": String(Math.ceil(guard.error.retryAfterMs / 1000)) }
          : undefined,
      },
    )
    if (guard.prospectRef && guard.setProspectCookie) {
      setProspectCookieOn(response, guard.prospectRef, 60 * 60 * 24 * 365)
    }
    return response
  }

  let tokenResult: { token: string; url: string }
  if (sarahRealtimeTokenTestMode()) {
    tokenResult = {
      token: "sarah-local-test-token",
      url: "wss://example.invalid/sarah-local-test-realtime",
    }
  } else {
    try {
      const { gateway } = await import("@ai-sdk/gateway")
      tokenResult = await gateway.experimental_realtime.getToken({
        model: "openai/gpt-realtime-2",
      })
    } catch (error) {
      return json(
        {
          error: {
            code: "gateway_unavailable",
            message:
              error instanceof Error
                ? error.message
                : "realtime gateway unavailable",
          },
        },
        { status: 503 },
      )
    }
  }

  await recordSarahRealtimeTokenMint()
  const tools = await getSarahRealtimeToolDefinitions()
  const response = json({
    token: tokenResult.token,
    url: tokenResult.url,
    tools,
    expiresAt: guard.activeSessionExpiresAt,
  })
  if (guard.setProspectCookie) {
    setProspectCookieOn(response, guard.prospectRef, 60 * 60 * 24 * 365)
  }
  return response
}

async function handleProspectSession(request: Request): Promise<Response> {
  let prospectRef = readSarahProspectRef(request)
  let minted = false
  if (!prospectRef) {
    prospectRef = mintSarahProspectRef()
    minted = true
  }
  const response = json({
    prospectRef,
    threadId: threadIdForProspectRef(prospectRef),
    minted,
  })
  if (minted) {
    setProspectCookieOn(response, prospectRef, 60 * 60 * 24 * 365)
  }
  return response
}

async function handleSessionConfig(): Promise<Response> {
  const instructions = await getSarahRealtimeInstructions()
  return json({
    instructions,
    inputAudioTranscription: {},
    voice: process.env.SARAH_VOICE ?? "alloy",
    turnDetection: { type: "server-vad" },
  })
}

async function handleOperatorProspects(): Promise<Response> {
  const prospects = await listSarahProspectSessions()
  return json({ prospects })
}

async function handleOperatorFollowUps(request: Request): Promise<Response> {
  if (request.method === "POST") {
    const result = await processDueSarahFollowUps()
    return json(result)
  }
  return json({ ok: true, detail: "POST to process due follow-ups" })
}

async function handleOperatorEmailDrafts(request: Request): Promise<Response> {
  if (request.method !== "POST") {
    return json({ error: "method_not_allowed" }, { status: 405 })
  }
  const body = (await request.json()) as Record<string, unknown>
  const result = await enqueueSarahEmailDraft({
    to: String(body.to ?? ""),
    subject: String(body.subject ?? ""),
    bodyText: String(body.bodyText ?? body.body ?? ""),
    prospectRef:
      typeof body.prospectRef === "string" ? body.prospectRef : undefined,
    contactId: typeof body.contactId === "string" ? body.contactId : undefined,
    sourceRef:
      typeof body.sourceRef === "string" ? body.sourceRef : "operator_http",
  })
  const ok = "ok" in result ? result.ok : true
  return json(result, { status: ok ? 200 : 400 })
}

/**
 * Admin bearer guard for the KHS-4 collective-learning operator endpoints
 * (#8603). Same posture as the monolith's operator-write routes
 * (OPENAGENTS_ADMIN_API_TOKEN bearer) and Sarah's own avatar-brain bearer:
 * fail CLOSED — with no token configured the endpoints are 503 (an approve
 * without the guard is impossible), and a missing/wrong bearer is 401.
 * `SARAH_OPERATOR_ADMIN_TOKEN` allows a Sarah-scoped token; it falls back to
 * the shared `OPENAGENTS_ADMIN_API_TOKEN` in the monolith deployment.
 */
function checkOperatorAdmin(request: Request): Response | null {
  const token =
    process.env.SARAH_OPERATOR_ADMIN_TOKEN?.trim() ||
    process.env.OPENAGENTS_ADMIN_API_TOKEN?.trim()
  if (!token) {
    return json(
      { error: { code: "operator_admin_not_armed" } },
      { status: 503 },
    )
  }
  const auth = request.headers.get("authorization") ?? ""
  if (auth !== `Bearer ${token}`) {
    return json({ error: { code: "unauthorized" } }, { status: 401 })
  }
  return null
}

async function handleOperatorLearning(
  request: Request,
  apiPath: string,
): Promise<Response> {
  const denied = checkOperatorAdmin(request)
  if (denied) return denied

  if (apiPath === "/api/operator/learning" && request.method === "GET") {
    const [pending, approved, receipts] = await Promise.all([
      listLearningCandidates("pending"),
      listLearningCandidates("approved"),
      listLearningReceipts(),
    ])
    // SQ-6 (#8623): candidates carry taxonomy / whyGeneralize / exampleCount
    // / sourceRecency; the summary gives the reviewer per-taxonomy pending
    // counts for triage at a glance.
    const taxonomySummary: Record<string, number> = {}
    for (const candidate of pending) {
      taxonomySummary[candidate.taxonomy] =
        (taxonomySummary[candidate.taxonomy] ?? 0) + 1
    }
    return json({
      pending,
      approved,
      receipts,
      taxonomySummary,
      status: sarahCollectiveLearningStatus(),
    })
  }
  if (apiPath === "/api/operator/learning/distill" && request.method === "POST") {
    const result = await distillLearningCandidates()
    return json(result, { status: result.ok ? 200 : 503 })
  }
  const decision = apiPath.match(
    /^\/api\/operator\/learning\/([^/]+)\/(approve|reject)$/,
  )
  if (decision && request.method === "POST") {
    const [, id, action] = decision
    const body = (await request.json().catch(() => ({}))) as {
      by?: string
      answer?: string
      reason?: string
    }
    const by = typeof body.by === "string" && body.by.trim() ? body.by : "owner"
    const result =
      action === "approve"
        ? await approveLearningCandidate({
            id: decodeURIComponent(id!),
            by,
            answerText: typeof body.answer === "string" ? body.answer : undefined,
          })
        : await rejectLearningCandidate({
            id: decodeURIComponent(id!),
            by,
            reason: typeof body.reason === "string" ? body.reason : undefined,
          })
    if (!result.ok) {
      const status = result.error === "candidate_not_found" ? 404 : 409
      return json({ error: { code: result.error } }, { status })
    }
    return json(result)
  }
  return json({ error: "not_found", path: apiPath }, { status: 404 })
}

/**
 * KHS-5 (#8604) Sarah's Blueprint operator endpoints. Same fail-closed admin
 * posture as the KHS-4 learning routes: unarmed → 503, wrong bearer → 401.
 * Every write goes through a receipted revision (add/retire with a change
 * note); retiring never deletes. The KHS-4 seam: POST .../promote turns an
 * owner-APPROVED winning_answer learning into a playbook fact whose
 * provenance is the approval receipt (learning_receipt:<id>).
 */
async function handleOperatorBlueprint(
  request: Request,
  apiPath: string,
): Promise<Response> {
  const denied = checkOperatorAdmin(request)
  if (denied) return denied

  if (apiPath === "/api/operator/blueprint" && request.method === "GET") {
    const blueprint = await loadSarahBlueprint()
    return json({ ...blueprint, status: sarahBlueprintStatus() })
  }
  if (
    apiPath === "/api/operator/blueprint/facts" &&
    request.method === "POST"
  ) {
    const body = (await request.json().catch(() => ({}))) as {
      action?: string
      factId?: string
      section?: string
      statement?: string
      heading?: string
      format?: string
      source?: string
      ref?: string
      dealRuleRefs?: unknown
      promiseIds?: unknown
      by?: string
      changeNote?: string
    }
    const by = typeof body.by === "string" && body.by.trim() ? body.by : "owner"
    const changeNote =
      typeof body.changeNote === "string" ? body.changeNote : ""
    const stringArray = (value: unknown): string[] =>
      Array.isArray(value) ? value.filter((v) => typeof v === "string") : []
    const result =
      body.action === "retire"
        ? await retireSarahBlueprintFact({
            factId: typeof body.factId === "string" ? body.factId : "",
            by,
            changeNote,
          })
        : await addSarahBlueprintFact({
            section: typeof body.section === "string" ? body.section : "",
            statement:
              typeof body.statement === "string" ? body.statement : "",
            heading: typeof body.heading === "string" ? body.heading : null,
            format:
              typeof body.format === "string"
                ? (body.format as BlueprintFactFormat)
                : undefined,
            source:
              typeof body.source === "string" ? body.source : "owner_directive",
            ref: typeof body.ref === "string" ? body.ref : null,
            dealRuleRefs: stringArray(body.dealRuleRefs),
            promiseIds: stringArray(body.promiseIds),
            by,
            changeNote,
          })
    if (!result.ok) {
      const status =
        result.error === "fact_not_found"
          ? 404
          : result.error === "already_retired" ||
              result.error === "fact_already_exists"
            ? 409
            : result.error === "store_write_failed" ||
                result.error === "blueprint_store_unavailable"
              ? 503
              : 400
      return json({ error: { code: result.error } }, { status })
    }
    return json(result)
  }
  if (
    apiPath === "/api/operator/blueprint/promote" &&
    request.method === "POST"
  ) {
    const body = (await request.json().catch(() => ({}))) as {
      candidateId?: string
      by?: string
      changeNote?: string
    }
    const result = await promoteLearningToBlueprintFact({
      candidateId: typeof body.candidateId === "string" ? body.candidateId : "",
      by:
        typeof body.by === "string" && body.by.trim() ? body.by : "owner",
      changeNote:
        typeof body.changeNote === "string" ? body.changeNote : undefined,
    })
    if (!result.ok) {
      const status =
        result.error === "approved_candidate_not_found"
          ? 404
          : result.error === "fact_already_exists"
            ? 409
            : 400
      return json({ error: { code: result.error } }, { status })
    }
    return json(result)
  }
  return json({ error: "not_found", path: apiPath }, { status: 404 })
}

async function handleOperatorOps(): Promise<Response> {
  return json({
    service: "apps/sarah",
    mount: "/sarah",
    authority: "openagents.com API",
    emailRail: "crm_operator_rail",
    agentRuntime: "owned_effect_seed",
    avatar:
      sarahAvatarRenderer() === "owned"
        ? { renderer: "owned", ...ownedRendererStatus() }
        : { renderer: "liveavatar", ...sarahAvatarStatus() },
    turnStore: sarahTurnStoreStatus(),
    answerCache: sarahAnswerCacheStatus(),
    collectiveLearning: sarahCollectiveLearningStatus(),
    blueprint: sarahBlueprintStatus(),
    ecosystem: sarahEcosystemStatus(),
    modelPath:
      sarahInferenceTransport() === "khala_gateway"
        ? `khala_gateway_live:${sarahActiveModelId()}`
        : sarahInferenceTransport() === "google_direct"
          ? `google_gemma_live:${sarahActiveModelId()}`
          : "seed_echo_not_armed",
    ui: "effect_native_dom_zero_react",
  })
}

async function handleEveTurn(request: Request): Promise<Response> {
  // SM-4: owned runtime (eve retired as dependency for the HTTP turn path).
  const body = (await request.json().catch(() => ({}))) as {
    message?: string
    threadId?: string
    prospectRef?: string
  }
  // Every conversation gets a durable prospect ref so all turns are saved
  // and attributable (owner directive 2026-07-09) — mint on first contact.
  let prospectRef = body.prospectRef ?? readSarahProspectRef(request)
  let mintedCookie = false
  if (!prospectRef) {
    prospectRef = mintSarahProspectRef()
    mintedCookie = true
  }
  const result = await runOwnedSarahTurn({
    message: body.message ?? "",
    threadId: body.threadId,
    prospectRef,
  })
  const response = json(result)
  if (mintedCookie) setSarahProspectCookie(response, prospectRef)
  return response
}

async function handleEveToolCall(request: Request): Promise<Response> {
  const body = (await request.json().catch(() => ({}))) as {
    toolName?: string
    args?: unknown
    prospectRef?: string
  }
  const result = await runOwnedSarahTurn({
    message: "",
    toolCall: {
      toolName: body.toolName ?? "",
      args: body.args ?? {},
    },
    prospectRef: body.prospectRef ?? readSarahProspectRef(request) ?? undefined,
  })
  return json(result)
}

/**
 * KHS-7 (#8606): link state for the current prospect cookie. Purely a
 * sarah_prospect_contacts read — no auth loopback on the poll path.
 */
async function handleAccountStatus(request: Request): Promise<Response> {
  const prospectRef = readSarahProspectRef(request)
  if (!prospectRef) {
    return json({ linked: false, prospect: false })
  }
  const status = await getSarahAccountLinkStatus(prospectRef)
  return json({
    linked: status.linked,
    ...(status.email ? { email: status.email } : {}),
    prospect: true,
  })
}

async function handleCustomerBlueprintCurrent(request: Request): Promise<Response> {
  const prospectRef = readSarahProspectRef(request)
  if (!prospectRef) {
    return json({
      prospect: false,
      draft: null,
      facts: [],
      contact: null,
      storeConfigured: false,
    })
  }
  const seed = await getCurrentCustomerBlueprintMapSeed(prospectRef)
  return json({
    prospect: true,
    ...(seed ?? {
      prospectRef,
      draft: null,
      facts: [],
      contact: null,
      storeConfigured: false,
    }),
  })
}

/**
 * KHS-7 (#8606): link the anonymous prospect_ref to the authenticated
 * openagents.com user. Identity comes ONLY from the first-party OpenAuth
 * session cookie verified against /api/auth/session — never from the body,
 * so an authenticated caller cannot link someone else's prospect ref.
 */
async function handleAccountLink(request: Request): Promise<Response> {
  const prospectRef = readSarahProspectRef(request)
  if (!prospectRef) {
    return json(
      {
        error: {
          code: "missing_prospect_ref",
          detail:
            "No sarah_prospect_ref cookie on this request — start a conversation on /sarah first.",
        },
      },
      { status: 400 },
    )
  }
  const user = await resolveOpenAgentsSession(request)
  if (!user) {
    return json(
      {
        error: {
          code: "not_authenticated",
          detail:
            "No verified openagents.com session — sign in via /login (same origin), then retry the link.",
        },
      },
      { status: 401 },
    )
  }
  const result = await linkSarahProspectAccount(prospectRef, user)
  return json({
    ok: true,
    linked: true,
    contactId: result.contactId,
    ...(result.email ? { email: result.email } : {}),
  })
}

async function handleUnsubscribe(request: Request): Promise<Response> {
  const url = new URL(request.url)
  const email = url.searchParams.get("email") ?? ""
  if (email) {
    const { suppressSarahEmail } = await import("./services/crm-email-rail.ts")
    await suppressSarahEmail({
      email,
      reason: "unsubscribe",
      source: "sarah.unsubscribe",
    })
  }
  // Production enforcement remains on the openagents.com CRM rail at send time;
  // local projection blocks further Sarah dry-run drafts for this address.
  return json({
    ok: true,
    email,
    status: "recorded_for_crm_suppression_rail",
    detail:
      "Opt-out recorded on Sarah's CRM rail projection; openagents.com CRM rail remains send authority.",
  })
}

function serveUi(pathname: string): Response | null {
  if (pathname === "/" || pathname === "/index.html") {
    return new Response(Bun.file(join(UI_DIR, "index.html")))
  }
  if (pathname === "/sarah.css") {
    return new Response(Bun.file(join(UI_DIR, "sarah.css")), {
      headers: { "content-type": "text/css; charset=utf-8" },
    })
  }
  return null
}

// The Effect Native surface bundle (#8598 AV-5). Production serves the
// deploy-built artifact from UI_DIR; dev builds once from source on demand.
let appBundlePromise: Promise<string | null> | null = null

async function serveAppBundle(): Promise<Response | null> {
  const built = Bun.file(join(UI_DIR, "app.js"))
  if (await built.exists()) {
    return new Response(built, {
      headers: { "content-type": "application/javascript; charset=utf-8" },
    })
  }
  appBundlePromise ??= (async () => {
    const entry = join(__dirname, "ui/main.ts")
    if (!(await Bun.file(entry).exists())) return null
    const result = await Bun.build({
      entrypoints: [entry],
      target: "browser",
      minify: false,
    })
    if (!result.success || result.outputs.length === 0) {
      console.error("[sarah] app bundle build failed", result.logs)
      return null
    }
    return await result.outputs[0]!.text()
  })()
  const code = await appBundlePromise
  if (code === null) return null
  return new Response(code, {
    headers: { "content-type": "application/javascript; charset=utf-8" },
  })
}

export async function handleSarahRequest(request: Request): Promise<Response> {
  const url = new URL(request.url)
  const path = stripPrefix(url.pathname)

  if (path === "/app.js") {
    const bundle = await serveAppBundle()
    if (bundle) return bundle
    return json({ error: { code: "app_bundle_unavailable" } }, { status: 500 })
  }
  const ui = serveUi(path)
  if (ui) return ui

  // Handoff continue: openagents.com/sarah/continue/<token> (non-API path)
  if (path.startsWith("/continue/")) {
    const token = path.slice("/continue/".length).split("/")[0] ?? ""
    return handleContinueHandoff(request, token)
  }

  if (path === "/unsubscribe" || path.startsWith("/unsubscribe?")) {
    return handleUnsubscribe(request)
  }

  // API routes (legacy private-sarah paths under /api, also under /sarah/api)
  const apiPath = path.startsWith("/api") ? path : null
  if (!apiPath) {
    return json({ error: "not_found", path }, { status: 404 })
  }

  if (apiPath === "/api/prospect/session" && request.method === "POST") {
    return handleProspectSession(request)
  }
  if (apiPath === "/api/account/status" && request.method === "GET") {
    return handleAccountStatus(request)
  }
  if (apiPath === "/api/account/link" && request.method === "POST") {
    return handleAccountLink(request)
  }
  if (apiPath === "/api/customer-blueprint/current" && request.method === "GET") {
    return handleCustomerBlueprintCurrent(request)
  }
  if (apiPath === "/api/realtime/token" && request.method === "POST") {
    return handleRealtimeToken(request)
  }
  if (apiPath === "/api/realtime/session-config" && request.method === "GET") {
    return handleSessionConfig()
  }
  if (apiPath === "/api/avatar/status" && request.method === "GET") {
    // OAV-4 (#8614): status reports the configured renderer. Flag-off default
    // is LiveAvatar with its status shape unchanged.
    const renderer = sarahAvatarRenderer()
    return json(
      renderer === "owned"
        ? { renderer, ...ownedRendererStatus() }
        : { renderer, ...sarahAvatarStatus() },
    )
  }
  if (apiPath === "/api/avatar/session" && request.method === "POST") {
    // OAV-4 (#8614) renderer seam: SARAH_AVATAR_RENDERER=owned mints on the
    // owned render service (webrtc join info instead of a LiveAvatar token).
    const prospectRef = readSarahProspectRef(request) ?? undefined
    if (sarahAvatarRenderer() === "owned") {
      reapStaleOwnedSessions()
      const owned = await mintOwnedAvatarSession({ prospectRef })
      if (!owned.ok) {
        return json(
          { error: { code: owned.error, detail: owned.detail } },
          { status: owned.status },
        )
      }
      // Owner requirement (2026-07-09): Sarah greets first, audibly — a
      // fresh session must never sit silent. Delayed fire-and-forget so the
      // browser WebRTC connect (typically <2s) lands before the audio.
      const greetDelayMs = Number(process.env.SARAH_AVATAR_GREETING_DELAY_MS ?? 2500)
      setTimeout(() => {
        void speakOwnedGreeting(owned.sessionId)
      }, greetDelayMs)
      return json(owned)
    }
    reapStaleAvatarSessions()
    const result = await mintSarahAvatarSession({ prospectRef })
    if (!result.ok) {
      return json(
        { error: { code: result.error, detail: result.detail } },
        { status: result.status },
      )
    }
    return json({ renderer: "liveavatar", ...result })
  }
  if (apiPath === "/api/avatar/stop" && request.method === "POST") {
    const body = (await request.json().catch(() => ({}))) as { sessionId?: string }
    if (!body.sessionId) return json({ error: { code: "missing_session_id" } }, { status: 400 })
    // Stop routes to whichever backend owns the session id — a browser may
    // stop a session minted before a renderer flag flip.
    if (isOwnedAvatarSession(body.sessionId)) {
      return json(await stopOwnedAvatarSession(body.sessionId))
    }
    return json(await stopSarahAvatarSession(body.sessionId))
  }
  if (apiPath === "/api/avatar/speak" && request.method === "POST") {
    // OAV-4 speaking bridge (owned renderer only): a text turn during an owned
    // avatar session runs the owned brain server-side and streams TTS PCM to
    // the render service. v1 is text-driven — mic ASR is a later lane.
    const body = (await request.json().catch(() => ({}))) as {
      sessionId?: string
      message?: string
    }
    if (!body.sessionId) return json({ error: { code: "missing_session_id" } }, { status: 400 })
    const spoken = await speakOwnedAvatarTurn({
      sessionId: body.sessionId,
      message: body.message ?? "",
    })
    if (!spoken.ok) {
      return json({ error: { code: spoken.error } }, { status: spoken.status })
    }
    return json(spoken)
  }
  if (apiPath === "/api/avatar/events" && request.method === "GET") {
    const ref = url.searchParams.get("ref")
    if (!ref) return json({ error: { code: "missing_ref" } }, { status: 400 })
    return sarahAvatarEventStream(ref)
  }
  if (apiPath === "/api/llm/chat/completions" && request.method === "POST") {
    return handleSarahChatCompletions(request)
  }
  if (apiPath === "/api/eve/turn" && request.method === "POST") {
    return handleEveTurn(request)
  }
  if (apiPath === "/api/eve/tool-call" && request.method === "POST") {
    return handleEveToolCall(request)
  }
  if (apiPath === "/api/operator/prospects" && request.method === "GET") {
    return handleOperatorProspects()
  }
  if (apiPath === "/api/operator/follow-ups") {
    return handleOperatorFollowUps(request)
  }
  if (apiPath === "/api/operator/email-drafts") {
    return handleOperatorEmailDrafts(request)
  }
  if (
    apiPath === "/api/operator/learning" ||
    apiPath.startsWith("/api/operator/learning/")
  ) {
    return handleOperatorLearning(request, apiPath)
  }
  if (
    apiPath === "/api/operator/blueprint" ||
    apiPath.startsWith("/api/operator/blueprint/")
  ) {
    return handleOperatorBlueprint(request, apiPath)
  }
  // KHS-9 (#8608): operator handoff view for customer Blueprint drafts —
  // admin-bearer-guarded (same fail-closed posture as the learning routes).
  if (
    apiPath === "/api/operator/customer-blueprints" &&
    request.method === "GET"
  ) {
    const denied = checkOperatorAdmin(request)
    if (denied) return denied
    return json(await listCustomerBlueprintsForOperator())
  }
  if (apiPath === "/api/operator/ops" && request.method === "GET") {
    return handleOperatorOps()
  }
  if (apiPath === "/api/unsubscribe") {
    return handleUnsubscribe(request)
  }

  return json({ error: "not_found", path: apiPath }, { status: 404 })
}

async function handleContinueHandoff(
  request: Request,
  handoffToken: string,
): Promise<Response> {
  if (!handoffToken || handoffToken.length < 4) {
    return json({ error: "invalid_handoff_token" }, { status: 400 })
  }
  // Prospect session continuity: mint/bind cookie; Worker handoff click is
  // recorded by the CRM rail when operator tooling posts the token.
  let prospectRef = readSarahProspectRef(request)
  if (!prospectRef) {
    prospectRef = mintSarahProspectRef()
  }
  const response = json({
    ok: true,
    handoffToken,
    prospectRef,
    threadId: threadIdForProspectRef(prospectRef),
    next: "/sarah/",
    detail:
      "Handoff accepted. Open /sarah/ to continue with this prospect cookie.",
  })
  setProspectCookieOn(response, prospectRef, 60 * 60 * 24 * 365)
  response.headers.set("location", "/sarah/")
  // Prefer HTML bounce for browsers.
  if ((request.headers.get("accept") ?? "").includes("text/html")) {
    return new Response(
      `<!doctype html><meta http-equiv="refresh" content="0;url=/sarah/"><p>Continuing to <a href="/sarah/">Sarah</a>…</p>`,
      {
        status: 302,
        headers: {
          location: "/sarah/",
          "set-cookie":
            response.headers.get("set-cookie") ??
            `${SARAH_PROSPECT_COOKIE}=${encodeURIComponent(prospectRef)}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000`,
          "content-type": "text/html; charset=utf-8",
        },
      },
    )
  }
  return response
}

const port = Number(process.env.SARAH_PORT ?? process.env.PORT ?? 8790)

if (import.meta.main) {
  Bun.serve({
    port,
    fetch: handleSarahRequest,
  })
  console.log(
    JSON.stringify({
      service: "apps/sarah",
      port,
      mount: PREFIX,
      health: `http://127.0.0.1:${port}${PREFIX}/api/operator/ops`,
    }),
  )
}
