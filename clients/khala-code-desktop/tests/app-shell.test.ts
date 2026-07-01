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
import { projectUnifiedInbox } from "../src/ui/inbox"

describe("khala code desktop app shell", () => {
  test("registers the Khala Code desktop view", () => {
    expect(config.app).toMatchObject({
      identifier: "com.openagents.khala.code.desktop",
      name: "Khala Code",
      version: "0.1.0",
    })
    expect(config.build.bun.entrypoint).toBe("src/bun/index.ts")
    expect(config.build.views).toEqual({})
    expect(config.build.copy).toMatchObject({
      "dist/index.html": "views/khala-code-desktop/index.html",
      "dist/assets/": "views/khala-code-desktop/assets/",
    })
  })

  test("renders the chat shell with the fleet panel container", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()

    expect(html).toContain('class="khala-code-shell antialiased"')
    expect(html).toContain('id="message-list"')
    expect(html).toContain('id="composer-form"')
    expect(html).toContain("oa-ai-command-composer")
    expect(html).toContain("data-oa-command-composer")
    expect(html).toContain('id="composer-rail"')
    expect(html).toContain('id="composer-hud"')
    expect(html).toContain('id="composer-input"')
    expect(html).toContain('id="slash-command-palette"')
    expect(html).toContain("data-oa-command-composer-native-editing")
    expect(html).toContain("autofocus")
    expect(html).toContain('id="send-button"')
    expect(html).toContain('id="inbox-panel"')
    expect(html).toContain('id="fleet-panel"')
    expect(html).toContain('id="gym-panel"')
    expect(html).not.toContain("Pylons")
  })

  test("wires the Unified Inbox shell and local-safe projection", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('{ value: "inbox", children: ["Inbox"] }')
    expect(html).toContain('aria-label="Unified Inbox"')
    expect(main).toContain("mountUnifiedInboxPanel")
    expect(main).toContain("codexAppServerStatus")
    expect(main).toContain("codexAppServerStart")
    expect(main).toContain("codexTurnInterrupt")
    expect(main).toContain("khala-code-desktop.session-id.v1")
    expect(main).toContain("Requested Codex interrupt for the active turn")
    expect(main).toContain("codexItemStatus")
    expect(main).toContain("renderMessageBody(message.body, message.role, message.codexItem)")
    expect(main).toContain('const showInbox = value === "inbox"')
    expect(main).toContain("inboxPanel?.setVisible(showInbox)")
    expect(css).toContain(".khala-code-inbox")
    expect(css).toContain(".codex-item-card")
    expect(css).toContain(".codex-item-card-copy")
    expect(css).toContain(".khala-inbox-coverage-row[data-status=\"not_connected\"]")

    const projection = projectUnifiedInbox({
      codexHarness: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "codex_harness",
        observedAt: "2026-06-30T00:00:00.000Z",
        reason: "ready",
        status: "ready",
        binary: {
          command: "codex",
          source: "PATH",
          available: true,
          version: "codex-cli 1.2.3",
          error: null,
        },
        home: {
          path: "/tmp/codex-home",
          source: "input",
          role: "main_user_codex_home",
          authPath: "/tmp/codex-home/auth.json",
          fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
        },
        auth: {
          state: "ready",
          blockerRefs: [],
          accessTokenPresent: true,
          accountIdPresent: false,
          refreshTokenPresent: false,
        },
        signIn: {
          required: false,
          command: "codex login",
          warning: "fleet accounts stay isolated",
        },
      },
      fleet: {
        ok: false,
        observedAt: "2026-06-30T00:00:00.000Z",
        pylon: {
          status: "unavailable",
          pylonRef: null,
          message: "Pylon offline",
        },
        availableCodexAssignments: null,
        maxCodexAssignments: null,
        tokenRate: {
          activeAdjustedTokensPerMinute: null,
          completedStatus: "not_measured",
          completedTokenRows: null,
          completedTokensPerMinute: null,
          inFlightTokens: null,
          inFlightTokensPerMinute: null,
          source: "unavailable",
          unavailableReason: "Pylon offline",
        },
        accounts: [{
          accountRef: "codex-2",
          capacity: null,
          provider: "codex",
          readiness: "credentials_missing",
          quotaState: null,
          accountKey: null,
          email: null,
        }],
        activeAssignments: [{
          assignmentRef: "assignment.khala.demo",
          elapsedMs: null,
          issueRef: "github.issue.openagents.7760",
          tokenRate: {
            source: "unavailable",
            status: "not_measured",
            tokenCountKind: null,
            tokens: null,
            tokensPerMinute: null,
          },
          updatedAt: "2026-06-30T00:01:00.000Z",
        }],
        processes: [],
      },
      pylon: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "pylon",
        observedAt: "2026-06-30T00:00:00.000Z",
        reason: "Pylon offline",
        status: "unavailable",
      },
      coding: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "coding",
        observedAt: "2026-06-30T00:00:00.000Z",
        reason: "ready",
        status: "ready",
      },
      tokenAccounting: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "token_accounting",
        observedAt: "2026-06-30T00:00:00.000Z",
        reason: "not configured",
        status: "not_configured",
      },
    })

    expect(projection.items.map(item => item.kind)).toEqual([
      "run_blocked",
      "missing_credential",
      "ready_for_review",
    ])
    expect(projection.items[1]).toMatchObject({
      accountRef: "codex-2",
      actions: ["reconnect", "open_fleet"],
    })
    expect(projection.items[2]).toMatchObject({
      assignmentRef: "assignment.khala.demo",
      resumeCommand: "khala closeout assignment.khala.demo --json",
    })
    expect(projection.coverage).toContainEqual({
      source: "Codex harness",
      status: "connected",
      summary: "The main local Codex install and Codex home are ready for wrapper sessions.",
    })
    expect(projection.coverage).toContainEqual({
      source: "approval queue",
      status: "not_connected",
      summary: "Pylon permission prompts are not exposed through the desktop RPC yet.",
    })
  })

  test("projects missing main Codex auth into the Unified Inbox", () => {
    const baseFleet = {
      ok: true,
      observedAt: "2026-07-01T00:00:00.000Z",
      pylon: {
        status: "started" as const,
        pylonRef: "pylon.local",
        message: "Pylon ready",
      },
      availableCodexAssignments: 0,
      maxCodexAssignments: 0,
      tokenRate: {
        activeAdjustedTokensPerMinute: null,
        completedStatus: "not_measured" as const,
        completedTokenRows: null,
        completedTokensPerMinute: null,
        inFlightTokens: null,
        inFlightTokensPerMinute: null,
        source: "unavailable" as const,
        unavailableReason: null,
      },
      accounts: [],
      activeAssignments: [],
      processes: [],
    }
    const projection = projectUnifiedInbox({
      codexHarness: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "codex_harness",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "Codex auth.json is missing. Run codex login intentionally in the main user home.",
        status: "unavailable",
        binary: {
          command: "codex",
          source: "PATH",
          available: true,
          version: "codex-cli 1.2.3",
          error: null,
        },
        home: {
          path: "/tmp/codex-home",
          source: "input",
          role: "main_user_codex_home",
          authPath: "/tmp/codex-home/auth.json",
          fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
        },
        auth: {
          state: "credentials_missing",
          blockerRefs: ["blocker.codex.credentials_missing"],
          accessTokenPresent: false,
          accountIdPresent: false,
          refreshTokenPresent: false,
          error: "Codex auth.json is missing.",
        },
        signIn: {
          required: true,
          command: "codex login",
          warning: "fleet accounts stay isolated",
        },
      },
      fleet: baseFleet,
      pylon: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "pylon",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "Pylon ready",
        status: "ready",
      },
      coding: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "coding",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "Codex auth.json is missing.",
        status: "unavailable",
      },
      tokenAccounting: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "token_accounting",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "not configured",
        status: "not_configured",
      },
    })

    expect(projection.items.find(item => item.ref === "inbox.runtime.codex_harness.unavailable")).toMatchObject({
      ref: "inbox.runtime.codex_harness.unavailable",
      kind: "missing_credential",
      title: "Codex setup required",
      source: "runtime",
      severity: "critical",
    })
  })

  test("renders Fleet Status capacity and token evidence chips", async () => {
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const fleetPanel = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(rpc).toContain("readonly capacity: KhalaCodeDesktopFleetCapacity | null")
    expect(handlers).toContain("capacity: account.capacity")
    expect(fleetPanel).toContain("isDisplayOnlyDefaultAccountRef")
    expect(fleetPanel).toContain("accountCapacityLabel")
    expect(fleetPanel).toContain("fleetTokenRateLabel")
    expect(fleetPanel).toContain("assignmentTokenRateLabel")
    expect(fleetPanel).toContain('appendChip(details, "routing", "default slot")')
    expect(fleetPanel).toContain('appendChip(details, "slots", accountCapacityLabel(account.capacity))')
    expect(fleetPanel).toContain('"busy"')
    expect(fleetPanel).toContain("account.capacity.busy")
    expect(fleetPanel).toContain('"queued"')
    expect(fleetPanel).toContain("account.capacity.queued")
    expect(fleetPanel).toContain('appendChip(pylonDetails, "token rate"')
    expect(fleetPanel).toContain('appendChip(chips, "tokens"')
    expect(css).toContain(".khala-fleet-card-details")
  })

  test("renders Fleet status with a board graph and run timeline mount", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const panel = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    const projection = await Bun.file(new URL("../src/ui/fleet-board-projection.ts", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/fleet-board-renderer.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("mountFleetPanel")
    expect(panel).toContain("appendFleetBoard(container, data)")
    expect(panel).toContain("buildKhalaFleetBoardProjection")
    expect(panel).toContain("renderKhalaFleetBoardHtml")
    expect(projection).toContain("openagents.khala_code.fleet_board_projection.v0")
    expect(renderer).toContain("Fleet board graph and run timeline")
    expect(css).toContain(".khala-fleet-board-summary")
    expect(css).toContain(".khala-fleet-timeline-event")
  })

  test("renders a visible Gym pane entry without seeded private proof data", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const pane = await Bun.file(new URL("../src/ui/gym-pane.ts", import.meta.url)).text()
    const loader = await Bun.file(new URL("../src/ui/gym-proof-loader.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('{ value: "gym", children: ["Gym"] }')
    expect(html).toContain('aria-label="Gym delegation graph"')
    expect(main).toContain("mountGymPane")
    expect(main).toContain("gymPaneStateFromLocation")
    expect(main).toContain("loadGymProof")
    expect(main).toContain("loadGymDemoProof")
    expect(main).toContain('const showGym = value === "gym"')
    expect(main).toContain("gymPanel?.setVisible(showGym)")
    expect(pane).toContain('phase: "empty"')
    expect(pane).toContain('phase: "loaded"')
    expect(pane).toContain('phase: "blocked"')
    expect(pane).toContain("No Gym proof loaded.")
    expect(loader).toContain("khalaCodeGymDemoBridgeProof")
    expect(loader).toContain("decisionGrade: false")
    expect(loader).toContain('proof === "fixture"')
    expect(css).toContain(".khala-code-gym")
    expect(css).toContain(".khala-gym-detail-grid")
    expect(css).toContain(".khala-gym-state[data-state=\"blocked\"]")
    expect(`${html}\n${sidebar}\n${main}\n${pane}\n${loader}`).not.toMatch(
      /\/Users\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace)|sk-[a-z0-9]/i,
    )
  })

  test("does not seed dummy code or diff messages on first load", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(main).not.toContain("assistant-code")
    expect(main).not.toContain("assistant-diff")
    expect(main).not.toContain("```diff")
    expect(main).not.toContain("QueueItem")
  })

  test("starts without a seeded assistant greeting", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(main).toContain("let messages: KhalaCodeDesktopMessage[] = []")
    expect(main).toContain("messages = []")
    expect(main).not.toContain("initialMessages")
    expect(main).not.toContain("Khala Code is awake")
    expect(main).not.toContain("assistant-wake")
    expect(main).not.toContain("Point us at a repo")
    expect(main).not.toContain("we will keep the patch")
    expect(main).not.toContain("Point me at a repo")
    expect(main).not.toContain("I will keep")
  })

  test("renders messages without speaker labels above them", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("article.append(body)")
    expect(main).not.toContain("roleLabel")
    expect(main).not.toContain("message-label")
    expect(main).not.toContain('return "You"')
    expect(main).not.toContain('return "Khala Code"')
    expect(css).not.toContain(".message-label")
  })

  test("keeps composer focus styling on the frame instead of the textarea", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".khala-code-composer:focus-within .oa-ai-command-composer-frame")
    expect(css).not.toContain("#composer-input:focus-visible")
  })

  test("wires Codex slash command palette and dispatch affordances", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(html).toContain('class="khala-code-slash-command-palette"')
    expect(css).toContain(".khala-code-slash-command-option")
    expect(css).toContain(".khala-code-slash-command-palette[hidden]")
    expect(main).toContain("slashCommandList")
    expect(main).toContain("slashCommandDispatch")
    expect(main).toContain("draftText.startsWith(\"/\")")
  })

  test("wires Codex approval response controls without the legacy permission dispatcher", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/transcript-render.ts", import.meta.url)).text()

    expect(renderer).toContain("codex-approval-button")
    expect(renderer).toContain("codexApprovalControlsElement")
    expect(main).toContain("codexApprovalRespond")
    expect(main).toContain("respondToCodexApproval")
    expect(main).not.toContain("allowAllKhalaPermissionService")
    expect(css).toContain(".codex-approval-controls")
  })

  test("keeps the composer footer controls in a clean inline strip", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html).not.toContain('id="preview-button"')
    expect(html).not.toContain('id="resize-button"')
    expect(html).not.toContain('data-oa-command-composer-control="Preview"')
    expect(html).not.toContain("data-oa-command-composer-resize")
    expect(css).toContain("grid-template-columns: minmax(0, 1fr) minmax(0, auto) 32px")
    expect(css).not.toContain("grid-template-columns: minmax(0, 1fr) minmax(0, auto) 32px 32px")
    expect(css).toContain(".khala-code-composer .oa-ai-command-composer-submit-label")
    expect(main).not.toContain("previewButton")
    expect(main).not.toContain("resizeButton")
    expect(main).not.toContain("composerExpanded")
    expect(main).not.toContain("togglePreview")
  })

  test("starts the composer compact with the placeholder near the top edge", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html).toContain('rows="2"')
    expect(css).toContain("--oa-command-composer-height: 6rem")
    expect(css).toContain("padding: 8px 10px")
    expect(css).toContain("min-height: 2.5rem")
    expect(css).toContain("#composer-input")
    expect(css).toContain("padding: 0")
    expect(css).not.toContain("--oa-command-composer-height: 8rem")
    expect(css).not.toContain("--oa-command-composer-height: 10.25rem")
    expect(css).not.toContain("min-height: 4.25rem")
    expect(css).not.toContain("min-height: 9rem")
    expect(css).not.toContain("padding: 16px 46px 48px 0")
  })

  test("keeps transcript scrolling full width while messages and composer stay on the 768px rail", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".message-list")
    expect(css).toContain("max-width: none")
    expect(css).toContain("scrollbar-gutter: stable")
    expect(css).toContain("margin-left: max(4px, calc((100% - 768px) / 2))")
    expect(css).toContain("margin-right: max(4px, calc((100% - 768px) / 2))")
    expect(css).toContain(".khala-code-composer")
    expect(css).toContain("max-width: 768px")
    expect(css).not.toContain("width: min(100%, 48rem)")
  })

  test("lets the transcript bleed vertically to the window and composer", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".khala-code-thread-shell")
    expect(css).toContain("padding: 0")
    expect(css).toContain("height: 100%")
    expect(css).toContain("padding: 0 14px 32px")
    expect(css).toContain("scroll-padding-bottom: 32px")
    expect(css).toContain("overscroll-behavior: contain")
    expect(css).toContain("display: flex")
    expect(css).toContain("flex-direction: column")
    expect(css).toContain(".message-list::before")
    expect(css).toContain("margin-top: auto")
    expect(css).not.toContain("align-content: end")
    expect(css).not.toContain("padding: 28px 14px 12px")
    expect(css).not.toContain("padding: 10px 4px 20px")
    expect(css).not.toContain("padding-top: 16px")
    expect(css).not.toContain("max-height: calc(100vh - 188px)")
    expect(css).not.toContain("max-height: calc(100dvh - 188px)")
    expect(css).not.toContain("max-height: calc(100vh - 176px)")
    expect(css).not.toContain("max-height: calc(100dvh - 176px)")
  })

  test("keeps user messages right anchored with left-aligned text", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".message-bubble--user {\n  align-self: flex-end;")
    expect(css).toContain("  justify-self: end;")
    expect(css).toContain("  justify-items: start;\n}")
    expect(css).toContain(".message-bubble--user .message-prose {\n  text-align: left;\n}")
    expect(css).not.toContain("  align-self: stretch;\n}")
    expect(css).not.toContain("  justify-items: end;\n}")
    expect(css).not.toContain(".message-bubble--user .message-prose {\n  text-align: right;\n}")
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

  test("keeps transcript scrolling user-controlled during streaming updates", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("const isNearTranscriptEnd")
    expect(main).toContain("let transcriptPinnedToEnd = true")
    expect(main).toContain("const stickToEnd = transcriptPinnedToEnd && isNearTranscriptEnd()")
    expect(main).toContain("const previousScrollTop = messageList.scrollTop")
    expect(main).toContain("setTranscriptScrollTop(previousScrollTop)")
    expect(main).toContain("proxyTranscriptWheel")
    expect(main).toContain("window.addEventListener(\"wheel\", proxyTranscriptWheel, { passive: false })")
    expect(css).toContain(".tool-card-output")
    expect(css).toContain("max-height: 7rem")
  })

  test("shows a Thinking shimmer until the first streamed response event", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("@openagentsinc/ui/ai-elements/shimmer")
    expect(main).toContain("let thinkingTurnId: string | null = null")
    expect(main).toContain("renderThinkingIndicator")
    expect(main).toContain("message-bubble--thinking")
    expect(main).toContain('shimmer.textContent = "Thinking"')
    expect(main).toContain("thinkingTurnId = turnId")
    expect(main).toContain('event.type === "message_start"')
    expect(main).toContain('event.type === "message_delta"')
    expect(main).toContain('event.type === "message_replace"')
    expect(main).toContain("thinkingTurnId = null")
    expect(css).toContain(".message-bubble--thinking")
    expect(css).toContain(".message-bubble--thinking .oa-ai-shimmer")
    expect(css).toContain("font-size: 0.8125rem")
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

  test("keeps tool names with underscores literal in markdown prose", () => {
    const inline = parseMarkdownInline(
      "Tools include exec_command, codex_spawn, pylon_ensure, and codex_fleet_status.",
    )

    expect(inline).toEqual([
      {
        kind: "text",
        text: "Tools include exec_command, codex_spawn, pylon_ensure, and codex_fleet_status.",
      },
    ])
  })

  test("hides dangling markdown markers while a response is still streaming", () => {
    expect(parseMarkdownInline("We can **search files")).toEqual([
      { kind: "text", text: "We can search files" },
    ])
    expect(parseMarkdownInline("Run `bun test")).toEqual([
      { kind: "text", text: "Run bun test" },
    ])
    expect(parseMarkdownInline("Use _preview")).toEqual([
      { kind: "text", text: "Use preview" },
    ])
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
    expect(parseToolTranscript("codex_spawn: running\n\nPreparing the Pylon/Codex handoff...")).toEqual({
      output: "Preparing the Pylon/Codex handoff...",
      status: "running",
      toolName: "codex_spawn",
    })
    expect(parseToolTranscript("codex_spawn: ok\n\nCodex spawn: accepted 0/1\n- slot 1: failed\n  command timed out")).toEqual({
      output: "Codex spawn: accepted 0/1\n- slot 1: failed\n  command timed out",
      status: "failed",
      toolName: "codex_spawn",
    })
  })

  test("wraps long tool output instead of clipping errors offscreen", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".tool-card-output")
    expect(css).toContain("overflow-x: hidden")
    expect(css).toContain("overflow-y: auto")
    expect(css).toContain("overflow-wrap: anywhere")
    expect(css).toContain("white-space: pre-wrap")
    expect(css).toContain('[data-expanded="true"] .tool-card-output')
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
