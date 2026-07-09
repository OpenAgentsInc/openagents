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

const __dirname = fileURLToPath(new URL(".", import.meta.url))
const UI_DIR = join(__dirname, "ui")
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
  })
  return json(result, { status: result.ok ? 200 : 400 })
}

async function handleOperatorOps(): Promise<Response> {
  return json({
    service: "apps/sarah",
    mount: "/sarah",
    authority: "openagents.com Worker APIs",
    emailRail: "crm_operator_rail",
    agentRuntime: "owned_effect_seed",
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
  const result = await runOwnedSarahTurn({
    message: body.message ?? "",
    threadId: body.threadId,
    prospectRef: body.prospectRef ?? readSarahProspectRef(request) ?? undefined,
  })
  return json(result)
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

async function handleUnsubscribe(request: Request): Promise<Response> {
  const url = new URL(request.url)
  const email = url.searchParams.get("email") ?? ""
  // Suppression is Worker-owned; record activity intent only.
  return json({
    ok: true,
    email,
    status: "recorded_for_crm_suppression_rail",
    detail:
      "Suppression is enforced by the openagents.com CRM rail on send, not a Sarah-local list.",
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
  if (pathname === "/sarah.js") {
    return new Response(Bun.file(join(UI_DIR, "sarah.js")), {
      headers: { "content-type": "application/javascript; charset=utf-8" },
    })
  }
  return null
}

export async function handleSarahRequest(request: Request): Promise<Response> {
  const url = new URL(request.url)
  const path = stripPrefix(url.pathname)

  const ui = serveUi(path)
  if (ui) return ui

  // API routes (legacy private-sarah paths under /api, also under /sarah/api)
  const apiPath = path.startsWith("/api") ? path : null
  if (!apiPath) {
    return json({ error: "not_found", path }, { status: 404 })
  }

  if (apiPath === "/api/prospect/session" && request.method === "POST") {
    return handleProspectSession(request)
  }
  if (apiPath === "/api/realtime/token" && request.method === "POST") {
    return handleRealtimeToken(request)
  }
  if (apiPath === "/api/realtime/session-config" && request.method === "GET") {
    return handleSessionConfig()
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
  if (apiPath === "/api/operator/ops" && request.method === "GET") {
    return handleOperatorOps()
  }
  if (apiPath === "/api/unsubscribe" || path === "/unsubscribe") {
    return handleUnsubscribe(request)
  }

  return json({ error: "not_found", path: apiPath }, { status: 404 })
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
