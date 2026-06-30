import { describe, expect, test } from "bun:test"
import {
  parseMarkdownBlocks,
  parseMarkdownInline,
} from "@openagentsinc/ui/ai-elements/markdown"

import config from "../electrobun.config.js"
import { khalaCodeDesktopApplicationMenu } from "../src/bun/application-menu"
import {
  parseMessageSegments,
  parseToolTranscript,
} from "../src/ui/transcript-render"

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
    expect(html).toContain("oa-ai-command-composer")
    expect(html).toContain("data-oa-command-composer")
    expect(html).toContain('id="composer-rail"')
    expect(html).toContain('id="composer-hud"')
    expect(html).toContain('id="composer-input"')
    expect(html).toContain("data-oa-command-composer-native-editing")
    expect(html).toContain("autofocus")
    expect(html).toContain('id="send-button"')
    expect(html).not.toContain("Pylons")
    expect(html).not.toContain("Fleet")
  })

  test("does not seed dummy code or diff messages on first load", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(main).not.toContain("assistant-code")
    expect(main).not.toContain("assistant-diff")
    expect(main).not.toContain("```diff")
    expect(main).not.toContain("QueueItem")
  })

  test("seeds first-load copy in Khala's plural voice", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(main).toContain("Point us at a repo")
    expect(main).toContain("we will keep the patch")
    expect(main).not.toContain("Point me at a repo")
    expect(main).not.toContain("I will keep")
  })

  test("keeps composer focus styling on the frame instead of the textarea", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".khala-code-composer:focus-within .oa-ai-command-composer-frame")
    expect(css).not.toContain("#composer-input:focus-visible")
  })

  test("keeps the composer footer controls in a clean inline strip", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html.indexOf('id="resize-button"')).toBeLessThan(html.indexOf('id="send-button"'))
    expect(css).toContain("grid-template-columns: minmax(0, 1fr) minmax(0, auto) 36px 36px")
    expect(css).toContain(".khala-code-composer .oa-ai-command-composer-submit-label")
    expect(css).toContain("position: static")
    expect(main).not.toContain('composerExpanded ? "expanded" : "compact"')
  })

  test("keeps the composer input available while a turn is pending", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("sendButton.disabled = !pendingTurn && !canSubmitComposer()")
    expect(main).toContain('sendButton.type = pendingTurn ? "button" : "submit"')
    expect(main).toContain("stopActiveTurn")
    expect(main).not.toContain("composerInput.disabled = pendingTurn")
    expect(main).toContain("requestAnimationFrame(focusComposerInput)")
    expect(css).not.toContain("#composer-input:disabled")
    expect(css).not.toContain("cursor: wait")
  })

  test("keeps preview HTTP RPC off Electrobun native internal ports", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(main).toContain("KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT")
    expect(main).toContain("isKhalaPreviewWindow")
    expect(main).toContain("const rpc = isKhalaPreviewWindow ? previewRpc() : nativeRpc")
    expect(main).not.toContain("__electrobunRpcSocketPort")
  })

  test("wires shared composer state for attachments, large paste, and HUD projection", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const pkg = await Bun.file(new URL("../package.json", import.meta.url)).json() as {
      dependencies: Record<string, string>
    }

    expect(main).toContain("@openagentsinc/composer-state")
    expect(main).toContain("stageComposerPastedFiles")
    expect(main).toContain("stageComposerDroppedFiles")
    expect(main).toContain("offerComposerLargeTextPaste")
    expect(main).toContain("createCommandComposerHud")
    expect(pkg.dependencies["@openagentsinc/composer-state"]).toBe("workspace:*")
    expect(pkg.dependencies["@openagentsinc/three-effect"]).toContain("fa84064796")
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

  test("parses assistant prose as markdown instead of literal asterisks", () => {
    const blocks = parseMarkdownBlocks(
      "We can:\n\n- **Explore** files\n- Run `tests`\n\n[Docs](/docs) [bad](javascript:alert(1))",
    )
    const inline = parseMarkdownInline("**Explore** files, run `tests`, and read [docs](/docs). [bad](javascript:alert(1))")

    expect(blocks.map(block => block.kind)).toEqual([
      "paragraph",
      "unordered-list",
      "paragraph",
    ])
    expect(inline.some(part => part.kind === "strong")).toBe(true)
    expect(inline.some(part => part.kind === "code")).toBe(true)
    expect(inline.some(part => part.kind === "link" && part.href === "/docs")).toBe(true)
    expect(inline.some(part => part.kind === "link" && part.href.startsWith("javascript:"))).toBe(false)
  })

  test("parses tool transcripts without flattening terminal output", () => {
    expect(parseToolTranscript("ls: ok\n\n.:\nalpha.txt\nBeta.txt\nZoo/")).toEqual({
      output: ".:\nalpha.txt\nBeta.txt\nZoo/",
      status: "ok",
      toolName: "ls",
    })
    expect(parseToolTranscript("read: failed\n\nread_blocked_binary: read only supports text files")).toEqual({
      output: "read_blocked_binary: read only supports text files",
      status: "failed",
      toolName: "read",
    })
  })

  test("wraps long tool output instead of clipping errors offscreen", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".tool-card-output")
    expect(css).toContain("overflow-x: hidden")
    expect(css).toContain("overflow-wrap: anywhere")
    expect(css).toContain("white-space: pre-wrap")
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
