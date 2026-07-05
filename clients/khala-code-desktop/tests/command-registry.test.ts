import { describe, expect, test } from "bun:test"

import {
  createKhalaCodeCommandRegistry,
  khalaCodeCommandForKeyboardEvent,
  type KhalaCodeCommandDefinition,
} from "../src/ui/command-registry"

const command = (
  input: Partial<KhalaCodeCommandDefinition> & Pick<KhalaCodeCommandDefinition, "id" | "title">,
): KhalaCodeCommandDefinition => ({
  analyticsRef: `test.${input.id}`,
  category: "navigation",
  execute: () => undefined,
  ...input,
})

const keyboardEvent = (input: Partial<KeyboardEvent>): KeyboardEvent => ({
  altKey: false,
  ctrlKey: false,
  key: "",
  metaKey: false,
  shiftKey: false,
  ...input,
}) as KeyboardEvent

describe("Khala Code command registry", () => {
  test("selects palette records deterministically and keeps disabled commands explicit", () => {
    const registry = createKhalaCodeCommandRegistry([
      command({ id: "view.fleet", title: "Open Surface" }),
      command({ id: "view.chat", title: "Open Surface" }),
      command({
        available: () => false,
        disabledReason: () => "No active turn is running",
        id: "composer.stop_turn",
        title: "Stop Active Turn",
      }),
    ])

    expect(registry.search({ query: "open", limit: 3 }).map(result => result.id)).toEqual([
      "view.chat",
      "view.fleet",
    ])
    expect(registry.search({ includeDisabled: true, query: "stop" })).toEqual([
      expect.objectContaining({
        disabled: true,
        disabledReason: "No active turn is running",
        id: "composer.stop_turn",
      }),
    ])
    expect(registry.search({
      query: "server",
      records: [
        {
          group: "server",
          id: "server:session.refresh",
          kind: "server",
          metadataRef: "test.server.refresh",
          scoreHints: ["server"],
          subtitle: "Reload sessions from the desktop bridge",
          title: "Refresh Session Catalog",
        },
      ],
    })).toEqual([
      expect.objectContaining({
        id: "server:session.refresh",
        kind: "server",
      }),
    ])
    expect(registry.search({ query: "zzzz" })).toEqual([])
  })

  test("matches default keybindings through the central registry", () => {
    const registry = createKhalaCodeCommandRegistry([
      command({
        defaultKeybindings: [{ key: "k", meta: true }],
        id: "palette.open",
        title: "Open Command Palette",
      }),
    ])

    const event = keyboardEvent({ key: "k", metaKey: true })
    expect(khalaCodeCommandForKeyboardEvent(registry, event)).toBe("palette.open")
    expect(khalaCodeCommandForKeyboardEvent(
      registry,
      keyboardEvent({ key: "k" }),
    )).toBeNull()
  })
})
