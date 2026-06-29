// User-configurable keybinds for the Pylon TUI (issue #4738), following
// opencode's user-editable keybind config approach. The file lives in the
// Pylon home directory as `keybinds.json`:
//
//   { "bindings": { "palette.open": "ctrl+p", "logs.verbose-toggle": "f9" } }
//
// Keys are command names from the TUI command registry; values are
// @opentui/keymap key strings ("ctrl+k", "f1", "tab", ...). Invalid files
// are reported and ignored — a broken config must never block the node.

import { join } from "node:path"
import { Schema } from "effect"

export const keybindsFileName = "keybinds.json"

const KeybindsSchema = Schema.Struct({
  bindings: Schema.Record(Schema.String, Schema.String),
})

export type KeybindOverrides = Record<string, string>

export interface KeybindsLoadResult {
  overrides: KeybindOverrides
  path: string
  state: "loaded" | "absent" | "invalid"
  error?: string
}

export function parseKeybindsConfig(content: string): KeybindOverrides {
  const decoded = Schema.decodeUnknownSync(KeybindsSchema)(JSON.parse(content))
  return decoded.bindings
}

export async function loadKeybindOverrides(homeDir: string): Promise<KeybindsLoadResult> {
  const path = join(homeDir, keybindsFileName)
  const file = Bun.file(path)
  if (!(await file.exists())) {
    return { overrides: {}, path, state: "absent" }
  }
  try {
    const overrides = parseKeybindsConfig(await file.text())
    return { overrides, path, state: "loaded" }
  } catch (error) {
    return {
      overrides: {},
      path,
      state: "invalid",
      error: error instanceof Error ? error.message : String(error),
    }
  }
}
