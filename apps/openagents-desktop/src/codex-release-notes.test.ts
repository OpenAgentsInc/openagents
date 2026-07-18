import { describe, expect, test } from "vite-plus/test"

import { fetchCodexReleaseNotes } from "./codex-release-notes.ts"

describe("Codex release notes", () => {
  test("loads the exact official release tag and bounds presentation content", async () => {
    let requested = ""
    const notes = await fetchCodexReleaseNotes("0.144.1", {
      fetch: (async (input: string | URL | Request) => {
        requested = String(input)
        return new Response(JSON.stringify({
          name: "Codex 0.144.1",
          body: "A".repeat(20_000),
          html_url: "https://github.com/openai/codex/releases/tag/rust-v0.144.1",
          published_at: "2026-07-14T12:00:00Z",
        }), { status: 200 })
      }) as typeof fetch,
    })
    expect(requested.endsWith("/rust-v0.144.1")).toBe(true)
    expect(notes?.body).toHaveLength(12_000)
    expect(notes?.version).toBe("0.144.1")
  })

  test("fails closed for invalid versions and non-official release URLs", async () => {
    expect(await fetchCodexReleaseNotes("latest")).toBeNull()
    expect(await fetchCodexReleaseNotes("0.144.1", {
      fetch: (async () => new Response(JSON.stringify({
        body: "spoof",
        html_url: "https://example.com/release",
      }), { status: 200 })) as typeof fetch,
    })).toBeNull()
  })
})
