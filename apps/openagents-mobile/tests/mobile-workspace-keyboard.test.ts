import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import { buildHomeProgram } from "../src/screens/home-core"
import { mobileWorkspaceKeyboardCommand } from "../src/screens/mobile-workspace-keyboard"

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})
const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.workspace_keyboard.v1", () => {
  test("parses only the closed command set", () => {
    expect(mobileWorkspaceKeyboardCommand({ key: "n", metaKey: true })).toBe("new_task")
    expect(mobileWorkspaceKeyboardCommand({ key: "k", ctrlKey: true })).toBe("navigation")
    expect(mobileWorkspaceKeyboardCommand({ key: "1", metaKey: true })).toBe("navigation")
    expect(mobileWorkspaceKeyboardCommand({ key: "2", metaKey: true })).toBe("detail")
    expect(mobileWorkspaceKeyboardCommand({ key: "Escape" })).toBe("dismiss")
    expect(mobileWorkspaceKeyboardCommand({ key: "n" })).toBeNull()
    expect(mobileWorkspaceKeyboardCommand({ key: "x", metaKey: true })).toBeNull()
  })

  test("dispatches layout-aware navigation/detail/dismiss without changing authority", async () => {
    const program = buildHomeProgram({ workspaceWidth: 390 })
    program.khala.draftChanged("Preserve authority")
    program.workspace.dispatchKeyboardCommand("navigation")
    await Effect.runPromise(settle)
    expect(await Effect.runPromise(lastState(program))).toMatchObject({
      drawerOpen: true,
      workspaceFocusTarget: "navigation",
      khala: { draft: "Preserve authority" },
    })
    program.workspace.dispatchKeyboardCommand("detail")
    await Effect.runPromise(settle)
    expect(await Effect.runPromise(lastState(program))).toMatchObject({
      drawerOpen: false,
      workspaceFocusTarget: "transcript",
      khala: { draft: "Preserve authority" },
    })
  })
})
