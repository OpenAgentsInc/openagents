/**
 * EN view-program unit tests (#8574): pure state -> expected component tree,
 * plus the full typed intent loop run headlessly through the real registry —
 * dispatch -> handler -> SubscriptionRef -> re-rendered view.
 */
import { describe, expect, test } from "bun:test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  desktopShellIntents,
  desktopShellView,
  formatRelativeTimestamp,
  formatShellTimestamp,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  noteMessage,
  withInput,
  withFleetDeploymentRequested,
  withFleetDeploymentResult,
  withFleetDesk,
  withFleetObjective,
  withNewChat,
  withChatSelected,
  withLoopProof,
  withWorkspace,
  withCommandPalette,
  withWorkspaceFile,
  withWorkspaceSnapshot,
  withNote,
  withPending,
  withTurnResult,
  type DesktopShellState,
} from "./shell.ts"
import { openagentsDesktopTheme } from "./theme.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

/** Collect every catalog node in a view tree (children live under varying props). */
const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const navItemById = (view: View, id: string): AnyNode | undefined => {
  const nav = nodeByKey(view, "sidebar-navigation") as AnyNode | undefined
  const sections = nav?.sections as ReadonlyArray<{ items?: ReadonlyArray<AnyNode> }> | undefined
  return sections?.flatMap((section) => section.items ?? []).find((item) => item.id === id)
}

const testThread = { id: "test-thread", title: "New chat", updatedAt: "2026-07-10T18:04:00.000Z", notes: [] } as const
const baseState: DesktopShellState = { ...initialDesktopShellState("electron/darwin", "18:04"), threads: [testThread], activeThreadId: testThread.id }
const fixedNow = () => "18:05"

/** A minimal loaded Codex history detail page (the VIDEOEDITS-regression shape). */
const historyPageFixture = {
  rootThreadRef: "history-root",
  selectedThreadRef: "history-root",
  agents: [],
  items: [{
    itemRef: "item-1",
    threadRef: "history-root",
    sequence: 0,
    timestamp: "2026-07-10T18:04:00.000Z",
    kind: "user_message" as const,
    label: "You",
    summary: "historical message",
    status: null,
    fields: [],
    redacted: false,
    sourceType: "codex/session",
  }],
  offset: 0,
  limit: 50,
  totalItems: 1,
  hasPrevious: false,
  hasNext: false,
  completeness: { source: 1, rendered: 1, redactions: 0, gaps: 0, complete: true },
} as const

describe("desktopShellView (state -> component tree)", () => {
  test("renders neutral chat workspace without a duplicate top bar", () => {
    const view = desktopShellView(baseState)

    expect(view._tag).toBe("BackgroundGradient")
    expect(view.key).toBe("desktop-liquid-backdrop")

    expect(nodeByKey(view, "shell-header")).toBeUndefined()
    expect(nodeByKey(view, "shell-title")).toBeUndefined()

    expect(nodeByKey(view, "shell-welcome-title")).toBeUndefined()

    const transcript = nodeByKey(view, "shell-transcript")
    expect(transcript?._tag).toBe("Transcript")
    expect(transcript?.pinToEnd).toBe(true)
    expect((transcript?.messages as Array<unknown>).length).toBe(0)

    expect(nodeByKey(view, "shell-input")?._tag).toBe("TextField")
    expect(nodeByKey(view, "shell-note")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-sidebar")?._tag).toBe("Stack")
    expect((nodeByKey(view, "shell-sidebar")?.style as { surface?: string }).surface).toBe("glass")
    expect(nodeByKey(view, "sidebar-navigation")?._tag).toBe("NavRail")
    expect(navItemById(view, "workspace-chat")).toMatchObject({icon:"Chats",accessibilityLabel:"Chat"})
    expect(navItemById(view, "shell-command-palette-toggle")).toMatchObject({icon:"Menu",accessibilityLabel:"Open command palette"})
    expect(navItemById(view, "shell-settings-toggle")).toMatchObject({icon:"Settings",accessibilityLabel:"Open Settings"})
    expect(navItemById(view, "workspace-home")?.icon).toBe("Home")
    expect((nodeByKey(view, "sidebar-navigation")?.sections as Array<AnyNode>)[1]?.label).toBe("Codex history · all time")
    expect(navItemById(view, "sidebar-thread-test-thread")?.label).toBe("New chat")
    expect(nodeByKey(view, "sidebar-thread-icon-test-thread")).toBeUndefined()
    expect(navItemById(view, "sidebar-thread-test-thread")?.meta).toBeDefined()
    expect(nodeByKey(view, "shell-send-icon")?.name).toBe("Plane")
    expect(nodeByKey(view, "codex-thread-details-title")).toBeUndefined()
    expect(nodeByKey(view, "codex-thread-details-label")).toBeUndefined()
  })

  test("New chat is the first dock item, Fleet second (owner directive), and Fleet opens the read-only workspace", () => {
    const view = desktopShellView(baseState)
    const nav = nodeByKey(view, "sidebar-navigation")
    const dock = (nav?.sections as Array<{ id: string; items: Array<AnyNode> }>)[0]
    expect(dock?.id).toBe("sidebar-workspace-dock")
    expect(dock?.items[0]?.id).toBe("workspace-new-chat")
    expect(dock?.items[1]?.id).toBe("workspace-fleet")
    expect(navItemById(view, "workspace-new-chat")).toMatchObject({ icon: "ChatCompose", accessibilityLabel: "New chat" })
    expect((navItemById(view, "workspace-new-chat")?.onSelect as { name?: string })?.name).toBe("DesktopNewChat")
    expect(navItemById(view, "workspace-fleet")).toMatchObject({ icon: "Agent", accessibilityLabel: "Fleet" })
    expect((navItemById(view, "workspace-fleet")?.onSelect as { name?: string })?.name).toBe("DesktopWorkspaceSelected")

    const fleetView = desktopShellView(withWorkspace(baseState, "fleet"))
    expect(nodeByKey(fleetView, "workspace-fleet-panel")?._tag).toBe("Stack")
    expect(nodeByKey(fleetView, "shell-composer")).toBeUndefined()
    expect(nodeByKey(fleetView, "shell-transcript")).toBeUndefined()
  })

  test("sidebar chat rows are compact navigation items with trailing metadata", () => {
    const view = desktopShellView(baseState)
    const item = navItemById(view, `sidebar-thread-${testThread.id}`)
    expect(item).toMatchObject({label:"New chat",accessibilityLabel:"Open chat New chat"})
    expect(item?.meta).toBeDefined()
    expect((item?.onSelect as {name?:string})?.name).toBe("DesktopChatSelected")
  })

  test("large Codex catalogs use one virtual scroll owner and pending selection is active immediately", () => {
    const roots = Array.from({ length: 50 }, (_, index) => ({
      threadRef:`history-${index}`,parentThreadRef:null,title:`History ${index}`,status:"completed" as const,
      createdAt:"2026-07-10T18:04:00.000Z",updatedAt:"2026-07-10T18:04:00.000Z",depth:0,descendantCount:0,
      model:null,role:null,nickname:null,agentPath:null,sourceVersion:null,reasoning:null,
    }))
    const state:DesktopShellState={...baseState,history:{...baseState.history,catalog:{roots,agents:roots},pendingThreadRef:"history-7"}}
    const view=desktopShellView(state)
    const nav=nodeByKey(view,"sidebar-navigation")
    const history=(nav?.sections as Array<AnyNode>)[1]
    expect(history?.id).toBe("sidebar-history-list")
    expect((history?.items as Array<unknown>)).toHaveLength(41)
    expect(navItemById(view,"sidebar-history-load-more")).toMatchObject({label:"Load 10 more"})
    expect(nav?.activeId).toBe("sidebar-thread-history-7")
  })

  test("holding the platform modifier replaces the first nine timestamps with jump hints", () => {
    const roots = Array.from({ length: 10 }, (_, index) => ({
      threadRef:`hint-${index}`,parentThreadRef:null,title:`History ${index}`,status:"completed" as const,
      createdAt:"2026-07-10T18:04:00.000Z",updatedAt:"2026-07-10T18:04:00.000Z",depth:0,descendantCount:0,
      model:null,role:null,nickname:null,agentPath:null,sourceVersion:null,reasoning:null,
    }))
    const view=desktopShellView({...baseState,historyShortcutHintsVisible:true,history:{...baseState.history,catalog:{roots,agents:roots}}})
    expect(navItemById(view,"sidebar-thread-hint-0")?.meta).toBe("1")
    expect(navItemById(view,"sidebar-thread-hint-8")?.meta).toBe("9")
    expect(navItemById(view,"sidebar-thread-hint-9")?.meta).toBe("")
  })

  test("buttons carry the typed intent refs (no ad hoc handlers)", () => {
    const view = desktopShellView(baseState)
    const note = nodeByKey(view, "shell-note") as { onPress?: { name?: string } }
    expect(note.onPress?.name).toBe("DesktopNoteSubmitted")
    const input = nodeByKey(view, "shell-input") as {
      onChange?: { name?: string }
      onSubmit?: { name?: string }
    }
    expect(input.onChange?.name).toBe("DesktopInputChanged")
    expect(input.onSubmit?.name).toBe("DesktopNoteSubmitted")
  })

  test("messages ride the v29 chat chrome contract: typed senderLabel/timestamp, body is only the text", () => {
    const system = noteMessage({ key: "boot-0", role: "system", text: "hello", timestamp: "18:04" })
    expect(system.key).toBe("boot-0")
    expect(system.role).toBe("system")
    expect(system.senderLabel).toBe("SYSTEM")
    expect(system.timestamp).toBe("18:04")
    expect(system.body.length).toBe(1)
    const systemText = system.body[0] as unknown as AnyNode
    expect(systemText._tag).toBe("Text")
    expect(systemText.color).toBe("textMuted")
    // sender identity is typed message data, never concatenated into the body
    expect(systemText.content).toBe("hello")

    const user = noteMessage({ key: "note-2", role: "user", text: "rofl", timestamp: "18:05" })
    expect(user.senderLabel).toBe("YOU")
    expect((user.body[0] as unknown as AnyNode).color).toBe("textPrimary")
    expect((user.body[0] as unknown as AnyNode).content).toBe("rofl")

    const assistant = noteMessage({ key: "assistant-1", role: "assistant", text: "I’m here", timestamp: "18:05" })
    expect(assistant.senderLabel).toBe("ASSISTANT")
    expect((assistant.body[0] as unknown as AnyNode).content).toBe("I’m here")

    const pending = noteMessage({ key: "pending-0", role: "user", text: "wait for it", timestamp: "18:05" })
    expect(pending.senderLabel).toBe("YOU · PENDING")
  })

  test("composer carries the harness selector with Codex selected by default", () => {
    const view = desktopShellView(baseState)
    const fable = nodeByKey(view, "shell-harness-fable") as { onPress?: { name?: string }; variant?: string; disabled?: boolean }
    const codex = nodeByKey(view, "shell-harness-codex") as { onPress?: { name?: string }; variant?: string }
    expect(fable?.onPress?.name).toBe("DesktopHarnessSelected")
    expect(codex?.onPress?.name).toBe("DesktopHarnessSelected")
    expect(codex?.variant).toBe("secondary")
    expect(fable?.variant).toBe("ghost")
    expect(baseState.selectedHarness).toBe("codex")

    const fableSelected = desktopShellView({ ...baseState, selectedHarness: "fable" })
    expect((nodeByKey(fableSelected, "shell-harness-fable") as { variant?: string }).variant).toBe("secondary")
    expect((nodeByKey(fableSelected, "shell-harness-codex") as { variant?: string }).variant).toBe("ghost")

    const pending = desktopShellView(withPending(baseState, true))
    expect(nodeByKey(pending, "shell-harness-fable")?.disabled).toBe(true)
    expect(nodeByKey(pending, "shell-harness-codex")?.disabled).toBe(true)
  })

  test("composer rides the v29 submit lifecycle contract: clearOnSubmit + pending disables", () => {
    const idle = nodeByKey(desktopShellView(baseState), "shell-input")
    expect(idle?.clearOnSubmit).toBe(true)
    expect(idle?.disabled).toBe(false)

    const pendingView = desktopShellView(withPending(baseState, true))
    expect(nodeByKey(pendingView, "shell-input")?.disabled).toBe(true)
    expect(nodeByKey(pendingView, "shell-note")?.disabled).toBe(true)
  })
})

describe("pure transitions", () => {
  test("withNote trims, clears the composer value binding, and appends a user note", () => {
    const next = withNote(withInput(baseState, "  hello desktop  "), "  hello desktop  ", "18:05")
    expect(next.input).toBe("")
    expect(next.pending).toBe(true)
    expect(next.notes.length).toBe(1)
    expect(next.notes[0]).toEqual({
      key: "pending-0",
      role: "user",
      text: "hello desktop",
      timestamp: "18:05",
    })
  })

  test("withNote ignores empty input", () => {
    expect(withNote(baseState, "   ", "18:05")).toBe(baseState)
  })

  test("withPending toggles the composer-disabling flag without touching notes", () => {
    const pending = withPending(baseState, true)
    expect(pending.pending).toBe(true)
    expect(pending.notes).toBe(baseState.notes)
    expect(withPending(pending, false).pending).toBe(false)
  })

  test("failed turns remove optimistic notes before rendering the error", () => {
    const pending = withNote(baseState, "not confirmed", "18:05")
    const next = withTurnResult(pending, { ok: false, error: "Still pending reconciliation." }, "18:06")

    expect(next.pending).toBe(false)
    expect(next.notes).toEqual([
      {
        key: "error-1",
        role: "system",
        text: "Still pending reconciliation.",
        timestamp: "18:06",
      },
    ])
  })

  test("Project home is a real typed workspace projection over persisted threads", () => {
    const home = withWorkspace(baseState, "home")
    expect(nodeByKey(desktopShellView(home), "workspace-home-panel")?._tag).toBe("Stack")
    expect(nodeByKey(desktopShellView(home), "workspace-home-thread-test-thread")?._tag).toBe("Card")
    expect(nodeByKey(desktopShellView(home), "shell-composer")).toBeUndefined()
  })

  test("Files workspace projects only host-provided local entries and file content", () => {
    const files = withWorkspaceSnapshot(withWorkspace(baseState, "files"), {
      root: "/workspace",
      label: "workspace",
      git: "changed",
      entries: [{ name: "README.md", path: "/workspace/README.md", kind: "file" }],
    })
    const view = desktopShellView(files)
    expect(nodeByKey(view, "workspace-files-panel")?._tag).toBe("Stack")
    expect(nodeByKey(view, "workspace-file-/workspace/README.md")?._tag).toBe("Button")
    expect(nodeByKey(view, "shell-composer")).toBeUndefined()
  })

  test("Files workspace exposes a bounded editor only for a complete host-provided text file", () => {
    const file = {
      path: "/workspace/README.md",
      content: "before",
      revision: "revision-before",
      truncated: false,
    } as const
    const files = withWorkspaceFile(withWorkspaceSnapshot(withWorkspace(baseState, "files"), {
      root: "/workspace",
      label: "workspace",
      git: "clean",
      entries: [{ name: "README.md", path: "/workspace/README.md", kind: "file" }],
    }), file)
    const view = desktopShellView(files)
    expect(nodeByKey(view, "workspace-file-editor")?._tag).toBe("TextField")
    expect(nodeByKey(view, "workspace-file-save")?._tag).toBe("Button")
    expect(nodeByKey(view, "workspace-file-preview-content")).toBeUndefined()

    const truncated = desktopShellView(withWorkspaceFile(files, { ...file, truncated: true }))
    expect(nodeByKey(truncated, "workspace-file-editor")).toBeUndefined()
    expect(nodeByKey(truncated, "workspace-file-preview-truncated")?._tag).toBe("Text")
  })

  test("Review workspace renders only typed changed-file rows and a bounded diff", () => {
    const review = {
      ...withWorkspaceSnapshot(withWorkspace(baseState, "review"), {
        root: "/workspace",
        label: "workspace",
        git: "changed" as const,
        entries: [],
      }),
      workspaceGitStatus: {
        state: "available" as const,
        changes: [{ path: "README.md", kind: "modified" as const }],
        truncated: false,
      },
      workspaceGitDiff: {
        state: "available" as const,
        path: "README.md",
        content: "-before\n+after",
        truncated: false,
      },
    }
    const view = desktopShellView(review)
    expect(nodeByKey(view, "workspace-review-panel")?._tag).toBe("Stack")
    expect(nodeByKey(view, "workspace-review-change-README.md")?._tag).toBe("Button")
    expect(nodeByKey(view, "workspace-review-diff-content")?.content).toBe("-before\n+after")
    expect(nodeByKey(view, "shell-composer")).toBeUndefined()
  })

  test("command palette exposes only closed registry commands that reuse existing intent refs", () => {
    const view = desktopShellView(withCommandPalette(baseState, true))
    expect(nodeByKey(view, "desktop-command-palette")?._tag).toBe("Card")
    const files = nodeByKey(view, "desktop-command-workspace.files") as {
      onPress?: { name?: string; payload?: unknown }
    }
    expect(files.onPress?.name).toBe("DesktopWorkspaceSelected")
    expect(JSON.stringify(files.onPress)).toContain("files")
    expect(nodeByKey(view, "desktop-command-palette-close")?._tag).toBe("Button")
  })

  test("New chat resets the conversation and current-chat navigation closes Fleet", () => {
    const activeFleet = withFleetDesk(withNote(baseState, "Ship the app", "18:05"))
    expect(activeFleet.fleetDeskOpen).toBe(true)
    const existing = { id: "test-thread", title: "Existing", updatedAt: "2026-07-10T18:05:00.000Z", notes: [] } as const
    expect(withChatSelected(activeFleet, existing).fleetDeskOpen).toBe(false)

    const fresh = withNewChat(activeFleet, existing)
    expect(fresh.fleetDeskOpen).toBe(false)
    expect(fresh.fleetObjective).toBe("")
    expect(fresh.fleetDeployment).toBe("not_requested")
    expect(fresh.notes).toHaveLength(0)
  })

  test("New chat exits a loaded Codex history page into a fresh empty transcript", () => {
    const withHistory: DesktopShellState = {
      ...baseState,
      history: {
        ...baseState.history,
        page: historyPageFixture,
        selectedItemRef: "item-1",
        expandedThreadRefs: ["history-root"],
        pendingThreadRef: "history-root",
      },
    }
    // Loaded history page owns the chat workspace before New chat…
    expect(nodeByKey(desktopShellView(withHistory), "history-workspace-split")).toBeDefined()
    expect(nodeByKey(desktopShellView(withHistory), "shell-transcript")).toBeUndefined()

    const freshThread = { id: "fresh-thread", title: "New chat", updatedAt: "2026-07-11T16:08:00.000Z", notes: [] } as const
    const next = withNewChat(withHistory, freshThread)
    expect(next.workspace).toBe("chat")
    expect(next.activeThreadId).toBe("fresh-thread")
    expect(next.history.page).toBeNull()
    expect(next.history.selectedItemRef).toBeNull()
    expect(next.history.pendingThreadRef).toBeNull()
    // The catalog (sidebar list) survives; only the loaded page is exited.
    expect(next.history.catalog).toBe(withHistory.history.catalog)

    const view = desktopShellView(next)
    expect(nodeByKey(view, "history-workspace-split")).toBeUndefined()
    const transcript = nodeByKey(view, "shell-transcript")
    expect(transcript?._tag).toBe("Transcript")
    expect((transcript?.messages as Array<unknown>).length).toBe(0)
    expect(nodeByKey(view, "shell-composer")).toBeDefined()
  })

  test("withLoopProof increments and appends a system note", () => {
    const next = withLoopProof(baseState, "18:05")
    expect(next.loopProofs).toBe(1)
    expect(next.notes.length).toBe(1)
    expect(next.notes[0]?.role).toBe("system")
    expect(next.notes[0]?.timestamp).toBe("18:05")
  })

  test("Fleet staging remains an internal capability and does not enter the minimal chat surface", () => {
    const open = withFleetDesk(baseState)
    expect(open.fleetDeskOpen).toBe(true)
    expect(nodeByKey(desktopShellView(open), "fleet-desk")).toBeUndefined()
    expect(nodeByKey(desktopShellView(open), "shell-welcome")).toBeUndefined()

    const drafted = withFleetObjective(open, "Ship the desktop fleet chat")
    const dispatching = withFleetDeploymentRequested(drafted)
    expect(dispatching.fleetDeployment).toBe("dispatching")
    const staged = withFleetDeploymentResult(dispatching, {
      state: "accepted",
      message: "Local Pylon accepted the fleet brief.",
      intentStatus: "received",
    }, "18:05")
    expect(staged.fleetDeployment).toBe("accepted")
    expect(staged.notes.at(-1)?.text).toContain("accepted")
    expect(staged.notes.at(-1)?.text).not.toContain("runRef")
  })

  test("formatShellTimestamp is a zero-padded display string", () => {
    expect(formatShellTimestamp(new Date(2026, 6, 10, 9, 5))).toBe("09:05")
    expect(formatShellTimestamp(new Date(2026, 6, 10, 18, 45))).toBe("18:45")
  })

  test("relative sidebar timestamps stay compact", () => {
    const now = new Date("2026-07-10T18:10:00.000Z")
    expect(formatRelativeTimestamp("2026-07-10T18:09:30.000Z", now)).toBe("now")
    expect(formatRelativeTimestamp("2026-07-10T18:07:00.000Z", now)).toBe("3m")
    expect(formatRelativeTimestamp("2026-07-10T12:10:00.000Z", now)).toBe("6h")
  })
})

describe("typed chat intent loop end-to-end (registry -> state -> re-render)", () => {

  test("palette command uses the same workspace handler as the visible dock", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(withCommandPalette(baseState, true))
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const palette = desktopShellView(yield* SubscriptionRef.get(state))
        const command = nodeByKey(palette, "desktop-command-workspace.files") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(command.onPress, null))
        const after = yield* SubscriptionRef.get(state)
        expect(after.workspace).toBe("files")
        expect(after.commandPaletteOpen).toBe(false)
      }),
    )
  })

  test("workspace save sends one revision-bound request and requires explicit reload after conflict", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const file = {
          path: "/workspace/README.md",
          content: "before",
          revision: "revision-before",
          truncated: false,
        } as const
        const state = yield* SubscriptionRef.make(withWorkspaceFile(withWorkspaceSnapshot(withWorkspace(baseState, "files"), {
          root: "/workspace",
          label: "workspace",
          git: "clean",
          entries: [{ name: "README.md", path: "/workspace/README.md", kind: "file" }],
        }), file))
        const requests: Array<unknown> = []
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow, undefined, undefined, {
            summary: async () => null,
            choose: async () => null,
            readFile: async () => null,
            saveFile: async (input) => {
              requests.push(input)
              return {
                state: "conflict" as const,
                file: { ...file, content: "changed elsewhere", revision: "revision-current" },
              }
            },
            gitStatus: async () => ({ state: "unavailable" }),
            gitDiff: async () => ({ state: "unavailable", message: "Git review is unavailable." }),
          }),
        )
        const initial = desktopShellView(yield* SubscriptionRef.get(state))
        const editor = nodeByKey(initial, "workspace-file-editor") as {
          onChange: Parameters<typeof resolveIntentRef>[0]
        }
        const save = nodeByKey(initial, "workspace-file-save") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(editor.onChange, "local draft"))
        yield* registry.dispatch(resolveIntentRef(save.onPress, null))

        const conflicted = yield* SubscriptionRef.get(state)
        expect(requests).toEqual([{
          path: "/workspace/README.md",
          content: "local draft",
          expectedRevision: "revision-before",
        }])
        expect(conflicted.workspaceSave).toBe("conflict")
        expect(conflicted.workspaceDraft).toBe("local draft")
        expect(conflicted.workspaceFile?.content).toBe("changed elsewhere")
        const conflictView = desktopShellView(conflicted)
        expect(nodeByKey(conflictView, "workspace-file-save")?.disabled).toBe(true)
        const reload = nodeByKey(conflictView, "workspace-file-reload") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(reload.onPress, null))
        const reloaded = yield* SubscriptionRef.get(state)
        expect(reloaded.workspaceSave).toBe("idle")
        expect(reloaded.workspaceDraft).toBe("changed elsewhere")
        expect(reloaded.workspaceBaseRevision).toBe("revision-current")
      }),
    )
  })

  test("composer intents: input change then submit falls back to composer state on button press", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(baseState)
        const input = nodeByKey(view, "shell-input") as {
          onChange: Parameters<typeof resolveIntentRef>[0]
        }
        const note = nodeByKey(view, "shell-note") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        // TextField passes its component value; Button press passes null.
        yield* registry.dispatch(resolveIntentRef(input.onChange, "ship the shell"))
        yield* registry.dispatch(resolveIntentRef(note.onPress, null))

        const next = yield* SubscriptionRef.get(state)
        expect(next.input).toBe("")
        expect(next.notes[0]).toEqual({
          key: "error-1",
          role: "system",
          text: "Desktop chat is unavailable.",
          timestamp: "18:05",
        })
        expect(next.notes.some((entry) => entry.key.startsWith("pending-"))).toBe(false)
      }),
    )
  })

  test("submit resets the composer value binding and the composer stays usable (clear-on-submit contract)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(baseState)
        const input = nodeByKey(view, "shell-input") as {
          onChange: Parameters<typeof resolveIntentRef>[0]
          onSubmit: Parameters<typeof resolveIntentRef>[0]
        }

        // Type then submit through the field's own intent (Enter path): the
        // submit intent must reset the composer value binding to "".
        yield* registry.dispatch(resolveIntentRef(input.onChange, "first message"))
        expect((yield* SubscriptionRef.get(state)).input).toBe("first message")
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "first message"))

        const afterFirst = yield* SubscriptionRef.get(state)
        expect(afterFirst.input).toBe("")
        // the re-rendered TextField carries the emptied value + clearOnSubmit,
        // so the DOM renderer empties the focused input too (effect-native#72)
        const rerendered = nodeByKey(desktopShellView(afterFirst), "shell-input")
        expect(rerendered?.value).toBe("")
        expect(rerendered?.clearOnSubmit).toBe(true)
        expect(rerendered?.disabled).toBe(false)

        // the composer stays usable: a second round trip works end to end
        yield* registry.dispatch(resolveIntentRef(input.onChange, "second message"))
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "second message"))
        const afterSecond = yield* SubscriptionRef.get(state)
        expect(afterSecond.input).toBe("")
        expect(afterSecond.notes.at(-1)?.text).toBe("Desktop chat is unavailable.")
        expect(afterSecond.notes.length).toBe(2)
        expect(afterSecond.notes.some((entry) => entry.key.startsWith("pending-"))).toBe(false)
      }),
    )
  })

  test("harness selection dispatches through the registry and rides the next send", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const sent: Array<{ id: string; message: string; harness?: string }> = []
        const state = yield* SubscriptionRef.make(baseState)
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow, undefined, {
            listThreads: async () => [],
            newThread: async () => null,
            openThread: async () => null,
            sendMessage: async (input) => {
              sent.push({ id: input.id, message: input.message, harness: input.harness })
              return { ok: false, error: "Recorded only." }
            },
          }),
        )
        const view = desktopShellView(baseState)
        const fable = nodeByKey(view, "shell-harness-fable") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(fable.onPress, null))
        expect((yield* SubscriptionRef.get(state)).selectedHarness).toBe("fable")

        const input = nodeByKey(view, "shell-input") as {
          onSubmit: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "route me"))
        expect(sent).toEqual([{ id: testThread.id, message: "route me", harness: "fable" }])

        const codex = nodeByKey(view, "shell-harness-codex") as {
          onPress: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(codex.onPress, null))
        expect((yield* SubscriptionRef.get(state)).selectedHarness).toBe("codex")
      }),
    )
  })

  test("DesktopNewChat from a loaded history page yields an empty transcript bound to the new thread", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const loadedHistory: DesktopShellState = {
          ...baseState,
          history: {
            ...baseState.history,
            page: historyPageFixture,
            selectedItemRef: "item-1",
            expandedThreadRefs: ["history-root"],
          },
        }
        const state = yield* SubscriptionRef.make(loadedHistory)
        const freshThread = { id: "fresh-thread", title: "New chat", updatedAt: "2026-07-11T16:08:00.000Z", notes: [] }
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow, undefined, {
            listThreads: async () => [],
            newThread: async () => freshThread,
            openThread: async () => null,
            sendMessage: async () => ({ ok: false, error: "unused" }),
          }),
        )
        // The dock's first item is the New-chat affordance; dispatch through it.
        const before = desktopShellView(yield* SubscriptionRef.get(state))
        expect(nodeByKey(before, "history-workspace-split")).toBeDefined()
        const newChat = navItemById(before, "workspace-new-chat") as {
          onSelect: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(newChat.onSelect, null))

        const next = yield* SubscriptionRef.get(state)
        expect(next.workspace).toBe("chat")
        expect(next.activeThreadId).toBe("fresh-thread")
        expect(next.history.page).toBeNull()
        expect(next.notes).toHaveLength(0)
        const view = desktopShellView(next)
        expect(nodeByKey(view, "history-workspace-split")).toBeUndefined()
        const transcript = nodeByKey(view, "shell-transcript")
        expect(transcript?._tag).toBe("Transcript")
        expect((transcript?.messages as Array<unknown>).length).toBe(0)
      }),
    )
  })

  test("submit while pending is refused (disabled-while-pending contract)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(withPending(withInput(baseState, "held"), true))
        const registry = yield* makeIntentRegistry(
          desktopShellIntents,
          makeDesktopShellHandlers(state, fixedNow),
        )
        const view = desktopShellView(yield* SubscriptionRef.get(state))
        const input = nodeByKey(view, "shell-input") as {
          onSubmit: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(input.onSubmit, "held"))
        const next = yield* SubscriptionRef.get(state)
        expect(next.notes.length).toBe(0)
        expect(next.input).toBe("held")
      }),
    )
  })
})

describe("theme parity (one OpenAgents blue theme, many hosts)", () => {
  test("desktop theme keeps the shared token values", () => {
    expect(openagentsDesktopTheme.color.background).toBe("#03060b")
    expect(openagentsDesktopTheme.color.accent).toBe("#3a7bff")
    expect(openagentsDesktopTheme.color.border).toBe("#17315f")
    expect(openagentsDesktopTheme.color.focus).toBe("#4fd0ff")
    expect(openagentsDesktopTheme.color.textPrimary).toBe("#f1efe8")
  })
})
