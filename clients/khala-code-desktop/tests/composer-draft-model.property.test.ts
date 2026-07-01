import { describe, expect, test } from "bun:test"
import fc from "fast-check"

import {
  applyComposerTransaction,
  composerBlockId,
  emptyComposerState,
  runComposerInputRules,
  serializeComposerMarkdown,
  type ComposerState,
  type ComposerTransaction,
} from "@openagentsinc/composer-state"

type DraftAction = Readonly<{
  kind: "type" | "paste" | "slash"
  text: string
  cursorBias: number
}>

const draftText = fc.string({ maxLength: 50 })

const draftActionArbitrary: fc.Arbitrary<DraftAction> = fc.record({
  kind: fc.constantFrom("type", "paste", "slash"),
  text: draftText,
  cursorBias: fc.integer({ min: 0, max: 1_000 }),
})

const firstBlockText = (state: ComposerState): string => {
  const block = state.doc.blocks[0]
  if (block === undefined || block.kind === "attachmentRef" || block.kind === "list") return ""
  return block.text
}

const apply = (state: ComposerState, transaction: ComposerTransaction): ComposerState => {
  const result = applyComposerTransaction(state, transaction)
  expect(result.ok).toBe(true)
  if (!result.ok) return state
  return result.state
}

const insertText = (
  state: ComposerState,
  text: string,
  source: ComposerTransaction["meta"]["source"],
  cursorBias: number,
): ComposerState => {
  const current = firstBlockText(state)
  const offset = current.length === 0 ? 0 : cursorBias % (current.length + 1)
  return apply(state, {
    steps: [{
      _tag: "InsertText",
      at: { blockId: composerBlockId("block-1"), offset },
      text,
    }],
    meta: { source, time: 1 },
  })
}

const runInputRules = (state: ComposerState, insertedText: string): ComposerState => {
  const transaction = runComposerInputRules(state, insertedText)
  return transaction === null ? state : apply(state, transaction)
}

describe("composer draft model properties", () => {
  test("arbitrary type, paste, and slash sequences preserve committed text", () => {
    fc.assert(
      fc.property(fc.array(draftActionArbitrary, { maxLength: 120 }), (actions) => {
        let state = emptyComposerState()
        let model = ""

        for (const action of actions) {
          const text = action.kind === "slash" ? `/${action.text}` : action.text
          const offset = model.length === 0 ? 0 : action.cursorBias % (model.length + 1)
          model = `${model.slice(0, offset)}${text}${model.slice(offset)}`
          state = insertText(
            state,
            text,
            action.kind === "paste" ? "paste" : "input",
            action.cursorBias,
          )
          state = runInputRules(state, text)
        }

        expect(firstBlockText(state)).toBe(model)
        expect(serializeComposerMarkdown(state.doc)).toContain(model)
      }),
      { numRuns: 150 },
    )
  })
})
