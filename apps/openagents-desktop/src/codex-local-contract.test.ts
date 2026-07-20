import { describe, expect, test } from "vite-plus/test"
import { decodeCodexLocalContinuationProfile } from "./codex-local-contract.ts"
import { isCodexModel } from "./claude-local-contract.ts"

describe("Codex local continuation contract", () => {
  test("admits a non-default model from the installed app-server catalog", () => {
    const installedModels = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.4-mini"]

    expect(isCodexModel("gpt-5.6-terra")).toBe(true)
    expect(decodeCodexLocalContinuationProfile({
      accountRef: "ambient",
      model: "gpt-5.6-terra",
      reasoningEffort: "high",
    }, installedModels)).toEqual({
      accountRef: "ambient",
      model: "gpt-5.6-terra",
      reasoningEffort: "high",
    })
  })

  test("refuses a structurally valid model that is absent from the installed catalog", () => {
    expect(decodeCodexLocalContinuationProfile({
      model: "gpt-5.6-terra",
      reasoningEffort: "medium",
    }, ["gpt-5.6-sol"])).toEqual({
      accountRef: null,
      model: null,
      reasoningEffort: "medium",
    })
  })
})
