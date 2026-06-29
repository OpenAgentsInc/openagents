import { describe, expect, test } from "bun:test"

import { selectModel, type Provider } from "../src/tas/model-provider"

const providers: readonly Provider[] = [
  {
    id: "openai",
    available: true,
    models: [
      {
        id: "fast",
        capabilities: ["text", "tools"],
      },
      {
        id: "vision",
        capabilities: ["text", "vision"],
      },
    ],
  },
  {
    id: "anthropic",
    available: true,
    models: [
      {
        id: "deep",
        capabilities: ["text", "tools", "reasoning"],
      },
    ],
  },
]

describe("tas model provider core", () => {
  test("preferred model is chosen when available and capable", () => {
    const result = selectModel(
      {
        requiredCapabilities: ["text", "tools"],
        preferred: "deep",
      },
      providers,
    )

    expect(result.chosen?.provider.id).toBe("anthropic")
    expect(result.chosen?.model.id).toBe("deep")
    expect(result.fallbackOrder.map((selection) => selection.model.id)).toEqual([
      "fast",
      "deep",
    ])
    expect(result.reason).toBe("preferred_model_selected")
  })

  test("falls back when preferred provider is unavailable", () => {
    const result = selectModel(
      {
        requiredCapabilities: ["text", "tools"],
        preferred: "deep",
      },
      [
        providers[0],
        {
          ...providers[1],
          available: false,
        },
      ],
    )

    expect(result.chosen?.provider.id).toBe("openai")
    expect(result.chosen?.model.id).toBe("fast")
    expect(result.fallbackOrder.map((selection) => selection.model.id)).toEqual([
      "fast",
    ])
    expect(result.reason).toBe("preferred_model_unavailable_using_fallback")
  })

  test("filters fallback order by required capabilities", () => {
    const result = selectModel(
      {
        requiredCapabilities: ["vision"],
      },
      providers,
    )

    expect(result.chosen?.model.id).toBe("vision")
    expect(result.fallbackOrder.map((selection) => selection.model.id)).toEqual([
      "vision",
    ])
    expect(result.reason).toBe("first_available_capable_model_selected")
  })

  test("reports none available when no available model matches", () => {
    const result = selectModel(
      {
        requiredCapabilities: ["audio"],
        preferred: "deep",
      },
      providers,
    )

    expect(result).toEqual({
      fallbackOrder: [],
      reason: "no_available_model_matches_required_capabilities",
    })
  })
})
