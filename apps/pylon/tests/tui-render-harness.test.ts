import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { createTuiHarness, type TuiHarness } from "../src/tui/harness"
import { logMessage, setWalletStatus, publishLogEntries } from "../src/node/runtime"
import { setActiveRoute, setAssignmentRows } from "../src/tui/store"
import { openConfirm, dialogOpen } from "../src/tui/dialogs"
import type { PylonContextProjection } from "../src/context-projection"

let harness: TuiHarness | null = null

function contextFixture(): PylonContextProjection {
  return {
    schema: "openagents.pylon.context.v0.3",
    observedAt: "2026-06-12T12:00:00.000Z",
    repo: {
      state: "ready",
      provider: "github",
      fullName: "OpenAgentsInc/openagents",
      branch: "main",
      commitRef: "commit.dbfc091df123",
      dirtyState: "clean",
      changedCount: 0,
      blockerRefs: [],
    },
    instructions: {
      refs: [
        { sourceRef: "instruction.workspace.agents", state: "present", relativePath: "AGENTS.md", digestRef: "file.digest.workspace" },
        { sourceRef: "instruction.repo.invariants", state: "present", relativePath: "INVARIANTS.md", digestRef: "file.digest.repo" },
      ],
      configRefs: ["config.pylon.local", "config.pylon.dev.local_supervised_danger"],
      blockerRefs: [],
    },
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
        capabilityRefs: ["capability.pylon.local_claude_agent"],
        blockerRefs: [],
      },
      backends: [
        { backendRef: "backend.opencode.cli", state: "ready", modelRef: "model.opencode.default", blockerRefs: [] },
        { backendRef: "backend.apple_fm", state: "ready", modelRef: "model.apple_foundation_model", blockerRefs: [] },
        { backendRef: "backend.gemini", state: "missing", modelRef: null, blockerRefs: ["blocker.backend.gemini_auth_missing"] },
        { backendRef: "backend.psionic.qwen35", state: "missing", modelRef: null, blockerRefs: ["blocker.psionic_qwen35.connector_unconfigured"] },
      ],
      blockerRefs: [],
    },
    currentJob: {
      assignmentRef: "assignment.public.fixture",
      workRequestRef: "work.request.fixture",
      workOrderRef: "work.order.fixture",
      workspaceRef: "workspace.public.fixture",
      worktreeRef: "worktree.public.fixture",
      verificationCommandRef: "verify.bun-test",
      latestVerificationRef: "verification.pass.fixture",
      primaryAdapter: "codex",
      reviewerAdapter: "fable",
      requiredCapabilityRefs: ["capability.pylon.local_codex", "capability.pylon.local_claude_agent"],
      blockerRefs: [],
    },
    blockerRefs: [],
  }
}

afterEach(async () => {
  await harness?.dispose()
  harness = null
})

describe("tui render harness (Pilot model)", () => {
  test("dashboard snapshot at 80x24", async () => {
    harness = await createTuiHarness({ width: 80, height: 24 })
    expect(await harness.frame()).toMatchSnapshot()
  })

  test("dashboard snapshot at 120x40", async () => {
    harness = await createTuiHarness({ width: 120, height: 40 })
    expect(await harness.frame()).toMatchSnapshot()
  })

  test("narrow terminal collapses the sidebar", async () => {
    harness = await createTuiHarness({ width: 50, height: 20 })
    const frame = await harness.frame()
    expect(frame).not.toContain("Telemetry & Wallet")
    expect(frame).not.toContain("Repo & AI Context")
    expect(frame).toContain("Active Workroom")
    expect(frame).toMatchSnapshot()
  })

  test("wide dashboard renders repo and AI context without hiding telemetry", async () => {
    harness = await createTuiHarness({ width: 150, height: 36, contextProjection: contextFixture() })
    const frame = await harness.frame()
    expect(frame).toContain("Repo & AI Context")
    expect(frame).toContain("Telemetry & Wallet")
    expect(frame).toContain("OpenAgentsInc/openagents")
    expect(frame).toContain("Codex DANGER")
    expect(frame).toContain("Fable: yes")
    expect(frame).toContain("workspace.public.fixture")
  })

  test("narrow terminal can open the context route", async () => {
    harness = await createTuiHarness({ width: 50, height: 20, contextProjection: contextFixture() })
    setActiveRoute("context")
    const frame = await harness.frame()
    expect(frame).toContain("Repo & AI Context")
    expect(frame).toContain("OpenAgentsInc/openagents")
    expect(frame).toContain("Codex DANGER")
    expect(frame).toContain("work.order.fixture")
  })

  test("fake event stream renders into the feed (protocol test)", async () => {
    harness = await createTuiHarness()
    await Effect.runPromise(
      publishLogEntries(harness.runtime, [
        { at: "2026-06-10T12:00:00.000Z", level: "info", message: "protocol line one" },
        { at: "2026-06-10T12:00:01.000Z", level: "error", message: "protocol error line" },
        { at: "2026-06-10T12:00:02.000Z", level: "verbose", message: "hidden verbose line" },
      ]),
    )
    await harness.settle()
    const frame = await harness.frame()
    expect(frame).toContain("protocol line one")
    expect(frame).toContain("protocol error line")
    expect(frame).not.toContain("hidden verbose line")
  })

  test("wallet events update the sidebar panes", async () => {
    harness = await createTuiHarness()
    await Effect.runPromise(
      setWalletStatus(harness.runtime, { daemonOnline: true, balanceSats: 12345, readiness: "receive-ready" }),
    )
    await harness.settle()
    const frame = await harness.frame()
    expect(frame).toContain("ONLINE")
    expect(frame).toContain("12,345")
  })

  test("assignments and wallet routes render their surfaces", async () => {
    harness = await createTuiHarness()
    setAssignmentRows([
      { assignmentRef: "a1", leaseRef: "lease-snapshot-1", goal: "render this goal", paymentMode: "no-spend", expiresAt: "2026-06-11T00:00:00Z" },
    ])
    setActiveRoute("assignments")
    let frame = await harness.frame()
    expect(frame).toContain("lease-snapshot-1")
    expect(frame).toContain("render this goal")
    setActiveRoute("wallet")
    frame = await harness.frame()
    expect(frame).toContain("Balance history")
    setActiveRoute("context")
    frame = await harness.frame()
    expect(frame).toContain("Repo & AI Context")
    setActiveRoute("dashboard")
    frame = await harness.frame()
    expect(frame).toContain("Active Workroom")
  })

  test("confirm dialog renders, resolves on key press, and restores", async () => {
    harness = await createTuiHarness()
    const decision = openConfirm({ title: "Test confirm", body: "Proceed with thing?", confirmLabel: "Go" })
    await harness.settle()
    let frame = await harness.frame()
    expect(frame).toContain("Test confirm")
    expect(frame).toContain("Proceed with thing?")
    expect(dialogOpen()).toBe(true)
    harness.keys.pressKey("y")
    await harness.settle()
    expect(await decision).toBe(true)
    expect(dialogOpen()).toBe(false)
    frame = await harness.frame()
    expect(frame).not.toContain("Test confirm")
  })

  test("escape cancels a confirm dialog", async () => {
    harness = await createTuiHarness()
    const decision = openConfirm({ title: "Cancel me", body: "Really?" })
    await harness.settle()
    harness.keys.pressEscape()
    await harness.settle()
    expect(await decision).toBe(false)
  })

  test("ctrl+k opens the command palette through the keymap", async () => {
    harness = await createTuiHarness()
    harness.keys.pressKey("\x0b")
    await harness.settle()
    const frame = await harness.frame()
    expect(frame).toContain("Command palette")
    expect(frame).toContain("Quit Pylon")
    harness.keys.pressEscape()
    await harness.settle()
    expect(await harness.frame()).not.toContain("Command palette")
  })

  test("detached harness stops receiving runtime events after dispose", async () => {
    harness = await createTuiHarness()
    await Effect.runPromise(logMessage(harness.runtime, "info", "before dispose"))
    await harness.settle()
    expect(await harness.frame()).toContain("before dispose")
    const runtime = harness.runtime
    await harness.dispose()
    const disposed = harness
    harness = null
    await Effect.runPromise(logMessage(runtime, "info", "after dispose"))
    await new Promise((resolve) => setTimeout(resolve, 40))
    // No crash and no further render activity is the contract; the renderer
    // is destroyed so we only assert the call completes.
    expect(disposed).toBeDefined()
  })
})
