/**
 * Clip-tier oracles (epic #8610; KHS-6 seam #8605).
 * Contract: sarah.avatar_opens_with_shippable_opener_clip.v1
 *   (oracle avatar_opener_clip_tier.unit)
 *
 * Covers the license law (only MIT Hallo2 renders representable — never the
 * CodeFormer-derived *-sr.mp4 variants), the /sarah/api/clips routes, the
 * mint greeting:"client_clip" suppression of the server TTS greeting (no
 * double greet), the no-clip TTS fallback, and the /api/avatar/greet
 * clip-failure restore path.
 */

import { afterAll, afterEach, beforeAll, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { handleSarahRequest } from "../server.ts"
import {
  __resetSarahClipRotationForTest,
  getSarahClip,
  listSarahClipsForApi,
  pickSarahOpenerClip,
  resolveServableSarahClip,
  SARAH_CLIP_CATALOG,
  sarahClipUrl,
} from "./opener-clips.ts"

const savedEnv: Record<string, string | undefined> = {}
const ENV_KEYS = [
  "SARAH_CLIPS_DIR",
  "SARAH_OPENER_CLIP_ROTATION",
  "SARAH_AVATAR_RENDERER",
  "SARAH_RENDER_SERVICE_URL",
  "SARAH_RENDER_SERVICE_TOKEN",
  "SARAH_TTS_SERVICE_URL",
  "SARAH_TTS_SERVICE_TOKEN",
  "SARAH_AVATAR_GREETING_DELAY_MS",
  "SARAH_AVATAR_MAX_ACTIVE_SESSIONS",
  "LIVEAVATAR_API_KEY",
]

let clipsDir: string
let renderServer: ReturnType<typeof Bun.serve>
let ttsServer: ReturnType<typeof Bun.serve>
let ttsRequests: Array<Record<string, unknown>> = []
let sessionCounter = 0

/** Fake MP4 payload — route serving is byte/headers behavior, not codec. */
const FAKE_MP4 = new Uint8Array(4096).map((_, i) => i % 251)

beforeAll(() => {
  for (const key of ENV_KEYS) savedEnv[key] = process.env[key]
  clipsDir = mkdtempSync(join(tmpdir(), "sarah-clips-test-"))
  // Only two of the five catalog clips "exist" so availability is exercised.
  writeFileSync(join(clipsDir, "opener-01-hello-hallo2.mp4"), FAKE_MP4)
  writeFileSync(join(clipsDir, "opener-05-show-you-hallo2.mp4"), FAKE_MP4)

  renderServer = Bun.serve({
    port: 0,
    fetch: async (request) => {
      const url = new URL(request.url)
      if (request.method === "POST" && url.pathname === "/sessions") {
        sessionCounter += 1
        return Response.json({
          session_id: `clip-sess-${sessionCounter}`,
          webrtc: { offer_url: `http://127.0.0.1:1/whep/clip-sess-${sessionCounter}` },
        })
      }
      if (request.method === "POST" && url.pathname.endsWith("/control")) {
        return Response.json({ ok: true })
      }
      if (request.method === "DELETE") return new Response(null, { status: 204 })
      return new Response("not found", { status: 404 })
    },
  })
  ttsServer = Bun.serve({
    port: 0,
    fetch: async (request) => {
      ttsRequests.push((await request.json().catch(() => ({}))) as Record<string, unknown>)
      return new Response(new Uint8Array(9600), {
        headers: { "content-type": "application/octet-stream" },
      })
    },
  })
})

afterAll(() => {
  renderServer.stop(true)
  ttsServer.stop(true)
  rmSync(clipsDir, { recursive: true, force: true })
  for (const key of ENV_KEYS) {
    if (savedEnv[key] === undefined) delete process.env[key]
    else process.env[key] = savedEnv[key]
  }
})

afterEach(() => {
  for (const key of ENV_KEYS) delete process.env[key]
  __resetSarahClipRotationForTest()
  ttsRequests = []
})

function armClips() {
  process.env.SARAH_CLIPS_DIR = clipsDir
}

function armOwnedRenderer() {
  process.env.SARAH_AVATAR_RENDERER = "owned"
  process.env.SARAH_RENDER_SERVICE_URL = `http://127.0.0.1:${renderServer.port}`
  process.env.SARAH_RENDER_SERVICE_TOKEN = "render-test-token"
  process.env.SARAH_TTS_SERVICE_URL = `http://127.0.0.1:${ttsServer.port}`
  process.env.SARAH_TTS_SERVICE_TOKEN = "tts-test-token"
  process.env.SARAH_AVATAR_GREETING_DELAY_MS = "100"
}

async function stopSession(sessionId: string) {
  await handleSarahRequest(
    new Request("http://localhost/sarah/api/avatar/stop", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId }),
    }),
  )
}

describe("license law: the catalog is the shippable tier only", () => {
  test("every catalog file is a raw MIT Hallo2 render — SR variants unrepresentable", () => {
    expect(SARAH_CLIP_CATALOG.length).toBe(5)
    for (const clip of SARAH_CLIP_CATALOG) {
      expect(clip.tier).toBe("hallo2_512_mit")
      expect(clip.file.endsWith("-hallo2.mp4")).toBe(true)
      expect(clip.file.includes("-sr")).toBe(false)
      expect(clip.name.includes("/")).toBe(false)
      expect(clip.script.length).toBeGreaterThan(10)
      expect(clip.sttTranscript.length).toBeGreaterThan(10)
    }
  })

  test("lookups resolve only against the closed catalog", () => {
    expect(getSarahClip("opener-01-hello")).not.toBeNull()
    expect(getSarahClip("opener-01-hello-hallo2-sr")).toBeNull()
    expect(getSarahClip("../../etc/passwd")).toBeNull()
    expect(getSarahClip("")).toBeNull()
  })
})

describe("GET /sarah/api/clips (typed manifest)", () => {
  test("lists the catalog with availability and public-safe urls", async () => {
    armClips()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/clips"),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      library: string
      clips: Array<Record<string, unknown>>
    }
    expect(body.library).toBe("sarah-avatar/openers-v2")
    expect(body.clips.length).toBe(5)
    const byName = new Map(body.clips.map((c) => [c.name, c]))
    expect(byName.get("opener-01-hello")?.available).toBe(true)
    expect(byName.get("opener-05-show-you")?.available).toBe(true)
    expect(byName.get("opener-02-welcome-back")?.available).toBe(false)
    for (const clip of body.clips) {
      expect(String(clip.url)).toStartWith("/sarah/api/clips/")
      expect(String(clip.url).includes("-sr")).toBe(false)
      expect(typeof clip.script).toBe("string")
      expect(typeof clip.transcript).toBe("string")
    }
  })
})

describe("GET /sarah/api/clips/:name (mp4 serving)", () => {
  test("serves video/mp4 with immutable caching and accept-ranges", async () => {
    armClips()
    const response = await handleSarahRequest(
      new Request(`http://localhost${sarahClipUrl("opener-01-hello")}`),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toBe("video/mp4")
    expect(response.headers.get("cache-control")).toBe(
      "public, max-age=31536000, immutable",
    )
    expect(response.headers.get("accept-ranges")).toBe("bytes")
    const bytes = new Uint8Array(await response.arrayBuffer())
    expect(bytes).toEqual(FAKE_MP4)
  })

  test("serves a 206 partial for range requests (Safari playback contract)", async () => {
    armClips()
    const response = await handleSarahRequest(
      new Request(`http://localhost${sarahClipUrl("opener-01-hello")}`, {
        headers: { range: "bytes=0-1" },
      }),
    )
    expect(response.status).toBe(206)
    expect(response.headers.get("content-range")).toBe(`bytes 0-1/${FAKE_MP4.length}`)
    expect(response.headers.get("content-length")).toBe("2")
    const bytes = new Uint8Array(await response.arrayBuffer())
    expect(bytes).toEqual(FAKE_MP4.slice(0, 2))

    const tail = await handleSarahRequest(
      new Request(`http://localhost${sarahClipUrl("opener-01-hello")}`, {
        headers: { range: "bytes=-16" },
      }),
    )
    expect(tail.status).toBe(206)
    expect(new Uint8Array(await tail.arrayBuffer())).toEqual(
      FAKE_MP4.slice(FAKE_MP4.length - 16),
    )

    const invalid = await handleSarahRequest(
      new Request(`http://localhost${sarahClipUrl("opener-01-hello")}`, {
        headers: { range: `bytes=${FAKE_MP4.length}-` },
      }),
    )
    expect(invalid.status).toBe(416)
  })

  test("unknown, SR, and traversal names are 404 — never filesystem reads", async () => {
    armClips()
    for (const name of [
      "opener-99-nope",
      "opener-01-hello-hallo2-sr",
      "..%2F..%2Fserver.ts",
    ]) {
      const response = await handleSarahRequest(
        new Request(`http://localhost/sarah/api/clips/${name}`),
      )
      expect(response.status).toBe(404)
      const body = (await response.json()) as { error: { code: string } }
      expect(body.error.code).toBe("clip_not_found")
    }
  })

  test("a catalog clip whose file is missing is clip_unavailable", async () => {
    armClips()
    const response = await handleSarahRequest(
      new Request(`http://localhost${sarahClipUrl("opener-03-good-question")}`),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: { code: string } }
    expect(body.error.code).toBe("clip_unavailable")
  })
})

describe("opener pick + clipRef resolution", () => {
  test("default rotation picks opener-01 when available; null without files", async () => {
    armClips()
    const pick = await pickSarahOpenerClip()
    expect(pick?.name).toBe("opener-01-hello")
    expect(pick?.url).toBe("/sarah/api/clips/opener-01-hello")
    expect(pick?.script).toBe("Hello! I'm Sarah. What's on your mind today?")

    process.env.SARAH_CLIPS_DIR = join(clipsDir, "does-not-exist")
    expect(await pickSarahOpenerClip()).toBeNull()
  })

  test("rotation env cycles across available clips and skips missing ones", async () => {
    armClips()
    process.env.SARAH_OPENER_CLIP_ROTATION =
      "opener-01-hello,opener-02-welcome-back,opener-05-show-you"
    const first = await pickSarahOpenerClip()
    const second = await pickSarahOpenerClip()
    const third = await pickSarahOpenerClip()
    // opener-02's file is absent — the rotation never yields it.
    expect(first?.name).toBe("opener-01-hello")
    expect(second?.name).toBe("opener-05-show-you")
    expect(third?.name).toBe("opener-01-hello")
  })

  test("resolveServableSarahClip accepts clip:<name> and bare names; missing degrades to null", async () => {
    armClips()
    expect(await resolveServableSarahClip("clip:opener-05-show-you")).toEqual({
      name: "opener-05-show-you",
      url: "/sarah/api/clips/opener-05-show-you",
    })
    expect(await resolveServableSarahClip("opener-01-hello")).not.toBeNull()
    expect(await resolveServableSarahClip("clip:opener-03-good-question")).toBeNull()
    expect(await resolveServableSarahClip("clip:not-in-catalog")).toBeNull()
  })
})

describe("mint greeting:\"client_clip\" (no double greeting)", () => {
  const collectSse = async (
    conversationRef: string,
    untilText: string,
    maxReads = 20,
  ): Promise<string> => {
    const sse = await handleSarahRequest(
      new Request(
        `http://localhost/sarah/api/avatar/events?ref=${encodeURIComponent(conversationRef)}`,
      ),
    )
    const reader = sse.body!.getReader()
    const decoder = new TextDecoder()
    let text = ""
    for (let i = 0; i < maxReads && !text.includes(untilText); i++) {
      const { value, done } = await reader.read()
      if (done) break
      text += decoder.decode(value)
    }
    await reader.cancel().catch(() => {})
    return text
  }

  test("clip available: mint returns openerClip, publishes the transcript line, and sends NO TTS greeting", async () => {
    armClips()
    armOwnedRenderer()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ greeting: "client_clip" }),
      }),
    )
    expect(response.status).toBe(200)
    const mint = (await response.json()) as {
      sessionId: string
      conversationRef: string
      openerClip?: { name: string; url: string; script: string }
    }
    expect(mint.openerClip?.name).toBe("opener-01-hello")
    expect(mint.openerClip?.url).toBe("/sarah/api/clips/opener-01-hello")
    expect(mint.openerClip?.script).toContain("I'm Sarah")

    const sseText = await collectSse(mint.conversationRef, "I'm Sarah")
    expect(sseText).toContain("I'm Sarah")
    expect(sseText).toContain('"type":"transcript"')
    // The suppression law: no TTS synthesis for the greeting in clip mode.
    expect(ttsRequests.length).toBe(0)
    await stopSession(mint.sessionId)
  })

  test("no clip available: mint falls back to the TTS greeting (never silent)", async () => {
    process.env.SARAH_CLIPS_DIR = join(clipsDir, "does-not-exist")
    armOwnedRenderer()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ greeting: "client_clip" }),
      }),
    )
    expect(response.status).toBe(200)
    const mint = (await response.json()) as {
      sessionId: string
      conversationRef: string
      openerClip?: unknown
    }
    expect(mint.openerClip).toBeUndefined()
    const sseText = await collectSse(mint.conversationRef, "I'm Sarah")
    expect(sseText).toContain("I'm Sarah")
    // TTS greeting fired (fire-and-forget; give it a beat).
    await new Promise((resolve) => setTimeout(resolve, 150))
    expect(ttsRequests.length).toBeGreaterThan(0)
    await stopSession(mint.sessionId)
  })

  test("a mint without the option keeps today's TTS greeting behavior", async () => {
    armClips()
    armOwnedRenderer()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", {
        method: "POST",
      }),
    )
    expect(response.status).toBe(200)
    const mint = (await response.json()) as {
      sessionId: string
      openerClip?: unknown
    }
    expect(mint.openerClip).toBeUndefined()
    await new Promise((resolve) => setTimeout(resolve, 200))
    expect(ttsRequests.length).toBeGreaterThan(0)
    await stopSession(mint.sessionId)
  })
})

describe("POST /sarah/api/avatar/greet (clip-failure fallback)", () => {
  test("restores the TTS greeting for a live owned session; 404 otherwise", async () => {
    armClips()
    armOwnedRenderer()
    const response = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ greeting: "client_clip" }),
      }),
    )
    const mint = (await response.json()) as { sessionId: string }
    ttsRequests = []

    const greet = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/greet", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: mint.sessionId }),
      }),
    )
    expect(greet.status).toBe(200)
    await new Promise((resolve) => setTimeout(resolve, 150))
    expect(ttsRequests.length).toBeGreaterThan(0)
    await stopSession(mint.sessionId)

    const unknown = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/greet", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: "nope" }),
      }),
    )
    expect(unknown.status).toBe(404)

    const missing = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/greet", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "{}",
      }),
    )
    expect(missing.status).toBe(400)
  })
})
