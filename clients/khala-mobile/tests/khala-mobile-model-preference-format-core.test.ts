import { describe, expect, test } from "bun:test"

import {
  modelDisplayLabel,
  modelPreferenceFallbackMessage,
} from "../src/sync/khala-mobile-model-preference-format-core"

describe("modelDisplayLabel", () => {
  test("labels the gemini alias specially", () => {
    expect(modelDisplayLabel("gemini")).toBe("Gemini")
  })

  test("title-cases hyphenated/underscored ids", () => {
    expect(modelDisplayLabel("vertex-anthropic-claude")).toBe("Vertex Anthropic Claude")
  })

  test("uppercases short acronym-like parts", () => {
    expect(modelDisplayLabel("gpt-oss-120b")).toBe("GPT OSS 120b")
  })
})

describe("modelPreferenceFallbackMessage", () => {
  test("is silent for the expected/quiet fallback cases", () => {
    expect(modelPreferenceFallbackMessage("none")).toBeNull()
    expect(modelPreferenceFallbackMessage("no_preference_set")).toBeNull()
  })

  test("explains preference_unavailable", () => {
    expect(modelPreferenceFallbackMessage("preference_unavailable")).toContain("default instead")
  })

  test("explains default_unavailable", () => {
    expect(modelPreferenceFallbackMessage("default_unavailable")).toContain("Try again")
  })
})
