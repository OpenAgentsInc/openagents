import { describe, expect, test } from "bun:test"

import {
  assembleContext,
  type ContextAssemblyItem,
} from "../src/tas/context-assembly"

const item = (
  ref: string,
  priority: number,
  tokens: number,
  pinned = false,
): ContextAssemblyItem => ({
  ref,
  priority,
  tokens,
  pinned,
})

describe("tas context assembly core", () => {
  test("pinned items are always included", () => {
    expect(
      assembleContext(
        [
          item("context.pinned.large", 1, 200, true),
          item("context.unpinned.small", 10, 1),
        ],
        100,
      ),
    ).toEqual({
      included: ["context.pinned.large"],
      droppedRefs: ["context.unpinned.small"],
      usedTokens: 200,
    })
  })

  test("budget is respected for unpinned items", () => {
    expect(
      assembleContext(
        [
          item("context.a", 30, 40),
          item("context.b", 20, 30),
          item("context.c", 10, 31),
        ],
        70,
      ),
    ).toEqual({
      included: ["context.a", "context.b"],
      droppedRefs: ["context.c"],
      usedTokens: 70,
    })
  })

  test("higher priority wins under budget pressure", () => {
    expect(
      assembleContext(
        [
          item("context.low", 1, 50),
          item("context.high", 100, 50),
          item("context.medium", 50, 50),
        ],
        100,
      ),
    ).toEqual({
      included: ["context.high", "context.medium"],
      droppedRefs: ["context.low"],
      usedTokens: 100,
    })
  })

  test("deterministic tie-break sorts equal priority by ref", () => {
    expect(
      assembleContext(
        [
          item("context.c", 10, 10),
          item("context.a", 10, 10),
          item("context.b", 10, 10),
        ],
        20,
      ),
    ).toEqual({
      included: ["context.a", "context.b"],
      droppedRefs: ["context.c"],
      usedTokens: 20,
    })
  })
})
