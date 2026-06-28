import { describe, expect, test } from "bun:test"
import { appendPromptHistory } from "./input.js"

describe("prompt history", () => {
  test("appends submitted prompts and drops only duplicate tails", () => {
    expect(appendPromptHistory([], "hi")).toEqual(["hi"])
    expect(appendPromptHistory(["hi"], "hi")).toEqual(["hi"])
    expect(appendPromptHistory(["hi"], "there")).toEqual(["hi", "there"])
  })

  test("keeps the newest prompts within the limit", () => {
    expect(appendPromptHistory(["one", "two"], "three", 2)).toEqual(["two", "three"])
  })
})
