// Owned headless render harness for the Pylon TUI (issue #4742) — the
// Textual "Pilot" model implemented in-repo on top of @opentui/solid's
// testRender: mount the real dashboard against a dumpable text buffer with
// no TTY, inject key events programmatically, capture character frames for
// bun:test snapshots, and drive the real Phase 0 runtime + bridge so fake
// PylonEvent streams assert rendered frames. No upstreaming planned; this
// module is also importable by the runtime package's renderer tests.

import { testRender, useRenderer } from "@opentui/solid"
import { ErrorBoundary } from "solid-js"
import type { CliRenderer } from "@opentui/core"
import { Effect, Fiber } from "effect"
import { makePylonNodeRuntime, type PylonNodeRuntime } from "../node/runtime"
import { attachRuntimeToView } from "./bridge"
import { Dashboard, installDashboardChrome, type StartDashboardOptions } from "./app"
import { resetCommandRegistry, type AssignmentActions, type WalletActions } from "./commands"
import { resetDialogState } from "./dialogs"
import { resetSurfaceState, resetViewState } from "./store"

export interface TuiHarnessOptions {
  width?: number
  height?: number
  verbose?: boolean
  walletActions?: WalletActions
  assignmentActions?: AssignmentActions | null
  keybindOverrides?: Record<string, string>
  onRequestShutdown?: () => void
}

export interface TuiHarness {
  runtime: PylonNodeRuntime
  renderer: CliRenderer
  // Renders a frame and returns it as plain text rows.
  frame: () => Promise<string>
  keys: {
    typeText: (text: string) => void
    // Raw key sequence (e.g. "y", "\x1b[15~" for f5).
    pressKey: (key: string, modifiers?: Record<string, boolean>) => void
    pressEnter: () => void
    pressEscape: () => void
    pressArrow: (direction: "up" | "down" | "left" | "right") => void
  }
  // Lets async work (Effect fibers, store updates) settle before capturing.
  settle: (ms?: number) => Promise<void>
  dispose: () => Promise<void>
}

const noopWalletActions: WalletActions = {
  send: async () => ({}),
  receive: async () => ({}),
  admitPayoutTarget: async () => ({}),
}

export async function createTuiHarness(options: TuiHarnessOptions = {}): Promise<TuiHarness> {
  // Every harness gets pristine module-level view state.
  resetViewState()
  resetSurfaceState()
  resetDialogState()
  resetCommandRegistry()

  const runtime = await Effect.runPromise(makePylonNodeRuntime)

  // Note: the 3D network pane stays disabled in the harness (enable3dFlag
  // defaults to false and only startDashboard turns it on), keeping frames
  // deterministic and CI GPU-free.
  const chromeOptions: Pick<
    StartDashboardOptions,
    "walletActions" | "onRequestShutdown" | "onVerboseChange" | "keybindOverrides"
  > = {
    walletActions: options.walletActions ?? noopWalletActions,
    onRequestShutdown: options.onRequestShutdown ?? (() => {}),
    keybindOverrides: options.keybindOverrides,
  }

  // HarnessRoot installs chrome from inside the component tree, where
  // useRenderer() is available, before <Dashboard/> children mount - the
  // same ordering startDashboard guarantees in a real terminal.
  function HarnessRoot() {
    const renderer = useRenderer()
    installDashboardChrome(renderer as CliRenderer, chromeOptions, options.assignmentActions ?? null)
    return (
      <ErrorBoundary fallback={(error) => <text>{`dashboard crashed: ${String(error)}`}</text>}>
        <Dashboard />
      </ErrorBoundary>
    )
  }

  const setup = await testRender(() => <HarnessRoot />, {
    width: options.width ?? 80,
    height: options.height ?? 24,
  })

  const attachFiber = Effect.runFork(
    Effect.scoped(
      Effect.gen(function* () {
        yield* attachRuntimeToView(runtime, { verbose: options.verbose ?? false, batchWindowMs: 1 })
        yield* Effect.never
      }),
    ),
  )

  const settle = async (ms = 30) => {
    await new Promise((resolve) => setTimeout(resolve, ms))
    await setup.renderOnce()
  }

  return {
    runtime,
    renderer: setup.renderer,
    frame: async () => {
      await setup.renderOnce()
      return setup.captureCharFrame()
    },
    keys: {
      typeText: (text) => setup.mockInput.typeText(text),
      pressKey: (key, modifiers) => setup.mockInput.pressKey(key, modifiers as never),
      pressEnter: () => setup.mockInput.pressEnter(),
      pressEscape: () => setup.mockInput.pressEscape(),
      pressArrow: (direction) => setup.mockInput.pressArrow(direction),
    },
    settle,
    dispose: async () => {
      await Effect.runPromise(Fiber.interrupt(attachFiber))
      setup.renderer.destroy()
    },
  }
}
