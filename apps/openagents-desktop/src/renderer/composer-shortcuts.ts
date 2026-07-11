/**
 * Composer keyboard shortcuts (EP250 owner statement, verbatim: "i want
 * shift+tab to togle between modes in composer (fable / codex) in this
 * case").
 *
 * Scope law: the gesture exists ONLY while the composer input has focus —
 * Shift+Tab anywhere else keeps normal reverse focus navigation (the same
 * editable-guard discipline the other boot.ts shortcuts follow, inverted:
 * this one requires the composer as the target). Plain Tab is untouched.
 *
 * Capability truth: toggling TO an unavailable lane is allowed — selection
 * moves and the disabled-reason popover / evidence-gated Send already
 * explain why that lane cannot act. The gesture is never silently blocked.
 */
import type { DesktopHarnessName } from "./shell.ts"

export const nextComposerHarness = (current: DesktopHarnessName): DesktopHarnessName =>
  current === "fable" ? "codex" : "fable"

export type ComposerShiftTabHooks = Readonly<{
  /** True when the event target is the composer input (shell-input). */
  isComposerInput: (target: unknown) => boolean
  selectedHarness: () => DesktopHarnessName
  /** Dispatches the SAME DesktopHarnessSelected intent the chips use. */
  selectHarness: (harness: DesktopHarnessName) => void
}>

/**
 * Returns true when the event was consumed (preventDefault called and the
 * harness toggled); false leaves the event to normal focus navigation.
 */
export const handleComposerShiftTab = (
  event: Readonly<{
    key: string
    shiftKey: boolean
    defaultPrevented: boolean
    target: unknown
    preventDefault: () => void
  }>,
  hooks: ComposerShiftTabHooks,
): boolean => {
  if (event.defaultPrevented || event.key !== "Tab" || !event.shiftKey) return false
  if (!hooks.isComposerInput(event.target)) return false
  event.preventDefault()
  hooks.selectHarness(nextComposerHarness(hooks.selectedHarness()))
  return true
}

/** The DOM guard boot.ts wires: target must be INSIDE the composer input. */
export const isShellComposerInputTarget = (target: unknown): boolean =>
  typeof HTMLElement !== "undefined" &&
  target instanceof HTMLElement &&
  target.closest('[data-en-key="shell-input"]') !== null
