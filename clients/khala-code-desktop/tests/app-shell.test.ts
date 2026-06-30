import { describe, expect, test } from "bun:test"

import config from "../electrobun.config.js"
import { khalaCodeDesktopApplicationMenu } from "../src/bun/application-menu"
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
    expect(html).toContain("autofocus")
    expect(html).toContain('id="send-button"')
    expect(html).not.toContain("Pylons")
    expect(html).not.toContain("Fleet")
  })

  test("keeps composer focus styling on the frame instead of the textarea", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".composer-shell:focus-within")
    expect(css).not.toContain("#composer-input:focus-visible")
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

  test("installs native edit menu accelerators for WebKit text editing", async () => {
    const edit = khalaCodeDesktopApplicationMenu.find(
      item => "label" in item && item.label === "Edit",
    ) as { submenu?: Array<{ role?: string; accelerator?: string }> } | undefined
    expect(edit).toBeDefined()
    const byRole = new Map(
      (edit?.submenu ?? [])
        .filter(item => typeof item.role === "string")
        .map(item => [item.role, item]),
    )

    expect(byRole.get("copy")?.accelerator).toBe("CommandOrControl+C")
    expect(byRole.get("paste")?.accelerator).toBe("CommandOrControl+V")
    expect(byRole.get("cut")?.accelerator).toBe("CommandOrControl+X")
    expect(byRole.get("selectAll")?.accelerator).toBe("CommandOrControl+A")
    expect(byRole.get("undo")?.accelerator).toBe("CommandOrControl+Z")

    const bunEntry = await Bun.file(new URL("../src/bun/index.ts", import.meta.url)).text()
    expect(bunEntry).toContain(
      "ApplicationMenu.setApplicationMenu(khalaCodeDesktopApplicationMenu)",
    )
  })
})
