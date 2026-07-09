import { describe, expect, test } from "bun:test"

import {
  PSIONIC_QWEN_MODEL_REFS,
  selectPsionicQwenModel,
} from "../src/backends/psionic-qwen/model-admission.js"

describe("archived Psionic Qwen model admission", () => {
  test("publishes archived refs and refuses every retained selection mode", () => {
    expect(Object.values(PSIONIC_QWEN_MODEL_REFS)).toEqual([
      "model.psionic.qwen35.0_8b.q8_0.archived",
      "model.psionic.qwen35.2b.q8_0.archived",
    ])

    const admission = {
      rows: [],
      admittedModelRefs: [],
      observedModelRefs: [],
      blockerRefs: [],
    }
    expect(selectPsionicQwenModel(admission, "coding_agent")).toEqual({
      admitted: false,
      selectedModelRef: null,
      blockerRefs: ["blocker.psionic_qwen35.archived_to_backroom"],
    })
    expect(selectPsionicQwenModel(admission, "requires_2b")).toEqual({
      admitted: false,
      selectedModelRef: null,
      blockerRefs: ["blocker.psionic_qwen35.archived_to_backroom"],
    })
  })
})
