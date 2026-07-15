/**
 * Composer focus on window open (#8787, owner verbatim: "the text input
 * should be focused immediately on open. so i can start typing right away.").
 *
 * Focus applies at SHELL-INTERACTABLE — the moment the shell mounts under the
 * branded boot frame (90bce8d89b boot ordering) — and again, only when focus
 * is UNOWNED, after background history hydration settles and on macOS window
 * re-activation. "Unowned" means the document's active element is the body /
 * root (nobody holds focus): a settle pass may claim unowned focus for the
 * composer but must NEVER steal focus the user has placed somewhere else
 * (sidebar search, an editor, a dialog).
 */

/** The one composer input — Lexical first, legacy form controls accepted. */
export const composerInputSelector =
  '[data-en-key="shell-input"] [contenteditable="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input'

export const findComposerInput = (root: ParentNode): HTMLElement | null =>
  root.querySelector<HTMLElement>(composerInputSelector)

const composerInputDisabled = (input: HTMLElement): boolean =>
  "disabled" in input && input.disabled === true

/**
 * Whether keyboard focus is currently unowned. Only then may an automatic
 * pass (post-hydration settle, window re-activate) move focus.
 */
export const focusIsUnowned = (doc: Document): boolean => {
  const active = doc.activeElement
  return active === null || active === doc.body || active === doc.documentElement
}

export type ComposerFocuserHooks = Readonly<{
  root: HTMLElement
  /** Injectable timer so tests drive retries deterministically. */
  setTimeout?: (callback: () => void, ms: number) => unknown
}>

/**
 * Retry-across-commits composer focus. A dispatch (New chat) or hydration can
 * (re)mount the composer on a LATER render commit than the triggering event,
 * and a re-parented input loses focus even when focused earlier — so retry
 * until the input exists AND holds focus, bounded.
 */
export const makeComposerFocuser = (hooks: ComposerFocuserHooks): (() => void) => {
  const schedule = hooks.setTimeout ?? ((callback: () => void, ms: number) => setTimeout(callback, ms))
  return () => {
    let attempts = 0
    const tryFocus = (): void => {
      const input = findComposerInput(hooks.root)
      if (input !== null && !composerInputDisabled(input)) {
        input.focus()
        if (hooks.root.ownerDocument.activeElement === input) return
      }
      attempts += 1
      if (attempts < 20) schedule(tryFocus, 16)
    }
    schedule(tryFocus, 0)
  }
}

/**
 * The guarded settle pass: claim unowned focus for the composer; never steal
 * owned focus. Used after history hydration lands and on window re-activate.
 */
export const makeComposerFocusSettler = (hooks: ComposerFocuserHooks): (() => void) => {
  const focus = makeComposerFocuser(hooks)
  return () => {
    if (focusIsUnowned(hooks.root.ownerDocument)) focus()
  }
}
