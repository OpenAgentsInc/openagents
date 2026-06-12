import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { buildCommandSpecs, PAYOUT_TARGET_KINDS, type CommandContext } from "../src/tui/commands"
import { filterSelectItems, fuzzyScore } from "../src/tui/dialogs"
import { loadKeybindOverrides, parseKeybindsConfig } from "../src/node/keybinds"

const stubCtx: CommandContext = {
  walletActions: {
    send: async () => ({}),
    receive: async () => ({}),
    admitPayoutTarget: async () => ({}),
  },
  assignmentActions: null,
  devActions: null,
  refreshContext: null,
  setRoute: () => {},
  refreshAssignments: async () => {},
  currentAssignments: () => [],
  focusLogs: () => {},
  focusComposer: () => {},
  focusedPane: () => "composer",
  scrollLogs: () => {},
  submitComposer: () => {},
  toggleVerbose: () => true,
  requestShutdown: () => {},
  log: () => {},
}

describe("command registry", () => {
  test("command names are unique and categorized", () => {
    const specs = buildCommandSpecs(stubCtx)
    const names = specs.map((spec) => spec.name)
    expect(new Set(names).size).toBe(names.length)
    for (const spec of specs) {
      expect(spec.title.length).toBeGreaterThan(0)
      expect(spec.category.length).toBeGreaterThan(0)
    }
  })

  test("every footer command has a default key", () => {
    for (const spec of buildCommandSpecs(stubCtx).filter((candidate) => candidate.footer)) {
      expect(spec.key, `${spec.name} is in the footer but has no key`).toBeDefined()
    }
  })

  test("wallet money flows are registered as palette commands", () => {
    const specs = buildCommandSpecs(stubCtx)
    for (const name of ["wallet.send", "wallet.receive", "wallet.admit-payout-target"]) {
      const spec = specs.find((candidate) => candidate.name === name)
      expect(spec, `${name} missing`).toBeDefined()
      expect(spec?.palette).toBe(true)
    }
    expect(PAYOUT_TARGET_KINDS.length).toBe(4)
  })

  test("dev loop actions are registered as palette commands", () => {
    const specs = buildCommandSpecs({
      ...stubCtx,
      devActions: {
        check: async () => ({ action: "check", schema: "test", state: "passed" }),
        apply: async () => ({ action: "apply", schema: "test", state: "no_op" }),
        reload: async () => ({ action: "reload", schema: "test", state: "noop" }),
      },
    })
    for (const name of ["dev.check", "dev.apply", "dev.reload"]) {
      const spec = specs.find((candidate) => candidate.name === name)
      expect(spec, `${name} missing`).toBeDefined()
      expect(spec?.palette).toBe(true)
    }
  })

  test("context view and refresh actions are registered", () => {
    const specs = buildCommandSpecs({
      ...stubCtx,
      refreshContext: async () => ({
        schema: "openagents.pylon.context.v0.3",
        observedAt: "2026-06-12T12:00:00.000Z",
        repo: {
          state: "ready",
          provider: "github",
          fullName: "OpenAgentsInc/openagents",
          branch: "main",
          commitRef: "commit.abc",
          dirtyState: "clean",
          changedCount: 0,
          blockerRefs: [],
        },
        instructions: { refs: [], configRefs: [], blockerRefs: [] },
        adapters: {
          mode: "dev",
          primaryAdapter: "codex",
          reviewerAdapter: "fable",
          codex: {
            state: "ready",
            enabled: true,
            cli: "present",
            credentialSourceRef: "credential.source.codex_agent.codex_cli_login",
            modelRef: "model.codex.gpt-5-codex",
            executionMode: "local_supervised_danger",
            sandboxMode: "danger-full-access",
            danger: true,
            capabilityRefs: ["capability.pylon.local_codex"],
            blockerRefs: [],
          },
          openai: { state: "configured", sourceRefs: ["credential.source.codex_agent.codex_cli_login"], blockerRefs: [] },
          claudeAgent: {
            state: "ready",
            enabled: true,
            credentialSourceRef: "credential.source.claude_agent.local_claude_session",
            modelRef: "model.claude_agent.claude-fable-5",
            fableReviewAvailable: true,
            executionMode: "local_bounded",
            permissionMode: "acceptEdits",
            danger: false,
            capabilityRefs: ["capability.pylon.local_claude_agent"],
            blockerRefs: [],
          },
          backends: [],
          blockerRefs: [],
        },
        currentJob: {
          assignmentRef: null,
          workRequestRef: null,
          workOrderRef: null,
          workspaceRef: null,
          worktreeRef: null,
          verificationCommandRef: null,
          latestVerificationRef: null,
          primaryAdapter: "codex",
          reviewerAdapter: "fable",
          requiredCapabilityRefs: ["capability.pylon.local_codex"],
          blockerRefs: [],
        },
        blockerRefs: [],
      }),
    })
    const refresh = specs.find((candidate) => candidate.name === "context.refresh")
    const route = specs.find((candidate) => candidate.name === "view.context")
    expect(refresh?.palette).toBe(true)
    expect(route?.key).toBe("f6")
  })

  test("focus toggle flips based on the focused pane", () => {
    let logsFocused = 0
    let composerFocused = 0
    let pane: "logs" | "composer" = "composer"
    const specs = buildCommandSpecs({
      ...stubCtx,
      focusedPane: () => pane,
      focusLogs: () => {
        logsFocused += 1
      },
      focusComposer: () => {
        composerFocused += 1
      },
    })
    const toggle = specs.find((spec) => spec.name === "focus.toggle")!
    void toggle.run()
    pane = "logs"
    void toggle.run()
    expect(logsFocused).toBe(1)
    expect(composerFocused).toBe(1)
  })
})

describe("fuzzy filter", () => {
  test("subsequence matching ranks contiguous matches first", () => {
    expect(fuzzyScore("wal", "wallet send")).not.toBeNull()
    expect(fuzzyScore("xyz", "wallet send")).toBeNull()
    expect(fuzzyScore("", "anything")).toBe(0)
    const items = [
      { id: "a", label: "toggle verbose logs" },
      { id: "b", label: "wallet send sats" },
      { id: "c", label: "weird about lots" },
    ]
    const filtered = filterSelectItems(items, "wal")
    expect(filtered[0]?.id).toBe("b")
  })
})

describe("keybind overrides", () => {
  test("parses a valid keybinds file", () => {
    const overrides = parseKeybindsConfig('{"bindings":{"palette.open":"ctrl+p"}}')
    expect(overrides["palette.open"]).toBe("ctrl+p")
  })

  test("rejects non-string binding values", () => {
    expect(() => parseKeybindsConfig('{"bindings":{"palette.open":7}}')).toThrow()
  })

  test("load: absent file yields empty overrides", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-keybinds-"))
    const result = await loadKeybindOverrides(dir)
    expect(result.state).toBe("absent")
    expect(result.overrides).toEqual({})
  })

  test("load: invalid file is reported and ignored", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-keybinds-"))
    writeFileSync(join(dir, "keybinds.json"), "{nope")
    const result = await loadKeybindOverrides(dir)
    expect(result.state).toBe("invalid")
    expect(result.overrides).toEqual({})
    expect(result.error).toBeDefined()
  })

  test("load: valid file yields overrides", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-keybinds-"))
    writeFileSync(join(dir, "keybinds.json"), '{"bindings":{"logs.verbose-toggle":"f9"}}')
    const result = await loadKeybindOverrides(dir)
    expect(result.state).toBe("loaded")
    expect(result.overrides["logs.verbose-toggle"]).toBe("f9")
  })
})
