import type { DesktopShellState } from "./shell.ts"

export const desktopPreviewReloadRisk = (state: DesktopShellState): boolean =>
  state.input.trim().length > 0 ||
  Object.values(state.composerDraftsByThread).some(draft => draft.trim().length > 0) ||
  state.composerImages.length > 0 ||
  state.composerReviewContext !== null ||
  state.composerFileContext !== null ||
  Object.values(state.questionCards).some(interaction =>
    interaction.submitting === true ||
    (!interaction.answered && (
      interaction.selections.some(selection => selection.length > 0) ||
      interaction.texts?.some(text => text.trim().length > 0) === true
    )),
  )
