import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"
import { Schema } from "effect"

import {
  DesktopCommandBindingUpdate,
  desktopCanonicalCommandRegistry,
  resolveDesktopCommandBindings,
  type DesktopCommandBindingProjection,
  type DesktopCommandChord,
  type DesktopCommandId,
} from "./desktop-command-contract"

const StoredBindings = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.command_bindings.store.v1"),
  overrides: Schema.Array(DesktopCommandBindingUpdate).check(Schema.isMaxLength(64)),
})
type StoredBindings = typeof StoredBindings.Type

const empty = (): StoredBindings => ({
  schema: "openagents.desktop.command_bindings.store.v1",
  overrides: [],
})

const read = (filePath: string): StoredBindings => {
  try {
    const decoded = Schema.decodeUnknownExit(StoredBindings)(JSON.parse(readFileSync(filePath, "utf8")))
    return decoded._tag === "Success" ? decoded.value : empty()
  } catch {
    return empty()
  }
}

const write = (filePath: string, value: StoredBindings): void => {
  mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 })
  chmodSync(path.dirname(filePath), 0o700)
  const temporary = `${filePath}.tmp`
  writeFileSync(temporary, `${JSON.stringify(value)}\n`, { mode: 0o600 })
  chmodSync(temporary, 0o600)
  renameSync(temporary, filePath)
  chmodSync(filePath, 0o600)
}

const projection = (stored: StoredBindings): DesktopCommandBindingProjection => {
  const overrides = stored.overrides.filter((value): value is DesktopCommandBindingUpdate & { chord: DesktopCommandChord } => value.chord !== null)
  const resolved = resolveDesktopCommandBindings(overrides)
  const conflicts = new Set(resolved.conflicts.flatMap(value => value.commandIds))
  return {
    schema: "openagents.desktop.command_bindings.v1",
    rows: desktopCanonicalCommandRegistry.map(command => ({
      commandId: command.id,
      label: command.label,
      defaultBindings: command.defaultBindings,
      overrideBinding: overrides.find(value => value.commandId === command.id)?.chord ?? null,
      effectiveBindings: resolved.bindings.filter(value => value.commandId === command.id).map(value => value.chord),
      conflict: conflicts.has(command.id),
      source: conflicts.has(command.id) ? "conflicted" as const
        : overrides.some(value => value.commandId === command.id) ? "user" as const
        : command.defaultBindings.length === 0 ? "unassigned" as const : "default" as const,
      platform: process.platform === "darwin" ? "darwin" as const : process.platform === "win32" ? "win32" as const : "linux" as const,
      context: command.scope,
      vimPrecedence: command.id.startsWith("editor.") ? "vim_scoped" as const : "app_before_vim" as const,
      conflictKinds: conflicts.has(command.id) ? ["exact" as const] : [],
    })),
    conflicts: resolved.conflicts,
  }
}

export type DesktopCommandBindingStore = Readonly<{
  snapshot: () => DesktopCommandBindingProjection
  save: (update: DesktopCommandBindingUpdate) => DesktopCommandBindingProjection
  reset: () => DesktopCommandBindingProjection
}>

export const openDesktopCommandBindingStore = (filePath: string): DesktopCommandBindingStore => ({
  snapshot: () => projection(read(filePath)),
  save: update => {
    const current = read(filePath)
    const next: StoredBindings = {
      ...current,
      overrides: [
        ...current.overrides.filter(value => value.commandId !== update.commandId),
        ...(update.chord === null ? [] : [update]),
      ],
    }
    write(filePath, next)
    return projection(next)
  },
  reset: () => {
    const next = empty()
    write(filePath, next)
    return projection(next)
  },
})

export const commandBindingForNativeMenu = (
  projection: DesktopCommandBindingProjection,
  commandId: DesktopCommandId,
): string | undefined => projection.rows.find(value => value.commandId === commandId)?.effectiveBindings[0]
