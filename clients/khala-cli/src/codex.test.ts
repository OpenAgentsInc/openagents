import { describe, expect, test } from "bun:test"

import { parseRouteSelection, selectKhalaRoute } from "./codex.js"

describe("Khala route selector", () => {
  test("parses a schema-valid spawn execution route", () => {
    expect(parseRouteSelection(JSON.stringify({
      route: "spawn_khala",
      reason: "parallel audit requested",
      intent: "execute",
      count: 5,
      objective: "audit the workspace",
      requiresWorkspace: true,
    }))).toEqual({
      route: "spawn_khala",
      reason: "parallel audit requested",
      intent: "execute",
      count: 5,
      objective: "audit the workspace",
      requiresWorkspace: true,
    })
  })

  test("parses a schema-valid spawn capability route without an objective", () => {
    expect(parseRouteSelection(JSON.stringify({
      route: "spawn_khala",
      reason: "user asked whether subprocesses exist",
      intent: "explain_capability",
    }))).toEqual({
      route: "spawn_khala",
      reason: "user asked whether subprocesses exist",
      intent: "explain_capability",
      requiresWorkspace: false,
    })
  })

  test("parses the public Artanis read-only route", () => {
    expect(parseRouteSelection(JSON.stringify({
      route: "public_artanis",
      reason: "OpenAgents operator entity",
    }))).toEqual({
      route: "public_artanis",
      reason: "OpenAgents operator entity",
    })
  })

  test("falls back to chat when the selector returns an invalid spawn count", () => {
    expect(parseRouteSelection(JSON.stringify({
      route: "spawn_khala",
      reason: "bad count",
      intent: "execute",
      count: 0,
      objective: "audit the workspace",
      requiresWorkspace: true,
    }))).toEqual({
      route: "chat",
      reason: "selector JSON schema parse failed",
    })
  })

  test("keeps the model-backed selector prompt schema-aware for spawn requests", async () => {
    let selectorPrompt = ""
    const server = Bun.serve({
      hostname: "127.0.0.1",
      port: 0,
      fetch: async request => {
        expect(new URL(request.url).pathname).toBe("/api/khala/chat")
        const body = await request.json() as { messages?: Array<{ content?: string; role?: string }> }
        selectorPrompt = body.messages?.[0]?.content ?? ""
        return sseResponse([
          'event: delta\ndata: {"text":"{\\"route\\":\\"spawn_khala\\",\\"reason\\":\\"parallel workers requested\\",\\"intent\\":\\"execute\\",\\"count\\":5,\\"objective\\":\\"audit X\\",\\"requiresWorkspace\\":true}"}',
          'event: done\ndata: {"done":true}',
          "",
        ].join("\n\n"))
      },
    })
    try {
      const selection = await selectKhalaRoute({
        baseUrl: `http://127.0.0.1:${server.port}`,
        env: {},
        history: [],
        mode: "public",
        prompt: "spin up 5 subagents to audit X",
      })
      expect(selection).toEqual({
        route: "spawn_khala",
        reason: "parallel workers requested",
        intent: "execute",
        count: 5,
        objective: "audit X",
        requiresWorkspace: true,
      })
      expect(selectorPrompt).toContain("spawn_khala")
      expect(selectorPrompt).toContain("intent")
      expect(selectorPrompt).toContain("count")
      expect(selectorPrompt).toContain("objective")
      expect(selectorPrompt).toContain("requiresWorkspace")
    } finally {
      server.stop(true)
    }
  })
})

function sseResponse(text: string): Response {
  return new Response(new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(text))
      controller.close()
    },
  }), {
    status: 200,
    headers: { "content-type": "text/event-stream" },
  })
}
