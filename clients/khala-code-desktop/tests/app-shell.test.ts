import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"
import {
  parseMarkdownBlocks,
  parseMarkdownInline,
} from "@openagentsinc/ui/ai-elements/markdown"

import config from "../electrobun.config.js"
import { khalaCodeDesktopApplicationMenu } from "../src/bun/application-menu"
import {
  compactToolSummary,
  parseMessageSegments,
  parseToolTranscript,
} from "../src/ui/transcript-render"
import { projectUnifiedInbox } from "../src/ui/inbox"
import {
  mountKhalaCodeSidebar,
  projectKhalaCodeSidebarFleetCounts,
} from "../src/ui/sidebar"
import {
  projectKhalaCodeDesktopCodexEcosystem,
} from "../src/shared/codex-ecosystem"
import {
  displayLocalPathsForKhalaCode,
  displayPathForKhalaCode,
} from "../src/shared/display-paths"

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

  test("packages the Khala Code app icon for desktop platforms", async () => {
    expect(config.build.mac?.icons).toBe("resources/AppIcon.iconset")
    expect(config.build.win?.icon).toBe("resources/khala-code-app-icon.png")
    expect(config.build.linux?.icon).toBe("resources/khala-code-app-icon.png")

    const icon = Bun.file(new URL("../resources/khala-code-app-icon.png", import.meta.url))
    const iconset = [
      "icon_16x16.png",
      "icon_16x16@2x.png",
      "icon_32x32.png",
      "icon_32x32@2x.png",
      "icon_128x128.png",
      "icon_128x128@2x.png",
      "icon_256x256.png",
      "icon_256x256@2x.png",
      "icon_512x512.png",
      "icon_512x512@2x.png",
    ]

    expect(await icon.exists()).toBe(true)
    for (const name of iconset) {
      expect(
        await Bun.file(new URL(`../resources/AppIcon.iconset/${name}`, import.meta.url)).exists(),
      ).toBe(true)
    }
  })

  test("opens the desktop window with full-bleed titlebar content", async () => {
    const source = await Bun.file(new URL("../src/bun/index.ts", import.meta.url)).text()

    expect(source).toContain("Screen.getPrimaryDisplay().workArea")
    expect(source).toContain("resolveMainWindowFrame()")
    expect(source).toContain("frame: resolveMainWindowFrame()")
    expect(source).toContain('titleBarStyle: "hiddenInset"')
    expect(source).toContain("FALLBACK_MAIN_WINDOW_FRAME")
    expect(source).not.toContain("setFullScreen(true)")
    expect(source).not.toContain("FullScreen: true")
  })

  test("keeps the Apple FM bridge disabled in launch startup", async () => {
    const entrypoint = await Bun.file(new URL("../src/bun/index.ts", import.meta.url)).text()
    const deciderHost = await Bun.file(
      new URL("../src/bun/on-device-decider-host.ts", import.meta.url),
    ).text()

    expect(entrypoint).not.toContain('from "./apple-fm-sidecar.js"')
    expect(entrypoint).not.toContain("createAppleFmSidecarHost")
    expect(entrypoint).not.toContain("sidecar:")
    expect(entrypoint).toContain("buildKhalaAppleFmDisabledReadiness")
    expect(deciderHost).toContain("readonly appleFmEnabled?: boolean")
    expect(deciderHost).toContain("options.appleFmEnabled === true")
  })

  test("renders the chat shell with the fleet panel container", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()

    expect(html).toContain('class="khala-code-shell antialiased"')
    expect(html).toContain('id="sidebar-root" class="khala-code-sidebar-shell"')
    expect(html).toContain('id="sidebar-nav-root" class="khala-code-sidebar"')
    expect(html).toContain('id="message-list"')
    expect(html).toContain('id="thread-token-counter"')
    expect(html).toContain('id="thread-token-popover"')
    expect(html).toContain('id="thread-sidebar"')
    expect(html).toContain('id="composer-form"')
    expect(html).toContain("oa-ai-command-composer")
    expect(html).toContain("data-oa-command-composer")
    expect(html).toContain('id="composer-rail"')
    expect(html).toContain('id="composer-hud"')
    expect(html).toContain('id="composer-follow-up-queue"')
    expect(html).toContain('id="composer-input"')
    expect(html).toContain('id="slash-command-palette"')
    expect(html).toContain("data-oa-command-composer-native-editing")
    expect(html).toContain("autofocus")
    expect(html).toContain('id="send-button"')
    expect(html).toContain('id="fleet-panel"')
    expect(html).toContain('id="forum-panel"')
    expect(html).toContain('id="inbox-panel"')
    expect(html).toContain('id="gym-panel"')
    expect(html).toContain('id="settings-panel"')
    expect(html).not.toContain('id="foldkit-demo-panel"')
    expect(html).not.toContain("Foldkit embedding skeleton")
    expect(html).not.toContain("Pylons")
  })

  test("wires the architect plan-first composer surface", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(rpc).toContain("architectPlanRun")
    expect(rpc).toContain("architectPlanDecision")
    expect(rpc).toContain("openagents.khala_code.architect_plan_artifact.v1")
    expect(model).toContain("architectPlanArtifact")
    expect(model).toContain("architectPlanMode")
    expect(model).toContain("architectPlanPending")
    expect(main).toContain("architectSlashCommand")
    expect(main).toContain('command: "architect"')
    expect(main).toContain('action: "architect_plan"')
    expect(main).toContain("renderArchitectPlanModeButton")
    expect(main).toContain("submitArchitectPlan")
    expect(main).toContain('draftText.startsWith("/architect ")')
    expect(main).toContain("renderArchitectPlanCard")
    expect(main).toContain("architectPlanDecision")
    expect(css).toContain(".khala-architect-plan-toggle")
    expect(css).toContain(".khala-architect-plan-card")
    expect(css).toContain(".khala-architect-plan-actions")
  })

  test("keeps backend runtime hooks while visible composer selectors stay commented out", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const executableMain = main
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/\/\/.*$/gm, "")
    const activeCss = css.replace(/\/\*[\s\S]*?\*\//g, "")
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()

    expect(main).toContain("// renderHarnessPill()")
    expect(main).toContain("// renderRuntimeBadge()")
    expect(main).toContain("// renderMicrophoneIndicator()")
    expect(main).toContain('{ label: "Codex", mode: "codex_harness" }')
    expect(main).toContain('{ label: "Claude", mode: "claude_runtime" }')
    expect(main).toContain('{ label: "Khala", mode: "khala_native_runtime" }')
    expect(main).toContain("rpc.request.harnessSettingRead()")
    expect(executableMain).not.toContain("rpc.request.harnessSettingWrite({ mode })")
    expect(main).toContain("response.backend.runtimeMode")
    expect(executableMain).not.toContain("renderHarnessPill()")
    expect(executableMain).not.toContain("renderRuntimeBadge()")
    expect(executableMain).not.toContain("renderMicrophoneIndicator()")
    expect(activeCss).not.toContain(".khala-harness-pill")
    expect(activeCss).not.toContain(".khala-runtime-badge")
    expect(activeCss).not.toContain(".khala-microphone-indicator")
    expect(activeCss).not.toContain("oa-ai-command-composer-status::before")
    expect(rpc).toContain("harnessSettingRead")
    expect(rpc).toContain("harnessSettingWrite")
    expect(handlers).toContain("readKhalaCodeDesktopPersistedHarnessMode")
    expect(handlers).toContain("khalaCodeDesktopRuntimeEnvOverride")
    expect(handlers).toContain("Legacy Khala native runtime handled this turn.")
  })

  test("projects typed degraded states for every boot-time RPC failure", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()

    expect(html).toContain('id="boot-degraded-states"')
    expect(model).toContain('kind: "khala_code_boot_rpc_degraded"')
    expect(main).toContain("recordBootRpcDegradedState")
    expect(main).toContain("bootDegradedStates: () => shellModel().bootDegradedStates")

    for (const method of [
      "harnessSettingRead",
      "sessionCatalog",
      "events",
      "claudeApprovalPending",
      "fleetRunList",
      "codexFleetStatus",
    ]) {
      expect(model).toContain(`| "${method}"`)
      expect(main).toContain(`recordBootRpcDegradedState("${method}"`)
      expect(main).toContain(`clearBootRpcDegradedState("${method}"`)
    }

    expect(main).toContain("degradedSessionCatalog(")
    expect(main).toContain("degradedFleetStatus(")
    expect(main).toContain("degradedFleetRunList()")
    expect(main).not.toContain("console.error")
  })

  test("uses the shared blue sci-fi UI tokens and licensed-safe chat fonts", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain('@import "@openagentsinc/ui/styles.css";')
    expect(css).not.toContain("packages/design-tokens/src/theme.css")
    expect(css).not.toContain("Berkeley Mono")
    expect(css).not.toContain("@font-face")
    expect(css).toContain("font-family: var(--oa-font-sans)")
    expect(css).toContain(".message-prose")
    expect(css).toContain(".khala-thread-token-meter")
    expect(css).toContain(".khala-thread-token-popover-row")
    expect(css).toContain("font-family: var(--oa-font-code)")
    expect(css).toContain("var(--oa-color-khala-energy-cyan)")
    expect(css).toContain("var(--oa-color-khala-surface)")
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgba?\(/)
    expect(css).not.toMatch(/oa-color-(accent|warning|primary|bg|danger|success|info|review|hud)/)
  })

  test("wires an active-thread token meter to local token accounting ledgers", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()
    const telemetry = await Bun.file(new URL("../src/bun/codex-token-usage-telemetry.ts", import.meta.url)).text()

    expect(rpc).toContain("KhalaCodeDesktopThreadTokenSummary")
    expect(rpc).toContain("threadTokenSummary")
    expect(handlers).toContain("readKhalaCodeDesktopThreadTokenSummary")
    expect(telemetry).toContain("message-token-audit.jsonl")
    expect(telemetry).toContain("token-usage-report-failures.jsonl")
    expect(main).toContain("refreshThreadTokenSummary")
    expect(main).toContain("Leaderboard synced")
    expect(main).toContain('event.type === "thread_ready"')
  })

  test("mounts the Unified Inbox shell and keeps local-safe projection logic", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('value: "inbox"')
    expect(sidebar).toContain('icon: "NotificationBell"')
    expect(html).toContain('aria-label="Unified Inbox"')
    expect(main).toContain("mountUnifiedInboxPanel")
    expect(main).toContain('const showInbox = activeValue === "inbox"')
    expect(main).toContain("inboxPanel?.setVisible(showInbox)")
    expect(main).toContain("ecosystem: await controls.codexEcosystemRead")
    expect(main).toContain("await controls.fleetRunControl({ runRef, verb: \"resume\" })")
    expect(main).toContain("codexAppServerStatus")
    expect(main).toContain("codexAppServerStart")
    expect(main).toContain("codexMcpToolCall")
    expect(main).toContain("codexTurnInterrupt")
    expect(main).toContain("khala-code-desktop.session-id.v1")
    expect(main).toContain("Requested Codex interrupt for the active turn")
    expect(main).toContain("codexItemStatus")
    expect(main).toContain("renderMessageBody(message.body, message.role, message.codexItem)")
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
      "credentials_missing",
      "run_blocked",
      "ready_for_review",
    ])
    expect(projection.items.find(item => item.kind === "credentials_missing")).toMatchObject({
      accountRef: "codex-2",
      actions: ["reconnect", "open_fleet"],
    })
    expect(projection.items.find(item => item.kind === "ready_for_review")).toMatchObject({
      assignmentRef: "assignment.khala.demo",
      resumeCommand: "khala closeout assignment.khala.demo --json",
    })
    expect(projection.coverage).toContainEqual({
      source: "Codex harness",
      status: "connected",
      summary: "The primary user Codex install and Codex home are ready for wrapper sessions.",
    })
    expect(projection.coverage).toContainEqual({
      source: "approval queue",
      status: "connected",
      summary: "Worker approval, blocker, and review events are projected from Fleet assignment metadata into Inbox.",
    })
  })

  test("mounts the OpenAgents Forum hotbar surface with Product Promises parity hooks", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const forumPanel = await Bun.file(new URL("../src/ui/forum-panel.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('label: "Forum"')
    expect(sidebar).toContain('value: "forum"')
    expect(sidebar).toContain('icon: "BookOpen"')
    expect(sidebar).toContain('actionId: "action_bar.slot_3"')
    expect(sidebar).toContain('hotkey: "3"')
    expect(html).toContain('aria-label="OpenAgents Forum"')
    expect(main).toContain("mountKhalaCodeForumPanel")
    expect(main).toContain('const showForum = activeValue === "forum"')
    expect(main).toContain("forumPanel?.setVisible(showForum)")
    expect(main).toContain("forumRequest: request =>")
    expect(main).toContain("const result = await controls.forumRequest(request)")
    expect(forumPanel).toContain('const ProductPromisesForumRef = "product-promises"')
    expect(forumPanel).toContain('"/api/forum/launch-status"')
    expect(forumPanel).toContain('`/api/forum/forums/${encodeURIComponent(forumRef)}/topics`')
    expect(forumPanel).toContain('`/api/forum/topics/${encodeURIComponent(topicId)}`')
    expect(forumPanel).toContain('`/api/forum/posts/${encodeURIComponent(postId)}/tips/ladder`')
    expect(forumPanel).toContain('`/api/forum/posts/${encodeURIComponent(postId)}/reports`')
    expect(forumPanel).toContain('credentials: "include"')
    expect(forumPanel).toContain('options.openExternal(externalPath(baseUrl, "/forum/f/product-promises"))')
    expect(css).toContain(".khala-code-forum")
    expect(css).toContain(".khala-forum-post-actions")
  })

  test("routes worker approval and blocker events into the Unified Inbox", () => {
    const projection = projectUnifiedInbox({
      fleet: {
        ok: true,
        observedAt: "2026-07-01T00:00:00.000Z",
        pylon: {
          status: "online",
          pylonRef: "pylon.local",
          message: "Pylon ready",
        },
        availableCodexAssignments: 0,
        maxCodexAssignments: 1,
        tokenRate: {
          activeAdjustedTokensPerMinute: null,
          completedStatus: "pending",
          completedTokenRows: null,
          completedTokensPerMinute: null,
          inFlightTokens: 128,
          inFlightTokensPerMinute: 64,
          source: "pylon_khala_apm",
          unavailableReason: null,
        },
        accounts: [{
          accountRef: "codex-worker",
          accountKey: "worker.public",
          capacity: {
            available: 0,
            busy: 1,
            queued: 1,
            ready: 1,
          },
          email: null,
          homeRole: "pylon_isolated_worker_codex_home",
          provider: "codex",
          queuePolicy: {
            admission: "pylon_capacity_gate",
            cooldown: "ready",
            refill: "pylon_presence_heartbeat",
            queued: 1,
          },
          quotaState: "available",
          readiness: "ready",
          sessionRole: "swarm_worker_codex_session",
        }],
        activeAssignments: [{
          assignmentRef: "assignment.public.approval",
          blockerRefs: ["blocker.public.worker.approval_required"],
          closeoutStatus: null,
          elapsedMs: 10_000,
          issueRef: "github.issue.openagents.7791",
          tokenRate: {
            source: "token_usage_events",
            status: "pending",
            tokenCountKind: null,
            tokens: null,
            tokensPerMinute: null,
          },
          updatedAt: "2026-07-01T00:01:00.000Z",
          workerSession: {
            approvalState: "approval_required",
            blockerRefs: ["blocker.public.worker.approval_required"],
            closeoutStatus: null,
            executionRuntime: "codex_harness",
            homeRole: "pylon_isolated_worker_codex_home",
            queuePolicy: {
              admission: "pylon_capacity_gate",
              cooldown: "unknown",
              refill: "pylon_presence_heartbeat",
              queued: null,
            },
            reviewState: "blocked",
            role: "swarm_worker_codex_session",
            transcriptRef: "transcript.public.approval",
          },
        }],
        processes: [],
        sessionLayers: {
          main: {
            homeRole: "main_user_codex_home_display_only",
            label: "Primary user Codex session",
            mutationPolicy: "codex_app_server_owned",
            role: "main_local_codex_session",
            runtime: "codex_harness",
            transcriptSurface: "chat",
          },
          workers: {
            homeRole: "pylon_isolated_worker_codex_home",
            label: "Khala swarm worker Codex sessions",
            mutationPolicy: "pylon_isolated_home_only",
            role: "swarm_worker_codex_session",
            runtime: "codex_harness",
            transcriptSurface: "fleet",
          },
        },
      },
      pylon: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "pylon",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "ready",
        status: "ready",
      },
      coding: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "coding",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "ready",
        status: "ready",
      },
      tokenAccounting: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "token_accounting",
        observedAt: "2026-07-01T00:00:00.000Z",
        reason: "ready",
        status: "ready",
      },
    })

    expect(projection.items).toHaveLength(1)
    expect(projection.items[0]).toMatchObject({
      actions: ["open_fleet", "refresh"],
      assignmentRef: "assignment.public.approval",
      kind: "approval_required",
      severity: "critical",
      source: "assignment",
      title: "github.issue.openagents.7791 needs approval",
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
        reason: "Codex auth.json is missing. Run codex login intentionally for the primary user Codex home before using Khala Code chat.",
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
          warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
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
      kind: "credentials_missing",
      title: "Codex install or sign-in required",
      source: "runtime",
      severity: "critical",
    })
  })

  test("projects persistent token usage reporting failures into the Unified Inbox", () => {
    const observedAt = "2026-07-01T00:00:00.000Z"
    const projection = projectUnifiedInbox({
      fleet: {
        ok: true,
        observedAt,
        pylon: {
          status: "started",
          pylonRef: "pylon.local",
          message: "Pylon ready",
        },
        availableCodexAssignments: 0,
        maxCodexAssignments: 0,
        tokenRate: {
          activeAdjustedTokensPerMinute: null,
          completedStatus: "not_measured",
          completedTokenRows: null,
          completedTokensPerMinute: null,
          inFlightTokens: null,
          inFlightTokensPerMinute: null,
          source: "unavailable",
          unavailableReason: null,
        },
        accounts: [],
        activeAssignments: [],
        processes: [],
      },
      pylon: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "pylon",
        observedAt,
        reason: "ready",
        status: "ready",
      },
      coding: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "coding",
        observedAt,
        reason: "ready",
        status: "ready",
      },
      tokenAccounting: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "token_accounting",
        observedAt,
        reason: "1 token usage reporting failure flag(s) need review; latest: token usage sync failed",
        status: "error",
      },
    })

    expect(projection.items).toContainEqual(expect.objectContaining({
      ref: "inbox.runtime.token_accounting.blocked",
      kind: "run_blocked",
      title: "Token accounting needs review",
      source: "runtime",
      severity: "critical",
    }))
  })

  test("projects Codex ecosystem diagnostics into the Unified Inbox", () => {
    const observedAt = "2026-07-01T00:00:00.000Z"
    const ecosystem = projectKhalaCodeDesktopCodexEcosystem({
      observedAt,
      mcpServerStatusList: {
        data: [{
          name: "github",
          serverInfo: null,
          tools: {},
          resources: [],
          resourceTemplates: [],
          authStatus: "notLoggedIn",
        }],
        nextCursor: null,
      },
      notifications: [{
        method: "skills/changed",
        params: {},
        receivedAt: "2026-07-01T00:01:00.000Z",
      }],
    })
    const projection = projectUnifiedInbox({
      ecosystem,
      fleet: {
        ok: true,
        observedAt,
        pylon: {
          status: "started",
          pylonRef: "pylon.local",
          message: "Pylon ready",
        },
        availableCodexAssignments: 0,
        maxCodexAssignments: 0,
        tokenRate: {
          activeAdjustedTokensPerMinute: null,
          completedStatus: "not_measured",
          completedTokenRows: null,
          completedTokensPerMinute: null,
          inFlightTokens: null,
          inFlightTokensPerMinute: null,
          source: "unavailable",
          unavailableReason: null,
        },
        accounts: [],
        activeAssignments: [],
        processes: [],
      },
      pylon: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "pylon",
        observedAt,
        reason: "Pylon ready",
        status: "ready",
      },
      coding: {
        ok: true,
        app: "Khala Code Desktop",
        available: true,
        capability: "coding",
        observedAt,
        reason: "ready",
        status: "ready",
      },
      tokenAccounting: {
        ok: true,
        app: "Khala Code Desktop",
        available: false,
        capability: "token_accounting",
        observedAt,
        reason: "not configured",
        status: "not_configured",
      },
    })

    expect(projection.items.map(item => item.kind)).toEqual([
      "mcp_failed",
      "codex_ecosystem",
    ])
    expect(projection.items[0]).toMatchObject({
      source: "mcp",
      actions: ["open_settings", "refresh"],
    })
    expect(projection.items[1]).toMatchObject({
      source: "codex_ecosystem",
      actions: ["refresh"],
    })
    expect(projection.coverage).toContainEqual({
      source: "MCP failures",
      status: "connected",
      summary: "1 Codex MCP servers projected with 1 auth blockers.",
    })
  })

  test("renders Fleet Status capacity and token evidence chips", async () => {
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const fleetPanel = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(rpc).toContain("export type KhalaCodeDesktopFleetAccount = typeof RpcFleetAccount.Type")
    expect(rpc).toContain("capacity: S.NullOr(RpcFleetCapacity)")
    expect(handlers).toContain("capacity: account.capacity")
    expect(fleetPanel).toContain("isDisplayOnlyDefaultAccountRef")
    expect(fleetPanel).toContain("accountCapacityLabel")
    expect(fleetPanel).toContain("fleetTokenRateLabel")
    expect(fleetPanel).toContain("buildKhalaFleetWorkerCards")
    expect(fleetPanel).toContain("renderDelegateRunner")
    expect(fleetPanel).toContain("renderOptimizationRunner")
    expect(fleetPanel).toContain("defaultKhalaFleetDelegationActiveParameters")
    expect(fleetPanel).toContain("Optimize delegation policy")
    expect(fleetPanel).toContain("Load demo proof")
    expect(fleetPanel).toContain("khala-code-delegation-gepa")
    expect(fleetPanel).toContain("startDelegationOptimization")
    expect(fleetPanel).toContain("loadGymDemoProof")
    expect(fleetPanel).toContain("Real-work mode requires repo, commit, and verify pins before dispatch")
    expect(fleetPanel).toContain("iconElement")
    expect(fleetPanel).toContain('"Trash"')
    expect(fleetPanel).toContain('"Play"')
    expect(fleetPanel).toContain("Codex session boundaries")
    expect(fleetPanel).toContain("primary user plus isolated workers")
    expect(fleetPanel).toContain("Worker Codex accounts")
    expect(fleetPanel).toContain("isolated worker Codex home")
    expect(fleetPanel).toContain("primary user Codex session")
    expect(fleetPanel).toContain("sessionRoleLabel")
    expect(fleetPanel).toContain("homeRoleLabel")
    expect(fleetPanel).toContain("queuePolicyLabel")
    expect(fleetPanel).toContain('"transcript"')
    expect(fleetPanel).toContain('"closeout"')
    expect(fleetPanel).toContain('appendChip(details, "routing", "default slot")')
    expect(fleetPanel).toContain('appendChip(details, "slots", accountCapacityLabel(account.capacity))')
    expect(fleetPanel).toContain('"busy"')
    expect(fleetPanel).toContain("account.capacity.busy")
    expect(fleetPanel).toContain('"queued"')
    expect(fleetPanel).toContain("account.capacity.queued")
    expect(fleetPanel).toContain('appendChip(pylonDetails, "token rate"')
    expect(fleetPanel).toContain('appendChip(chips, "tokens"')
    expect(css).toContain(".khala-fleet-card-details")
    expect(css).toContain(".khala-fleet-session")
    expect(css).toContain(".khala-fleet-delegate")
    expect(css).toContain(".khala-fleet-delegate-steps")
    expect(css).toContain(".khala-fleet-optimization")
    expect(css).toContain(".khala-fleet-parameter-readout")
    expect(main).toContain("loadGymDemoOptimization")
    expect(main).toContain("startDelegationOptimization")
    expect(main).toContain("lifecycleNdjson: fleetLifecycleLines.iterable")
    expect(`${rpc}\n${handlers}\n${fleetPanel}\n${main}`).toContain("codexFleetDelegateRun")
    expect(`${rpc}\n${handlers}\n${fleetPanel}\n${main}`).not.toMatch(
      /raw[_-]?(prompt|trace).*Projected:\s*true|localPathsProjected:\s*true|providerPayloadProjected:\s*true/i,
    )
  })

  test("renders Fleet status with a board graph and run timeline mount", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const panel = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    const foldkitRuntime = await Bun.file(new URL("../src/ui/foldkit/runtime.ts", import.meta.url)).text()
    const projection = await Bun.file(new URL("../src/ui/fleet-board-projection.ts", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/fleet-board-renderer.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("mountFleetPanel")
    expect(main).not.toContain("foldkitDemoPanelEl")
    expect(panel).toContain("mountKhalaCodeFleetCockpit")
    expect(panel).toContain("HostFleetCockpitStatus")
    expect(foldkitRuntime).toContain("embedKhalaCodeFleetCockpitProgram")
    expect(panel).toContain("appendFleetBoard(container, data)")
    expect(panel).toContain("buildKhalaFleetBoardProjection")
    expect(panel).toContain("renderKhalaFleetBoardHtml")
    expect(projection).toContain("openagents.khala_code.fleet_board_projection.v0")
    expect(projection).toContain("main-codex-session")
    expect(projection).toContain("caveat.khala_fleet.main_session_not_worker")
    expect(renderer).toContain("Fleet board graph and run timeline")
    expect(css).toContain(".khala-fleet-board-summary")
    expect(css).toContain(".khala-fleet-timeline-event")
  })

  test("keeps Fleet sidebar as hotkey-only chrome while projecting counts for state", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('label: "Fleet"')
    expect(sidebar).toContain('value: "fleet"')
    expect(sidebar).toContain('icon: "Robot"')
    expect(sidebar).toContain('actionId: "action_bar.slot_2"')
    expect(sidebar).toContain('hotkey: "2"')
    expect(sidebar).toContain('label: "Forum"')
    expect(sidebar).toContain('value: "forum"')
    expect(sidebar).toContain('icon: "BookOpen"')
    expect(sidebar).toContain('actionId: "action_bar.slot_3"')
    expect(sidebar).toContain('hotkey: "3"')
    expect(sidebar).toContain('label: "Settings"')
    expect(sidebar).toContain('label: "Inbox"')
    expect(sidebar).toContain('actionId: "action_bar.slot_4"')
    expect(sidebar).toContain('hotkey: "4"')
    expect(sidebar).toContain('actionId: "action_bar.slot_5"')
    expect(sidebar).toContain('hotkey: "5"')
    expect(sidebar).toContain("options.onActivate?.(slot.value)")
    expect(main).toContain("mountKhalaCodeSidebar(sidebarNavRoot, {")
    expect(main).toContain("sidebar.setFleetCounts(projectKhalaCodeSidebarFleetCounts(await controls.codexFleetStatus()))")
    expect(sidebar).toContain("projectKhalaCodeSidebarFleetCounts")
    expect(sidebar).not.toContain("data-khala-code-fleet-counts")
    expect(sidebar).not.toContain("window.setInterval(() => void refreshFleetSummary(), 7000)")
    expect(sidebar).not.toContain("button.dataset.fleetSession = session.ref")
    expect(sidebar).not.toContain('"idle"')
    expect(css).not.toContain(".khala-code-hotbar-fleet-counts")
    expect(css).not.toContain(".khala-code-fleet-session")
    expect(css).not.toContain(".khala-code-fleet-empty")
    expect(css).toContain("--khala-code-hotbar-titlebar-clearance: 2.75rem")
    expect(css).toContain("env(safe-area-inset-top, 0px)")
    expect(css).toContain("width: 1.52rem")
    expect(css).toContain("height: 1.52rem")

    const counts = projectKhalaCodeSidebarFleetCounts({
      ok: true,
      observedAt: "2026-07-01T18:00:00.000Z",
      pylon: {
        message: "online",
        pylonRef: "pylon.public.fleet",
        status: "online",
      },
      availableCodexAssignments: 2,
      maxCodexAssignments: 4,
      tokenRate: {
        activeAdjustedTokensPerMinute: null,
        completedStatus: "not_measured",
        completedTokenRows: null,
        completedTokensPerMinute: null,
        inFlightTokens: null,
        inFlightTokensPerMinute: null,
        source: "unavailable",
        unavailableReason: null,
      },
      accounts: [
        {
          accountKey: null,
          accountRef: "codex",
          capacity: null,
          email: null,
          provider: "codex",
          quotaState: null,
          readiness: "ready",
        },
        {
          accountKey: null,
          accountRef: "codex-2",
          capacity: null,
          email: null,
          provider: "codex",
          quotaState: null,
          readiness: "credentials_missing",
        },
      ],
      activeAssignments: [{
        assignmentRef: "assignment.public.sidebar",
        blockerRefs: ["blocker.public.worker.merge_conflict_wave"],
        elapsedMs: null,
        issueRef: "github.issue.openagents.7843",
        runRef: "fleet.run.public.sidebar",
        tokenRate: {
          source: "token_usage_events",
          status: "pending",
          tokenCountKind: null,
          tokens: null,
          tokensPerMinute: null,
        },
        updatedAt: "2026-07-01T18:01:00.000Z",
      }],
      processes: [],
    })
    expect(counts).toEqual({
      accountsReady: 1,
      workersActive: 1,
      slotsFree: 2,
      flags: 2,
    })

    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
    try {
      const container = document.createElement("div")
      mountKhalaCodeSidebar(container, { fleetCounts: counts, selectedValue: "fleet" })
      expect(container.textContent).not.toContain("1 work / 2 free / 2 flag")
      expect(container.textContent).not.toContain("work")
      expect(container.textContent).not.toContain("free")
      expect(container.textContent).not.toContain("flag")
      expect(container.textContent).not.toContain("acct")
      expect(container.querySelector<HTMLElement>("[data-khala-code-fleet-counts]")).toBeNull()
    } finally {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    }
  })

  test("keeps Gym proof pane available without a top-level Gym screen", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const pane = await Bun.file(new URL("../src/ui/gym-pane.ts", import.meta.url)).text()
    const loader = await Bun.file(new URL("../src/ui/gym-proof-loader.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).not.toContain('value: "gym"')
    expect(sidebar).not.toContain('icon: "Dumbbell"')
    expect(html).toContain('id="gym-panel"')
    expect(html).toContain('aria-label="Gym proof"')
    expect(html).not.toContain('aria-label="Gym delegation graph"')
    expect(main).toContain("mountGymPane")
    expect(main).toContain("gymPaneStateFromLocation")
    expect(main).toContain("loadGymProof")
    expect(main).toContain("loadGymDemoProof")
    expect(main).not.toContain('const showGym = value === "gym"')
    expect(main).not.toContain("gymPanel?.setVisible(showGym)")
    expect(pane).toContain('S.Literal("empty")')
    expect(pane).toContain('S.Literal("loaded")')
    expect(pane).toContain('S.Literal("blocked")')
    expect(pane).toContain('phase: "empty"')
    expect(pane).toContain("No Gym proof loaded.")
    expect(pane).toContain("Active delegation parameters")
    expect(loader).toContain("khalaCodeGymDemoBridgeProof")
    expect(loader).toContain("defaultKhalaFleetDelegationActiveParameters")
    expect(loader).toContain("gymPaneStateFromOptimizationRun")
    expect(loader).toContain("bridgeProofFromOptimizationProjection")
    expect(loader).toContain("openagents.khala.fleet_delegation.parameters.v0")
    expect(loader).toContain("decisionGrade: false")
    expect(loader).toContain('proof === "fixture"')
    expect(main).toContain("loadGymDemoOptimization")
    expect(main).toContain("startDelegationOptimization")
    expect(css).toContain(".khala-code-gym")
    expect(css).toContain(".khala-gym-parameters")
    expect(css).toContain(".khala-gym-detail-grid")
    expect(css).toContain(".khala-gym-state[data-state=\"blocked\"]")
    expect(`${html}\n${sidebar}\n${main}\n${pane}\n${loader}`).not.toMatch(
      /\/Users\/|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace)|sk-[a-z0-9]/i,
    )
  })

  test("wires the transcript 245 Part 2 recording smoke through UI actions", async () => {
    const readme = await Bun.file(new URL("../README.md", import.meta.url)).text()
    const packageJson = await Bun.file(new URL("../package.json", import.meta.url)).text()
    const smoke = await Bun.file(new URL("../scripts/part2-ui-recording-smoke.ts", import.meta.url)).text()

    expect(packageJson).toContain('"smoke:part2-ui"')
    expect(readme).toContain("bun run smoke:part2-ui")
    expect(readme).toContain("shared console/pageerror oracle")
    expect(smoke).toContain("PART2_UI_RECORDING_SMOKE_HARNESS")
    expect(smoke).toContain("codexFleetDelegateRun")
    expect(smoke).toContain("Optimize delegation policy")
    expect(smoke).toContain("Run delegate")
    expect(smoke).toContain("candidate manifest")
    expect(smoke).toContain("Gym ingest")
    expect(smoke).toContain("part2UiUnsafeTextPattern")
    expect(smoke).toContain("assertKhalaCodePublicSafeValue")
    expect(smoke).toContain("assertKhalaCodePagePublicSafe")
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
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()

    expect(main).toContain("initialKhalaCodeMainShellModel")
    expect(main).toContain("setShellMessages([])")
    expect(main).not.toContain("let messages")
    expect(model).toContain("messages: []")
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

  test("documents Khala tools as supplemental by default and legacy by flag", async () => {
    const readme = await Bun.file(new URL("../README.md", import.meta.url)).text()
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()

    expect(readme).toContain("default desktop `toolCatalog()`")
    expect(readme).toContain("only Khala's supplemental swarm/Pylon tools")
    expect(readme).toContain("npm install -g @openai/codex")
    expect(readme).toContain("codex app-server --stdio")
    expect(readme).toContain("Product Boundary")
    expect(readme).toContain("rather than rebuilding Codex Core behavior in TypeScript")
    expect(readme).toContain("2026-07-01-codex-required-product-positioning.md")
    expect(readme).toContain("OpenAgentsInc/openagents#7780")
    expect(readme).toContain("KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime")
    expect(readme).toContain("legacy turns are labeled")
    expect(readme).toContain("in the transcript")
    expect(readme).toContain("Codex owns those local coding capabilities")
    expect(handlers).toContain("Legacy Khala native runtime handled this turn")
    expect(readme).not.toContain("The current Khala tool presets remain available only for legacy/fallback")
  })

  test("wires the Codex settings panel to app-server config and catalog RPCs", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const panel = await Bun.file(new URL("../src/ui/codex-settings-panel.ts", import.meta.url)).text()
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain('value: "settings"')
    expect(sidebar).toContain('icon: "Settings"')
    expect(html).toContain('aria-label="Codex settings"')
    expect(main).toContain("mountCodexSettingsPanel")
    expect(main).toContain("codexSettingsRead")
    expect(main).toContain("codexConfigValueWrite")
    expect(main).toContain("modelRoleRegistryRead")
    expect(main).toContain("modelRoleRegistryWrite")
    expect(main).toContain('const showSettings = activeValue === "settings"')
    expect(main).toContain("settingsPanel?.setVisible(showSettings)")
    expect(panel).toContain("model_reasoning_effort")
    expect(panel).toContain("Model Roles")
    expect(panel).toContain("KHALA_CODE_MODEL_ROLE_ORDER")
    expect(panel).toContain("service_tier")
    expect(panel).toContain("default_permissions")
    expect(panel).toContain("Primary Codex app-server session")
    expect(panel).toContain("Harness Boundary")
    expect(panel).toContain("Fleet workers")
    expect(panel).toContain("Experimental fallback only")
    expect(handlers).toContain('"model/list"')
    expect(handlers).toContain('"modelProvider/capabilities/read"')
    expect(handlers).toContain('"permissionProfile/list"')
    expect(handlers).toContain('"config/read"')
    expect(handlers).toContain('"config/value/write"')
    expect(handlers).toContain('"account/usage/read"')
    expect(css).toContain(".khala-code-settings")
    expect(css).toContain(".khala-settings-select")
  })

  test("wires the Codex thread sidebar to app-server thread lifecycle RPCs", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()
    const panel = await Bun.file(new URL("../src/ui/codex-thread-sidebar.ts", import.meta.url)).text()
    const runtime = await Bun.file(new URL("../src/bun/codex-app-server-chat-runtime.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html).toContain('aria-label="Codex threads"')
    expect(main).toContain("mountCodexThreadSidebar")
    expect(main).toContain("khala-code-desktop.active-thread-id.v1")
    expect(main).toContain("localStorage.removeItem(activeThreadIdStorageKey)")
    expect(model).toContain("activeCodexThreadId: null")
    expect(main).toContain("THREAD_MESSAGE_CACHE_LIMIT")
    expect(main).toContain("THREAD_PREFETCH_LIMIT")
    expect(main).toContain("threadSwitchPerformance")
    expect(main).toContain("qaMetrics: qaMetricsSnapshot")
    expect(main).toContain("void rpc.request.qaMetricSample(sample)")
    expect(main).toContain("THREAD_SWITCH_FULL_HYDRATION_TIMEOUT_MS")
    expect(main).toContain("turn_start.latency_ms")
    expect(main).toContain("turn_start.first_event_ms")
    expect(main).toContain("first_render.ms")
    expect(main).toContain("startup.interactive_ms")
    expect(main).toContain("panel.open_ms")
    expect(main).toContain("composer.keystroke_echo_ms")
    expect(main).toContain("sse.event_to_ui_ms")
    expect(main).toContain("transcript.scroll_dropped_frames_pct")
    expect(main).toContain("cache.hit")
    expect(main).toContain("markVisibleChatTurnEventPaint")
    expect(main).toContain("startPreviewBridgeEvents")
    expect(main).toContain('eventSource.addEventListener("chatTurnEvent"')
    expect(main).toContain("KhalaCodeDesktopChatTurnEventSchema")
    expect(main).toContain("sampleTranscriptScrollDroppedFrames")
    expect(main).toContain("beginCodexThreadSwitch")
    expect(main).toContain("prefetchRecentThreadMessages")
    expect(main).toContain("activeTurnIds.clear()")
    expect(main).not.toContain("threadSidebar?.upsertDraftThread")
    expect(main).not.toContain("threadSidebar?.clearDraftThread")
    expect(main).toContain("codexThreadArchive")
    expect(main).toContain("codexThreadDelete")
    expect(main).toContain("codexThreadFork")
    expect(main).toContain("codexThreadRead")
    expect(main).toContain("codexThreadRename")
    expect(main).toContain("codexThreadUnarchive")
    expect(main).toContain("sessionCatalog")
    expect(main).toContain("sessionCatalogEntryToThreadSummary")
    expect(main).toContain("activateCodexThread")
    expect(main).toContain("const optimisticMessages = cached === null")
    expect(main).toContain("optimisticMessageCount: optimisticMessages?.length ?? 0")
    expect(main).toContain("const hydratedVisibleMessages = shellModel().messages")
    expect(main).toContain("visibleMessages: hydratedVisibleMessages")
    expect(main).toContain("response.backend.threadId")
    expect(main).toContain("recentThreadHotkeyIndexForEvent")
    expect(main).toContain("recentThreadCycleDirectionForEvent")
    expect(main).toContain("createKeyHoldTracker")
    expect(main).toContain("threadSidebar.selectRecentThread(recentThreadIndex)")
    expect(main).toContain("threadSidebar.selectAdjacentRecentThread(recentThreadCycleDirection)")
    expect(main).toContain("codexThreadRead({")
    expect(main).toContain("onThreadSelectionStarted: beginCodexThreadSwitch")
    expect(main).toContain("isThreadStreaming: threadId => shellModel().activeCodexThreadId === threadId && shellModel().pendingTurn")
    expect(main).toContain("threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)")
    expect(main).toContain(
      "const request: KhalaCodeDesktopChatTurnRequest = {\n      ...(imageAttachments.length === 0 ? {} : { attachments: imageAttachments }),\n      messages: shellModel().messages,\n      sessionId,\n      ...(activeThreadId === null ? { startNewThread: true } : { threadId: activeThreadId }),\n      turnId,\n    }",
    )
    expect(main).toContain("const imageAttachments = await imageAttachmentsForSubmit(attachments)")
    expect(panel).toContain("Search Codex threads")
    expect(panel).toContain("let searchOpen = false")
    expect(panel).toContain("const toggleSearch = (): void =>")
    expect(panel).toContain('const searchToggle = el("button", "khala-thread-sidebar-search-toggle")')
    expect(panel).toContain('searchToggle.setAttribute("aria-expanded", searchOpen ? "true" : "false")')
    expect(panel).toContain('searchToggle.setAttribute("aria-controls", "khala-thread-sidebar-search-flyout")')
    expect(panel).toContain('sidebarIcon("Search", "Search threads")')
    expect(panel).toContain('const searchFlyout = el("div", "khala-thread-sidebar-search-flyout")')
    expect(panel).toContain('search.name = "threadSearch"')
    expect(panel).toContain("searchTerm = search.value.trim()")
    expect(panel).toContain("searchTerm = \"\"\n      void refresh()")
    expect(panel).toContain("options.listThreads({ archived: false, searchTerm })")
    expect(panel).not.toContain("let archived = false")
    expect(panel).not.toContain("archiveInput")
    expect(panel).not.toContain("khala-thread-sidebar-toggle")
    expect(panel).not.toContain("Archived")
    expect(panel).toContain('from "@openagentsinc/ui/menu-dom"')
    expect(panel).toContain("createBasecoatContextMenu")
    expect(panel).not.toContain("readonly clearDraftThread")
    expect(panel).not.toContain("readonly upsertDraftThread")
    expect(panel).toContain("readonly isThreadStreaming?: (threadId: string) => boolean")
    expect(panel).toContain("readonly onNewThreadRequested: () => void")
    expect(panel).toContain("readonly onThreadSelectionStarted?:")
    expect(panel).toContain("activeThreadId = null\n    options.onNewThreadRequested()\n    render()")
    expect(panel).not.toContain("readonly startThread")
    expect(panel).not.toContain("readonly onThreadStarted")
    expect(panel).not.toContain("const draftThreadSummary =")
    expect(panel).not.toContain("const dataWithDraftThreads =")
    expect(panel).not.toContain('label: "Drafts"')
    expect(panel).not.toContain('statusLabel: "draft"')
    expect(panel).toContain("options.resumeThread(threadId)")
    expect(panel).toContain("readonly selectRecentThread")
    expect(panel).toContain("readonly selectAdjacentRecentThread")
    expect(panel).toContain("let selectionSequence = 0")
    expect(panel).toContain("options.onThreadSelectionStarted?.({ selectionId, source, threadId })")
    expect(panel).toContain('options.listThreads({ archived: false, searchTerm: "" })')
    expect(panel).toContain("recentThreadCycleIndex")
    expect(panel).toContain("recentThreadsForHotkeys")
    expect(panel).toContain("options.forkThread(thread.id)")
    expect(panel).toContain("let renamingThreadId: string | null = null")
    expect(panel).toContain("const beginRename = (thread: KhalaCodeDesktopCodexThreadSummary): void =>")
    expect(panel).toContain("onSelect: () => beginRename(thread)")
    expect(panel).toContain("khala-thread-sidebar-rename-form")
    expect(panel).toContain('renameInput.name = "threadName"')
    expect(panel).toContain('renameInput.setAttribute("aria-label", "Thread name")')
    expect(panel).toContain('save.type = "submit"')
    expect(panel).toContain('cancel.type = "button"')
    expect(panel).toContain('if (event.key !== "Escape") return')
    expect(panel).toContain("submitRename(thread, renameInput.value)")
    expect(panel).toContain("renamingThreadId === thread.id ? threadRenameForm(thread) : row")
    expect(panel).not.toContain("prompt(")
    expect(panel).toContain('label: "Copy session ID"')
    expect(panel).toContain('icon: "Copy"')
    expect(panel).toContain("thread.sessionId ?? thread.id")
    expect(panel).toContain("navigator.clipboard?.writeText(sessionId)")
    expect(panel).toContain('item.addEventListener("contextmenu"')
    expect(panel).toContain("threadMenu.openAt")
    expect(panel).toContain("const isThreadStreaming =")
    expect(panel).not.toContain('thread.status === "active"')
    expect(panel).toContain("options.isThreadStreaming?.(threadId) === true")
    expect(panel).toContain("const threadStreamingIndicator =")
    expect(panel).toContain('time.dataset.streaming = "true"')
    expect(panel).toContain("threadTimeContent(thread, options)")
    expect(panel).toContain("const dataForState =")
    expect(panel).toContain("let refreshSequence = 0")
    expect(panel).toContain("const previousData = dataForState(state)")
    expect(panel).toContain("? { phase: \"loading\" }")
    expect(panel).toContain(": { phase: \"loading\", data: previousData }")
    expect(panel).toContain("if (requestSequence !== refreshSequence) return")
    expect(panel).toContain("void refresh()")
    expect(panel).toContain("khala-thread-sidebar-item-row")
    expect(panel).not.toContain("khala-thread-sidebar-menu-button")
    expect(panel).not.toContain("khala-thread-sidebar-menu-summary")
    expect(panel).not.toContain('label: "Thread"')
    expect(panel).not.toContain('label: "Lifecycle"')
    expect(panel).not.toContain('description: "Set display name"')
    expect(panel).not.toContain('description: "Create branch thread"')
    expect(panel).not.toContain('description: "Move out of active threads"')
    expect(panel).not.toContain("khala-thread-sidebar-actions")
    expect(runtime).toContain('"thread/list"')
    expect(runtime).toContain('"thread/read"')
    expect(runtime).toContain('"thread/fork"')
    expect(runtime).toContain('"thread/archive"')
    expect(runtime).toContain('"thread/delete"')
    expect(runtime).toContain('"thread/unarchive"')
    expect(runtime).toContain('"thread/name/set"')
    expect(runtime).toContain("messagesFromThread")
    expect(css).toContain(".khala-code-thread-sidebar")
    expect(css).toContain(".khala-thread-sidebar-item")
    expect(css).toContain(".khala-thread-sidebar-item-row")
    expect(css).toContain(".khala-thread-sidebar-rename-form")
    expect(css).toContain(".khala-thread-sidebar-rename-input")
    expect(css).toContain(".khala-thread-sidebar-rename-action")
    expect(css).toContain(".khala-thread-sidebar-item-time[data-streaming=\"true\"]")
    expect(css).toContain(".khala-thread-sidebar-item-spinner")
    expect(css).toContain("@keyframes khala-thread-sidebar-item-spinner")
    expect(css).not.toContain(".khala-thread-sidebar-menu-button")
    expect(css).toContain("height: 2rem")
    expect(css).toContain(".khala-thread-sidebar-header-actions")
    expect(css).toContain(".khala-thread-sidebar-search-toggle")
    expect(css).toContain(".khala-thread-sidebar-search-flyout")
    expect(css).toContain("border: 0;")
    expect(css).not.toContain(".khala-thread-sidebar-controls")
    expect(css).not.toContain(".khala-thread-sidebar-toggle")
    expect(css).not.toContain(".khala-thread-sidebar-menu-summary")
    expect(css).not.toContain(".khala-thread-sidebar-actions")
  })

  test("registers QA metrics RPC and budget definitions", async () => {
    const rpc = await Bun.file(new URL("../src/shared/rpc.ts", import.meta.url)).text()
    const handlers = await Bun.file(new URL("../src/bun/rpc-handlers.ts", import.meta.url)).text()
    const index = await Bun.file(new URL("../src/bun/index.ts", import.meta.url)).text()
    const metrics = await Bun.file(new URL("../src/shared/qa-metrics.ts", import.meta.url)).text()
    const previewPolicy = await Bun.file(new URL("../src/bun/preview-rpc-policy.ts", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

    expect(rpc).toContain("qaMetricSample")
    expect(rpc).toContain("RpcQaMetricSampleResult")
    expect(rpc).toContain("qaMetrics")
    expect(rpc).toContain("RpcQaMetricsSnapshot")
    expect(handlers).toContain("recordQaMetricSample")
    expect(handlers).toContain("emptyKhalaCodeQaMetricsSnapshot")
    expect(index).toContain("recordQaMetricSample")
    expect(index).toContain("qaMetrics: qaMetricsSnapshot")
    expect(previewPolicy).toContain('"qaMetricSample"')
    expect(previewPolicy).toContain('"qaMetrics"')
    expect(main).toContain('>("qaMetricSample", sample)')
    expect(main).toContain('>("qaMetrics")')
    expect(main).toContain('pushQaMetricSample("thread_switch.rpc_ms"')
    expect(main).toContain('pushQaMetricSample("turn_start.first_event_ms"')
    expect(main).toContain('pushQaMetricSample("sse.event_to_ui_ms"')
    expect(main).toContain('"transcript.scroll_dropped_frames_pct"')
    expect(main).toContain('markQaTimer("composer.keystroke_echo_ms"')
    expect(main).toContain('markQaTimer("startup.interactive_ms"')
    expect(metrics).toContain("budget.khala_code.startup_interactive.v1")
    expect(metrics).toContain("budget.khala_code.thread_switch.optimistic.v1")
    expect(metrics).toContain("budget.khala_code.thread_switch.full.v1")
    expect(metrics).toContain("budget.khala_code.turn_start.first_event.v1")
    expect(metrics).toContain("budget.khala_code.composer.keystroke_echo.p95.v1")
    expect(metrics).toContain("budget.khala_code.panel.open.v1")
    expect(metrics).toContain("budget.khala_code.sse.event_to_ui.p95.v1")
    expect(metrics).toContain("budget.khala_code.transcript.scroll_dropped_frames.v1")
    expect(metrics).toContain("budget.khala_code.app_server.spawn_ready.v1")
    expect(metrics).toContain("budget.khala_code.cockpit_render.50_cards.v1")
    expect(metrics).toContain("budget.khala_code.lifecycle_event_to_card.p95.v1")
    expect(metrics).toContain("budget.khala_code.supervisor_tick.25_target.v1")
  })

  test("keeps Khala Code to one sidebar shell", async () => {
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(sidebar).toContain("KHALA_CODE_HOTBAR_SLOTS")
    expect(html).toContain('class="khala-code-sidebar-shell"')
    expect(html).toContain('id="sidebar-nav-root"')
    expect(html).toContain('<section\n          id="thread-sidebar"')
    expect(main).toContain('document.getElementById("sidebar-nav-root")')
    expect(css).toContain("grid-template-columns: var(--sidebar-width) minmax(0, 1fr)")
    expect(css).toContain(".khala-code-shell > :is(")
    expect(css).toContain(".khala-code-sidebar-shell")
    expect(css).toContain(".khala-code-shell:has(.khala-code-thread-sidebar:not([hidden]))")
    expect(css).toContain(
      "--sidebar-width: var(--sidebar-expanded-width);",
    )
    expect(css).toContain(".khala-code-sidebar-shell:has(.khala-code-thread-sidebar:not([hidden]))")
    expect(css).toContain("grid-template-columns: var(--sidebar-rail-width) minmax(0, 1fr);")
    expect(css).toContain(".khala-code-thread-sidebar {\n  grid-column: 1;")
    expect(css).toContain("grid-column: 2;")
    expect(css).not.toContain("--thread-sidebar-width")
    expect(css).not.toContain("grid-template-columns: var(--sidebar-width) var(--thread-sidebar-width) minmax(0, 1fr)")
    expect(css).not.toContain("left: var(--sidebar-width)")
    expect(css).not.toContain("margin-left: calc(var(--sidebar-width) + var(--thread-sidebar-width))")
  })

  test("keeps the composer footer controls in a clean inline strip", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const activeCss = css.replace(/\/\*[\s\S]*?\*\//g, "")
    const composerFooterRule =
      activeCss.match(/\.khala-code-composer \.oa-ai-command-composer-footer \{[\s\S]*?\}/)?.[0] ?? ""

    expect(html).not.toContain('id="preview-button"')
    expect(html).not.toContain('id="resize-button"')
    expect(html).not.toContain('data-oa-command-composer-control="Preview"')
    expect(html).not.toContain("data-oa-command-composer-resize")
    expect(composerFooterRule).toContain("grid-template-columns: minmax(0, 1fr) 32px")
    expect(composerFooterRule).not.toContain("grid-template-columns: minmax(0, 1fr) auto auto")
    expect(composerFooterRule).not.toContain("grid-template-columns: minmax(0, 1fr) minmax(0, auto) 32px 32px")
    expect(css).toContain(".khala-code-composer .oa-ai-command-composer-submit-label")
    expect(main).not.toContain("previewButton")
    expect(main).not.toContain("resizeButton")
    expect(main).not.toContain("composerExpanded")
    expect(main).not.toContain("togglePreview")
  })

  test("uses the shared Apps SDK icon catalog for composer and sidebar chrome", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const threadSidebar = await Bun.file(new URL("../src/ui/codex-thread-sidebar.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html).toContain('data-oa-command-composer-icon="attach"')
    expect(html).toContain('data-oa-command-composer-icon="send"')
    expect(html).not.toContain('aria-hidden="true">+</span>')
    expect(html).not.toContain('aria-hidden="true">^</span>')
    expect(main).toContain('from "@openagentsinc/ui/icon-dom"')
    expect(main).toContain("composerIconCatalog")
    expect(main).toContain('send: "ArrowUp"')
    expect(main).toContain('stop: "Stop"')
    expect(sidebar).toContain('from "@openagentsinc/ui/icon-dom"')
    expect(sidebar).toContain('from "@openagentsinc/ui/icon"')
    expect(sidebar).toContain('iconElement(slot.icon')
    expect(threadSidebar).toContain('from "@openagentsinc/ui/icon-dom"')
    expect(threadSidebar).toContain('sidebarIcon("Search", "Search threads")')
    expect(threadSidebar).toContain('sidebarIcon("Plus", "New thread")')
    expect(threadSidebar).toContain('icon: "Pencil"')
    expect(threadSidebar).not.toContain('sidebarIcon("DotsVerticalMoreMenu", "Thread actions")')
    expect(css).toContain(".khala-code-hotbar-icon")
    expect(css).toContain(".khala-thread-sidebar-icon")
    expect(css).toContain("--sidebar-rail-width: 5.25rem")
  })

  test("adapts the StarCraft command-card model into a vertical hotbar with shortcuts", async () => {
    const starcraft = await Bun.file(new URL("../../../docs/design/starcraft.md", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/sidebar.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(starcraft).toContain("command-card buttons need both pointer handlers and keyboard bindings")
    expect(starcraft).toContain("Use hotkey labels")
    expect(sidebar).toContain("export const KHALA_CODE_HOTBAR_SLOTS")
    expect(sidebar).toContain('actionId: "action_bar.slot_1"')
    expect(sidebar).toContain('actionId: "action_bar.slot_3"')
    expect(sidebar).toContain('actionId: "action_bar.slot_4"')
    expect(sidebar).toContain('actionId: "action_bar.slot_5"')
    expect(sidebar).toContain('button.dataset.hotbarAction = slot.actionId')
    expect(sidebar).toContain('button.dataset.hotkey = slot.hotkey')
    expect(sidebar).toContain("const hotbarShortcut = (): HotbarShortcut")
    expect(sidebar).toContain('ariaModifier: "Alt"')
    expect(sidebar).toContain('label: "Option"')
    expect(sidebar).toContain('label: "Alt"')
    expect(sidebar).toContain('visiblePrefix: "⌥"')
    expect(sidebar).toContain('visiblePrefix: "Alt+"')
    expect(sidebar).toContain('modifierKey: "altKey"')
    expect(sidebar).toContain('"aria-keyshortcuts"')
    expect(sidebar).toContain("`${shortcut.ariaModifier}+${slot.hotkey}`")
    expect(sidebar).toContain("`${shortcut.visiblePrefix}${slot.hotkey}`")
    expect(sidebar).toContain('window.addEventListener("keydown"')
    expect(sidebar).toContain("const explicitHotkey =")
    expect(sidebar).toContain("event[shortcut.modifierKey]")
    expect(sidebar).not.toContain("const ambientHotkey =")
    expect(sidebar).not.toContain("!editable")
    expect(sidebar).not.toContain("khala-code-hotbar-header")
    expect(css).toContain(".khala-code-hotbar")
    expect(css).toContain("--khala-code-hotbar-titlebar-clearance: 2.75rem")
    expect(css).toContain("env(safe-area-inset-top, 0px)")
    expect(css).toContain(".khala-code-hotbar-slot")
    expect(css).toContain(".khala-code-hotbar-key")
    expect(css).toContain(".khala-code-hotbar-slot[data-active=\"true\"]")
    expect(css).not.toContain(".khala-code-hotbar-header")
  })

  test("renders full hotbar shortcut labels and activates slots with the explicit shortcut", async () => {
    const { Window } = await import("happy-dom")
    const { mountKhalaCodeSidebar } = await import("../src/ui/sidebar")
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document
    const activated: string[] = []
    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
    try {
      const container = document.createElement("div")
      mountKhalaCodeSidebar(container, {
        selectedValue: "chat",
        onActivate: value => activated.push(value),
      })
      expect(container.textContent).toMatch(/(?:Alt\+|⌥)1/)
      window.dispatchEvent(new window.KeyboardEvent("keydown", {
        altKey: true,
        key: "2",
      }))
      expect(activated).toEqual(["fleet"])
      expect(container.querySelector('[data-khala-code-hotbar-value="fleet"]')?.getAttribute("data-active")).toBe("true")
    } finally {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    }
  })

  test("renders a compact StarCraft follow-up composer structure", async () => {
    const html = await Bun.file(new URL("../src/ui/index.html", import.meta.url)).text()
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(html).toContain('rows="2"')
    expect(html).toContain('id="composer-follow-up-queue"')
    expect(html).toContain('placeholder="Ask for follow-up changes"')
    expect(main).toContain("queueFollowUpDraft")
    expect(main).toContain("steerFollowUpDraft")
    expect(main).toContain('setButtonIcon(attachButton, "plus")')
    expect(css).toContain("max-width: min(48rem, calc(100vw - var(--sidebar-width) - 24px))")
    expect(css).toContain("--oa-command-composer-height: 7rem")
    expect(css).toContain("border-radius: 3px")
    expect(css).toContain("padding: 10px 10px 8px")
    expect(css).toContain("min-height: 3.45rem")
    expect(css).toContain(".khala-code-composer-follow-up-queue")
    expect(css).toContain("#composer-input")
    expect(css).toContain("padding: 0")
    expect(css).not.toContain("--oa-command-composer-height: 12.25rem")
    expect(css).not.toContain("border-radius: 28px")
    expect(css).not.toContain("padding: 30px 26px 16px")
    expect(css).not.toContain("min-height: 5.9rem")
    expect(css).not.toContain("min-height: 4.25rem")
    expect(css).not.toContain("min-height: 9rem")
    expect(css).not.toContain("padding: 16px 46px 48px 0")
  })

  test("keeps transcript scrolling full width while messages stay narrow and composer spans the dock", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(css).toContain(".message-list")
    expect(css).toContain("max-width: none")
    expect(css).toContain("scrollbar-gutter: stable")
    expect(css).toContain("margin-left: max(4px, calc((100% - 768px) / 2))")
    expect(css).toContain("margin-right: max(4px, calc((100% - 768px) / 2))")
    expect(css).toContain(".khala-code-composer")
    expect(css).toContain("max-width: min(48rem, calc(100vw - var(--sidebar-width) - 24px))")
    expect(css).not.toContain("max-width: min(92rem, calc(100vw - var(--sidebar-width) - 28px))")
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
    expect(css).toContain(".message-bubble--user .message-body")
    expect(css).toContain("color-mix(in srgb, var(--oa-color-khala-energy-cyan) 6%, transparent)")
    expect(css).toContain("color-mix(in srgb, var(--oa-color-khala-surface-active) 78%, var(--oa-color-khala-void))")
    expect(css).not.toContain(".message-bubble--user .message-body {\n  border:")
    expect(css).toContain(".message-bubble--user .message-prose {\n  text-align: left;\n}")
    expect(css).not.toContain("  align-self: stretch;\n}")
    expect(css).not.toContain("  justify-items: end;\n}")
    expect(css).not.toContain(".message-bubble--user .message-prose {\n  text-align: right;\n}")
  })

  test("keeps the composer input available while a turn is pending", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("sendButton.disabled = !shellModel().pendingTurn && !canSubmitComposer()")
    expect(main).toContain('sendButton.type = shellModel().pendingTurn ? "button" : "submit"')
    expect(main).toContain("stopActiveTurn")
    expect(main).toContain("queueFollowUpDraft(draftText)")
    expect(main).not.toContain("composerInput.disabled = pendingTurn")
    expect(main).toContain("requestAnimationFrame(focusComposerInput)")
    expect(css).not.toContain("#composer-input:disabled")
    expect(css).not.toContain("cursor: wait")
  })

  test("clears the active thread before composing a new chat from the sidebar", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const sidebar = await Bun.file(new URL("../src/ui/codex-thread-sidebar.ts", import.meta.url)).text()

    expect(sidebar).toContain("readonly onNewThreadRequested: () => void")
    expect(sidebar).toContain("const startNewChat = (): void => {")
    expect(sidebar).toContain("activeThreadId = null\n    options.onNewThreadRequested()\n    render()")
    expect(sidebar).not.toContain("options.startThread()")
    expect(sidebar).not.toContain("options.onThreadStarted")
    expect(main).toContain("const beginNewCodexThread = (): void => {")
    expect(main).toContain("setActiveCodexThreadId(null)\n  threadSwitchLoadingSelectionId = null\n  setShellMessages([])")
    expect(main).toContain("onNewThreadRequested: beginNewCodexThread")
  })

  test("keeps transcript scrolling user-controlled during streaming updates", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("const isNearTranscriptEnd")
    expect(model).toContain("transcriptPinnedToEnd: true")
    expect(main).toContain("const stickToEnd = shellModel().transcriptPinnedToEnd && isNearTranscriptEnd()")
    expect(main).toContain("const previousScrollTop = messageList.scrollTop")
    expect(main).toContain("setTranscriptScrollTop(previousScrollTop)")
    expect(main).toContain("proxyTranscriptWheel")
    expect(main).toContain("window.addEventListener(\"wheel\", proxyTranscriptWheel, { passive: false })")
    expect(css).toContain(".tool-card-output")
    expect(css).toContain("max-height: 7rem")
  })

  test("collapses tool cards to one-line summaries until expanded", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/transcript-render.ts", import.meta.url)).text()

    expect(renderer).toContain("@openagentsinc/ui/icon-dom")
    expect(renderer).toContain("tool-card-summary")
    expect(renderer).toContain("bindExpandableToolCard")
    expect(renderer).toContain('header.setAttribute("aria-expanded", "false")')
    expect(renderer).toContain('status.setAttribute("aria-label", input.codexItem.status)')
    expect(renderer).not.toContain("status.textContent = input.codexItem.status")
    expect(renderer).not.toContain("status.textContent = parts.status")
    expect(css).toContain(".tool-card-summary")
    expect(css).toContain(".tool-card-icon")
    expect(css).toContain("border-radius: 999px")
    expect(css).toContain('.tool-card:not([data-expanded="true"]) .tool-card-header')
    expect(css).toContain(".codex-item-card:not([data-expanded=\"true\"]) .codex-item-card-body")
    expect(css).toContain(".tool-card[data-expanded=\"true\"] .tool-card-output")
    expect(css).toContain(".codex-item-card[data-expanded=\"true\"] .codex-item-card-body")
    expect(css).toContain(".codex-item-card[data-expanded=\"true\"] .codex-item-card-copy")
  })

  test("shows a Thinking shimmer until the first streamed response event", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const model = await Bun.file(new URL("../src/ui/main-shell-model.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(main).toContain("@openagentsinc/ui/ai-elements/shimmer")
    expect(model).toContain("thinkingTurnId: null")
    expect(main).toContain("renderThinkingIndicator")
    expect(main).toContain("message-bubble--thinking")
    expect(main).toContain('shimmer.textContent = "Thinking"')
    expect(main).toContain("shellModel().thinkingTurnId = turnId")
    expect(main).toContain('event.type === "message_start"')
    expect(main).toContain('event.type === "message_delta"')
    expect(main).toContain('event.type === "message_replace"')
    expect(main).toContain("shellModel().thinkingTurnId = null")
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
    expect(main).toContain("planComposerAttachmentUpload")
    expect(main).toContain("readyComposerAttachmentTransaction")
    expect(main).toContain("projectComposerAttachmentUploadReceipt")
    expect(main).toContain("DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY")
    expect(main).toContain("base64FromArrayBuffer")
    expect(main).toContain("imageAttachmentsForSubmit")
    expect(main).toContain("sha256DigestForBytes")
    expect(main).toContain("attachmentReceipts")
    expect(main).toContain("createCommandComposerHud")
    expect(main).not.toContain("local-file:/private")
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

  test("wires desktop diff line comments back as review steering input", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/transcript-render.ts", import.meta.url)).text()
    const shared = await Bun.file(new URL("../src/shared/diff-review.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(shared).toContain("openagents.khala_code.diff_review_comment.v1")
    expect(shared).toContain("openagents.khala_code.judge_diff_verdict.v1")
    expect(shared).toContain("priority")
    expect(shared).toContain("confidence")
    expect(renderer).toContain("cb-diff-comment-button")
    expect(renderer).toContain("judge-verdict-card")
    expect(renderer).toContain("judgeDiffVerdictElement")
    expect(renderer).toContain("KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT")
    expect(renderer).toContain("verify command remains the merge authority")
    expect(main).toContain("KhalaCodeDiffReviewSubmitDetailSchema")
    expect(main).toContain("khalaCodeDiffReviewSteeringNote")
    expect(main).toContain("rpc.request.codexTurnSteer")
    expect(main).toContain("stageDiffReviewNoteInComposer")
    expect(css).toContain(".cb-diff-review-editor")
    expect(css).toContain(".cb-diff-review-textarea")
    expect(css).toContain(".judge-verdict-card")
    expect(css).toContain(".judge-verdict-steer-button")
  })

  test("wires source-control AI actions into the desktop diff review surface", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const renderer = await Bun.file(new URL("../src/ui/transcript-render.ts", import.meta.url)).text()
    const shared = await Bun.file(new URL("../src/shared/source-control-action.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(shared).toContain("openagents.khala_code.source_control_action_prompt.v1")
    expect(shared).toContain("commit_message")
    expect(shared).toContain("pr_body")
    expect(shared).toContain("fix_checks")
    expect(renderer).toContain("cb-diff-source-actions")
    expect(renderer).toContain("KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT")
    expect(main).toContain("KhalaCodeSourceControlActionSubmitDetailSchema")
    expect(main).toContain("khalaCodeSourceControlActionPromptText")
    expect(main).toContain("stageSourceControlActionPromptInComposer")
    expect(main).toContain("handleSourceControlActionSubmit")
    expect(css).toContain(".cb-diff-source-action-button")
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

  test("keeps markdown list markers visible in desktop prose", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const blocks = parseMarkdownBlocks("1. First\n2. Second\n\n- Alpha\n- Beta")

    expect(blocks.map(block => block.kind)).toEqual([
      "ordered-list",
      "unordered-list",
    ])
    expect(css).toContain(".message-markdown .md-list--ordered {\n  list-style-type: decimal;")
    expect(css).toContain(".message-markdown .md-list--unordered {\n  list-style-type: disc;")
    expect(css).toContain(".message-markdown .md-list-item + .md-list-item")
    expect(css).not.toContain(".message-markdown .md-list {\n  display: grid;")
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
    expect(parseToolTranscript("codex_spawn: ok\n\nFleet spawn: accepted 0/1\n- slot 1: failed\n  command timed out")).toEqual({
      output: "Fleet spawn: accepted 0/1\n- slot 1: failed\n  command timed out",
      status: "failed",
      toolName: "codex_spawn",
    })
  })

  test("summarizes tool card details for compact rows", () => {
    expect(compactToolSummary([
      "cwd: /tmp/project",
      "",
      "```bash",
      "bun test clients/khala-code-desktop/tests/app-shell.test.ts",
      "```",
      "",
      "Output",
      "",
      "```",
      "ok",
      "```",
    ].join("\n"))).toBe("bun test clients/khala-code-desktop/tests/app-shell.test.ts")
    expect(compactToolSummary([
      "Arguments",
      "",
      "```json",
      "{",
      "  \"uri\": \"uidotsh://ui\"",
      "}",
      "```",
    ].join("\n"))).toBe("\"uri\": \"uidotsh://ui\"")
    expect(compactToolSummary([
      "Arguments",
      "",
      "```json",
      "{",
      "  \"path\": \"/Users/christopherdavid/work/openagents/clients/khala-code-desktop/src/ui/main.ts\"",
      "}",
      "```",
    ].join("\n"))).toBe("\"path\": \"clients/khala-code-desktop/src/ui/main.ts\"")
    expect(compactToolSummary("")).toBe("Details available")
    expect(compactToolSummary("x".repeat(200))).toHaveLength(160)
  })

  test("formats local paths relative to the active Khala Code worktree", () => {
    const root = "/Users/christopherdavid/work/openagents"
    const file = `${root}/clients/khala-code-desktop/src/ui/transcript-render.ts`

    expect(displayPathForKhalaCode(file, root)).toBe(
      "clients/khala-code-desktop/src/ui/transcript-render.ts",
    )
    expect(displayPathForKhalaCode(root, root)).toBe(".")
    expect(displayLocalPathsForKhalaCode(`Edited ${file}`, root)).toBe(
      "Edited clients/khala-code-desktop/src/ui/transcript-render.ts",
    )
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
