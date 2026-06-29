import { describe, expect, test } from "bun:test"

import config from "../electrobun.config.js"
import { parseMessageSegments } from "../src/ui/transcript-render"

describe("khala code desktop app shell", () => {
  test("registers the Khala Code desktop view", () => {
    expect(config.app).toMatchObject({
      identifier: "com.openagents.khala.code.desktop",
      name: "Khala Code",
      version: "0.1.0",
    })
    expect(config.build.bun.entrypoint).toBe("src/bun/index.ts")
    expect(config.build.views["khala-code-desktop"]).toMatchObject({
      entrypoint: "resources/ui/main.js",
    })
    expect(config.build.copy).toMatchObject({
      "resources/ui/main.css": "views/khala-code-desktop/main.css",
      "src/ui/index.html": "views/khala-code-desktop/index.html",
    })
  })

  test("renders the chat-only surface", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()

    expect(html).toContain('class="khala-code-shell antialiased"')
    expect(html).toContain('id="message-list"')
    expect(html).toContain('id="composer-form"')
    expect(html).toContain('id="composer-input"')
    expect(html).toContain('id="send-button"')
    expect(html).not.toContain("Pylons")
    expect(html).not.toContain("Fleet")
  })

  test("splits code and diff fixtures for the initial transcript renderer", () => {
    const segments = parseMessageSegments(
      "Patch:\n\n```diff\n@@ -1 +1 @@\n-a\n+b\n```\n\nCode:\n\n```ts\nexport const ok = true\n```",
    )

    expect(segments.map(segment => segment.kind)).toEqual([
      "prose",
      "diff",
      "prose",
      "code",
    ])
  })
})
