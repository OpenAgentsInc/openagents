import { Effect } from "effect"

import type { DesktopPreferencesStore } from "../desktop-preferences-host.ts"
import {
  defaultDesktopEditorThemeId,
  defaultDesktopEditorThemeProjection,
} from "./desktop-editor-themes.ts"
import type { IdePortableCoordinatorError } from "./portable-coordinator-service.ts"

export type IdePortableDestinationSettingBinding = Readonly<{
  destinationPlacementRef: string
  attachmentRef: string
  generation: number
}>

export type IdePortableDestinationSettingsProjection =
  & IdePortableDestinationSettingBinding
  & Readonly<{
    vimEnabled: boolean
    themeId: typeof defaultDesktopEditorThemeId
    theme: typeof defaultDesktopEditorThemeProjection
  }>

export interface IdePortableDestinationSettingsTarget {
  readonly apply: (
    projection: IdePortableDestinationSettingsProjection,
  ) => Effect.Effect<void, IdePortableCoordinatorError>
}

/**
 * Read destination-owned product settings after attachment and apply them to
 * the new editor runtime. A checkpoint does not participate in this operation.
 */
export const makeIdePortableDestinationSettingsActivation = (
  preferences: Pick<DesktopPreferencesStore, "snapshot">,
  target: IdePortableDestinationSettingsTarget,
) => Effect.fn("IdePortableDestinationSettings.activate")(function* (
  destinationPlacementRef: string,
  attachmentRef: string,
  generation: number,
) {
  const destinationPreferences = yield* Effect.sync(() => preferences.snapshot())
  yield* target.apply({
    destinationPlacementRef,
    attachmentRef,
    generation,
    vimEnabled: destinationPreferences.editor.vim.enabled,
    themeId: defaultDesktopEditorThemeId,
    theme: defaultDesktopEditorThemeProjection,
  })
})
