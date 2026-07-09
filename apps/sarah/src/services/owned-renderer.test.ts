/**
 * OAV-4 (#8614) renderer-seam tests: the owned render-service contract client
 * exercised against a FAKE render service + FAKE TTS service (in-test Bun
 * servers), plus renderer selection through the real route handler.
 *
 * Contract under test (OAV-2 builds the real service in hydralisk):
 *   POST   /sessions                  -> { session_id, webrtc: { offer_url } }
 *   POST   /sessions/{id}/control     speak (b64 PCM 24k, shared event_id) /
 *                                     speak_end / interrupt / listening states
 *   DELETE /sessions/{id}
 */

import { afterAll, beforeAll, describe, expect, test } from "bun:test"

import { handleSarahRequest } from "../server.ts"
import {
  chunkPcmBase64,
  sarahAvatarRenderer,
  speakOwnedAvatarTurn,
} from "./owned-renderer.ts"

type RecordedRequest = {
  method: string
  path: string
  auth: string | null
  body: Record<string, unknown> | null
}

const renderRequests: RecordedRequest[] = []
const ttsRequests: Array<Record<string, unknown>> = []

let renderServer: ReturnType<typeof Bun.serve>
let ttsServer: ReturnType<typeof Bun.serve>
let sessionCounter = 0

/** 1.7 s of s16le 24 kHz mono PCM (81,600 bytes) → 600 ms + 1 s + 100 ms chunks. */
const TTS_PCM_BYTES = 81_600
const ttsPcm = new Uint8Array(TTS_PCM_BYTES).map((_, i) => i % 251)

const savedEnv: Record<string, string | undefined> = {}
const ENV_KEYS = [
  "SARAH_AVATAR_RENDERER",
  "SARAH_RENDER_SERVICE_URL",
  "SARAH_RENDER_SERVICE_TOKEN",
  "SARAH_TTS_SERVICE_URL",
  "SARAH_TTS_SERVICE_TOKEN",
  "SARAH_AVATAR_MAX_ACTIVE_SESSIONS",
  "SARAH_AVATAR_DAILY_SESSION_CAP",
  "LIVEAVATAR_API_KEY",
]

function armOwnedRenderer() {
  process.env.SARAH_AVATAR_RENDERER = "owned"
  process.env.SARAH_RENDER_SERVICE_URL = `http://127.0.0.1:${renderServer.port}`
  process.env.SARAH_RENDER_SERVICE_TOKEN = "render-test-token"
}

function armOwnedTts() {
  process.env.SARAH_TTS_SERVICE_URL = `http://127.0.0.1:${ttsServer.port}`
  process.env.SARAH_TTS_SERVICE_TOKEN = "tts-test-token"
}

function disarmAll() {
  for (const key of ENV_KEYS) delete process.env[key]
}

beforeAll(() => {
  for (const key of ENV_KEYS) savedEnv[key] = process.env[key]

  renderServer = Bun.serve({
    port: 0,
    fetch: async (request) => {
      const url = new URL(request.url)
      const body =
        request.method === "POST"
          ? ((await request.json().catch(() => null)) as Record<string, unknown> | null)
          : null
      renderRequests.push({
        method: request.method,
        path: url.pathname,
        auth: request.headers.get("authorization"),
        body,
      })
      if (request.headers.get("authorization") !== "Bearer render-test-token") {
        return new Response("unauthorized", { status: 401 })
      }
      if (request.method === "POST" && url.pathname === "/sessions") {
        sessionCounter += 1
        return Response.json({
          session_id: `owned-sess-${sessionCounter}`,
          webrtc: { offer_url: `http://127.0.0.1:1/whep/owned-sess-${sessionCounter}` },
        })
      }
      if (request.method === "POST" && url.pathname.endsWith("/control")) {
        return Response.json({ ok: true })
      }
      if (request.method === "DELETE" && url.pathname.startsWith("/sessions/")) {
        return new Response(null, { status: 204 })
      }
      return new Response("not found", { status: 404 })
    },
  })

  ttsServer = Bun.serve({
    port: 0,
    fetch: async (request) => {
      const body = (await request.json().catch(() => ({}))) as Record<string, unknown>
      ttsRequests.push(body)
      if (request.headers.get("authorization") !== "Bearer tts-test-token") {
        return new Response("unauthorized", { status: 401 })
      }
      return new Response(ttsPcm, {
        headers: { "content-type": "application/octet-stream" },
      })
    },
  })
})

afterAll(() => {
  renderServer.stop(true)
  ttsServer.stop(true)
  for (const key of ENV_KEYS) {
    if (savedEnv[key] === undefined) delete process.env[key]
    else process.env[key] = savedEnv[key]
  }
})

async function mintOwnedViaRoute(): Promise<{
  sessionId: string
  conversationRef: string
  webrtc: { offer_url?: string }
  renderer: string
}> {
  const response = await handleSarahRequest(
    new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
  )
  expect(response.status).toBe(200)
  return (await response.json()) as never
}

async function stopViaRoute(sessionId: string) {
  return handleSarahRequest(
    new Request("http://localhost/sarah/api/avatar/stop", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId }),
    }),
  )
}

describe("OAV-4 PCM speak chunking", () => {
  test("first chunk is ~600ms, then 1s chunks (24k s16le)", () => {
    const chunks = chunkPcmBase64(ttsPcm)
    const sizes = chunks.map((chunk) => Buffer.from(chunk, "base64").length)
    // 81,600 bytes = 28,800 (600ms) + 48,000 (1s) + 4,800 (remainder)
    expect(sizes).toEqual([28_800, 48_000, 4_800])
    // Chunks reassemble to the exact original PCM.
    const joined = Buffer.concat(chunks.map((chunk) => Buffer.from(chunk, "base64")))
    expect(new Uint8Array(joined)).toEqual(ttsPcm)
  })

  test("short utterance yields a single chunk; empty yields none", () => {
    expect(chunkPcmBase64(new Uint8Array(0))).toEqual([])
    const short = chunkPcmBase64(new Uint8Array(1_000))
    expect(short.length).toBe(1)
    expect(Buffer.from(short[0]!, "base64").length).toBe(1_000)
  })
})

describe("OAV-4 renderer selection", () => {
  test("flag-off default is liveavatar and behavior is unchanged", async () => {
    disarmAll()
    expect(sarahAvatarRenderer()).toBe("liveavatar")

    const status = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/status"),
    )
    const statusBody = await status.json()
    expect(statusBody.renderer).toBe("liveavatar")
    expect(statusBody.armed).toBe(false)
    expect(typeof statusBody.sandbox).toBe("boolean")

    // Unarmed LiveAvatar mint refusal — same code/status as before the seam.
    const mint = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
    )
    expect(mint.status).toBe(503)
    const mintBody = await mint.json()
    expect(mintBody.error.code).toBe("avatar_not_armed")
  })

  test("SARAH_AVATAR_RENDERER=owned without service config refuses 503", async () => {
    disarmAll()
    process.env.SARAH_AVATAR_RENDERER = "owned"
    const mint = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
    )
    expect(mint.status).toBe(503)
    const body = await mint.json()
    expect(body.error.code).toBe("avatar_not_armed")
    disarmAll()
  })

  test("owned status reports renderer + arm state", async () => {
    disarmAll()
    armOwnedRenderer()
    const status = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/status"),
    )
    const body = await status.json()
    expect(body.renderer).toBe("owned")
    expect(body.armed).toBe(true)
    expect(body.ttsArmed).toBe(false)
    expect(body.speechScope).toBe("text_driven_v1_no_mic_asr")
    disarmAll()
  })

  test("ops endpoint carries the configured renderer", async () => {
    disarmAll()
    armOwnedRenderer()
    const ops = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/ops"),
    )
    const body = await ops.json()
    expect(body.avatar.renderer).toBe("owned")
    disarmAll()
  })
})

describe("OAV-4 owned render-service contract client", () => {
  test("mint returns webrtc join info instead of a LiveAvatar token; stop routes to the owned backend (mint/stop symmetry)", async () => {
    disarmAll()
    armOwnedRenderer()
    renderRequests.length = 0

    const mint = await mintOwnedViaRoute()
    expect(mint.renderer).toBe("owned")
    expect(mint.sessionId).toStartWith("owned-sess-")
    expect(mint.conversationRef).toStartWith("visitor:")
    expect(mint.webrtc.offer_url).toContain("/whep/")
    expect((mint as Record<string, unknown>).sessionToken).toBeUndefined()

    const sessionPost = renderRequests.find(
      (r) => r.method === "POST" && r.path === "/sessions",
    )
    expect(sessionPost).toBeDefined()
    expect(sessionPost!.auth).toBe("Bearer render-test-token")
    expect(sessionPost!.body?.conversation_ref).toBe(mint.conversationRef)

    const statusLive = await (
      await handleSarahRequest(new Request("http://localhost/sarah/api/avatar/status"))
    ).json()
    expect(statusLive.activeSessions).toBe(1)

    const stop = await stopViaRoute(mint.sessionId)
    expect(stop.status).toBe(200)
    const stopBody = await stop.json()
    expect(stopBody.ok).toBe(true)
    expect(stopBody.minutes).toBeGreaterThanOrEqual(1)
    expect(
      renderRequests.some(
        (r) => r.method === "DELETE" && r.path === `/sessions/${mint.sessionId}`,
      ),
    ).toBe(true)

    const statusAfter = await (
      await handleSarahRequest(new Request("http://localhost/sarah/api/avatar/status"))
    ).json()
    expect(statusAfter.activeSessions).toBe(0)
    disarmAll()
  })

  test("active-session cap holds on the owned lane", async () => {
    disarmAll()
    armOwnedRenderer()
    process.env.SARAH_AVATAR_MAX_ACTIVE_SESSIONS = "1"

    const first = await mintOwnedViaRoute()
    const second = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
    )
    expect(second.status).toBe(429)
    const body = await second.json()
    expect(body.error.code).toBe("avatar_session_cap_exceeded")

    await stopViaRoute(first.sessionId)
    disarmAll()
  })

  test("speak bridge: brain reply → TTS PCM → chunked speak/speak_end with one event_id; SSE bus carries the transcript", async () => {
    disarmAll()
    armOwnedRenderer()
    armOwnedTts()
    const mint = await mintOwnedViaRoute()
    renderRequests.length = 0
    ttsRequests.length = 0

    // Subscribe to the SSE bus like the surface does.
    const sse = await handleSarahRequest(
      new Request(
        `http://localhost/sarah/api/avatar/events?ref=${encodeURIComponent(mint.conversationRef)}`,
      ),
    )
    const reader = sse.body!.getReader()
    await reader.read() // connected comment

    const speak = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/speak", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: mint.sessionId, message: "hello Sarah" }),
      }),
    )
    expect(speak.status).toBe(200)
    const spoken = await speak.json()
    expect(spoken.ok).toBe(true)
    expect(spoken.spoken).toBe(true)
    expect(spoken.reply).toBeTruthy()

    // The TTS seam received the brain's reply text with the PCM contract.
    expect(ttsRequests.length).toBe(1)
    expect(ttsRequests[0]!.text).toBe(spoken.reply)
    expect(ttsRequests[0]!.format).toBe("pcm_s16le")
    expect(ttsRequests[0]!.sample_rate_hz).toBe(24_000)

    // Control stream: speak chunks share one event_id, then speak_end.
    const controls = renderRequests.filter((r) =>
      r.path === `/sessions/${mint.sessionId}/control`,
    )
    const speaks = controls.filter((r) => r.body?.type === "speak")
    const ends = controls.filter((r) => r.body?.type === "speak_end")
    expect(speaks.length).toBe(3) // 81,600 bytes → 600ms + 1s + remainder
    expect(ends.length).toBe(1)
    const eventIds = new Set(
      [...speaks, ...ends].map((r) => String(r.body?.event_id)),
    )
    expect(eventIds.size).toBe(1)
    const firstChunk = Buffer.from(String(speaks[0]!.body?.audio_b64), "base64")
    expect(firstChunk.length).toBe(28_800) // ~600ms of 24k s16le
    expect(controls[controls.length - 1]!.body?.type).toBe("speak_end")

    // SSE transcript frames: the user turn and the assistant reply.
    let sseText = ""
    const decoder = new TextDecoder()
    for (let i = 0; i < 10 && !(sseText.includes("hello Sarah") && sseText.includes("assistant")); i++) {
      const { value, done } = await reader.read()
      if (done) break
      sseText += decoder.decode(value)
    }
    expect(sseText).toContain('"role":"user"')
    expect(sseText).toContain("hello Sarah")
    expect(sseText).toContain('"role":"assistant"')
    await reader.cancel()

    await stopViaRoute(mint.sessionId)
    disarmAll()
  })

  test("speak degrades honestly to text-only when TTS is not armed", async () => {
    disarmAll()
    armOwnedRenderer()
    const mint = await mintOwnedViaRoute()

    const result = await speakOwnedAvatarTurn({
      sessionId: mint.sessionId,
      message: "tell me about OpenAgents",
    })
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.spoken).toBe(false)
      expect(result.speechError).toBe("tts_not_armed")
      expect(result.reply).toBeTruthy()
    }

    await stopViaRoute(mint.sessionId)
    disarmAll()
  })

  test("speak on an unknown/non-owned session is a 404", async () => {
    disarmAll()
    armOwnedRenderer()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/speak", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: "not-a-session", message: "hi" }),
      }),
    )
    expect(response.status).toBe(404)
    const body = await response.json()
    expect(body.error.code).toBe("owned_session_not_found")
    disarmAll()
  })
})

describe("toSpeakableText (SQ-4 #8621)", () => {
  const { toSpeakableText } = require("./owned-renderer.ts")
  test("strips markdown emphasis, bullets, links, and code", () => {
    const md = "**Bold claim.** Here's a [link](https://x.com) and `code`.\n- bullet one\n- bullet two\n\n```js\nconsole.log(1)\n```\n## Header\n1. numbered"
    const out = toSpeakableText(md)
    expect(out).toBe("Bold claim. Here's a link and code. bullet one bullet two Header numbered")
  })
  test("keeps plain prose untouched", () => {
    expect(toSpeakableText("Hello! I'm Sarah. What's on your mind today?")).toBe(
      "Hello! I'm Sarah. What's on your mind today?",
    )
  })
})

describe("splitSpeakableSentences (SQ-4 #8621)", () => {
  const { splitSpeakableSentences } = require("./owned-renderer.ts")
  test("groups short sentences to the minimum size", () => {
    const groups = splitSpeakableSentences("Got it. And how much time does that eat up each week?", 40)
    expect(groups).toEqual(["Got it. And how much time does that eat up each week?"])
  })
  test("splits a long reply into multiple streamable groups", () => {
    const reply =
      "We help businesses put agents to work on real tasks every single day. " +
      "You pay for completed work, not for seats. " +
      "So tell me, what's eating the most hours in your business right now?"
    const groups = splitSpeakableSentences(reply, 40)
    expect(groups.length).toBeGreaterThan(1)
    expect(groups.join(" ")).toBe(reply)
  })
  test("merges a trailing fragment into the previous group", () => {
    const groups = splitSpeakableSentences("This is a fairly long first sentence for the group. Okay?", 40)
    expect(groups.length).toBe(1)
    expect(groups[0].endsWith("Okay?")).toBe(true)
  })
})
