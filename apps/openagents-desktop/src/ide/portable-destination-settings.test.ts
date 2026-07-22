import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import { defaultDesktopPreferences } from "../desktop-preferences-contract.ts"
import {
  makeIdePortableDestinationSettingsActivation,
  type IdePortableDestinationSettingsProjection,
} from "./portable-destination-settings.ts"

describe("IDE portable destination settings", () => {
  test("reads destination Vim state and reapplies the product theme after attachment", async () => {
    const defaults = defaultDesktopPreferences()
    const destinationPreferences = {
      ...defaults,
      editor: { vim: { enabled: true } },
    }
    const applied: IdePortableDestinationSettingsProjection[] = []
    const activate = makeIdePortableDestinationSettingsActivation(
      { snapshot: () => destinationPreferences },
      { apply: value => Effect.sync(() => { applied.push(value) }) },
    )

    await Effect.runPromise(activate(
      "placement.destination",
      "attachment.destination.2",
      2,
    ))

    expect(applied).toHaveLength(1)
    expect(applied[0]).toMatchObject({
      destinationPlacementRef: "placement.destination",
      attachmentRef: "attachment.destination.2",
      generation: 2,
      vimEnabled: true,
      themeId: "khala-editor",
      theme: {
        id: "khala-editor",
        kind: "owned_static_data",
        recreatesModelsOrSessions: false,
        pierre: { themeName: "openagents-khala-editor" },
      },
    })
  })
})
