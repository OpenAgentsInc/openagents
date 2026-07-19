import type {
  DesktopCommandBindingProjection,
  DesktopCommandChord,
  DesktopCommandId,
} from "../desktop-command-contract.ts"

export type DesktopKeyboardShortcutEvent = Readonly<{
  key: string
  metaKey: boolean
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
  defaultPrevented: boolean
  repeat: boolean
}>

const eventChord = (event: DesktopKeyboardShortcutEvent): string => {
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key
  return [
    ...(event.metaKey ? ["Meta"] : []),
    ...(event.ctrlKey ? ["Control"] : []),
    ...(event.altKey ? ["Alt"] : []),
    ...(event.shiftKey ? ["Shift"] : []),
    key,
  ].join("+")
}

const platformBindings = (
  row: DesktopCommandBindingProjection["rows"][number],
  platform: string,
): ReadonlyArray<DesktopCommandChord> => {
  if (row.overrideBinding !== null) return row.effectiveBindings
  const primary = platform === "darwin" ? "Meta+" : "Control+"
  return row.effectiveBindings.filter(binding =>
    binding.startsWith(primary) || (!binding.startsWith("Meta+") && !binding.startsWith("Control+")))
}

/**
 * Matches one schema-decoded effective binding. Conflicts have no effective
 * bindings, platform defaults select one primary modifier, and editable
 * targets/repeat events are never intercepted by global Desktop commands.
 */
export const desktopCommandShortcutMatches = (
  projection: DesktopCommandBindingProjection | null,
  commandId: DesktopCommandId,
  platform: string,
  event: DesktopKeyboardShortcutEvent,
  editableTarget: boolean,
): boolean => {
  if (projection === null || event.defaultPrevented || event.repeat || editableTarget) return false
  const row = projection.rows.find(candidate => candidate.commandId === commandId)
  if (row === undefined || row.conflict) return false
  const chord = eventChord(event)
  return platformBindings(row, platform).some(binding => binding === chord)
}
