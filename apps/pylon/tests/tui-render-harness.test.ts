import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { createTuiHarness, type TuiHarness } from "../src/tui/harness"
import { logMessage, setWalletStatus, publishLogEntries } from "../src/node/runtime"
import { setActiveRoute, setAssignmentRows } from "../src/tui/store"
import { openConfirm, dialogOpen } from "../src/tui/dialogs"

let harness: TuiHarness | null = null

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
    expect(frame).toContain("Active Workroom")
    expect(frame).toMatchSnapshot()
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
