import { Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IdeBasicIdeAcceptanceReceiptSchema,
  IdeBasicIdeMatrixIdSchema,
} from "./basic-ide-acceptance-contract.ts"

describe("IDE-07 basic IDE acceptance contract", () => {
  test("freezes the complete fifteen-journey matrix", () => {
    expect(IdeBasicIdeMatrixIdSchema.literals).toEqual([
      "finder_cold_open",
      "explorer_at_scale",
      "rapid_switching",
      "editing_and_recovery",
      "conflict",
      "search_and_navigation",
      "versioned_review",
      "language_bursts",
      "vim_on_off",
      "keyboard_and_assistive_tech",
      "visual_and_accessibility",
      "offline_and_failure",
      "resource_disposal",
      "rollback",
      "chat_only_launch",
    ])
  })

  test("does not admit a partial or producer-overridable receipt", () => {
    expect(() => Schema.decodeUnknownSync(IdeBasicIdeAcceptanceReceiptSchema)({
      schemaVersion: "openagents.desktop.ide-basic-ide-acceptance.v1",
      review: { producerCanOverride: true },
    })).toThrow()
  })
})
