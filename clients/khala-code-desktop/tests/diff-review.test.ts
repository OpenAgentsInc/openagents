import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"
import { Schema as S } from "effect"

import {
  KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA,
  KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
  KhalaCodeDiffReviewCommentSchema,
  khalaCodeDiffReviewComment,
  khalaCodeDiffReviewSteeringNote,
} from "../src/shared/diff-review"
import { diffElement } from "../src/ui/transcript-render"

let previousDocument: typeof globalThis.document | undefined
let previousWindow: typeof globalThis.window | undefined
let previousCustomEvent: typeof globalThis.CustomEvent | undefined

const installDom = (): void => {
  const window = new Window()
  previousDocument = globalThis.document
  previousWindow = globalThis.window
  previousCustomEvent = globalThis.CustomEvent
  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "CustomEvent", { configurable: true, value: window.CustomEvent })
}

const restoreDom = (): void => {
  Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
  Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
  Object.defineProperty(globalThis, "CustomEvent", { configurable: true, value: previousCustomEvent })
}

describe("Khala Code diff review annotations", () => {
  beforeEach(installDom)
  afterEach(restoreDom)

  test("formats line comments as structured steering notes", () => {
    const comment = khalaCodeDiffReviewComment({
      body: "Please keep this export named; downstream imports rely on it.",
      commentRef: "diff_review.test.1",
      filePath: "src/example.ts",
      lineKind: "add",
      lineNo: 12,
      lineSide: "new",
      patchRef: "diff.src/example.ts.add.12",
    })

    expect(S.decodeUnknownSync(KhalaCodeDiffReviewCommentSchema)(comment)).toEqual(comment)
    expect(khalaCodeDiffReviewSteeringNote(comment)).toContain(
      `Diff review comment (${KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA})`,
    )
    expect(khalaCodeDiffReviewSteeringNote(comment)).toContain("Line: src/example.ts:+12")
    expect(khalaCodeDiffReviewSteeringNote(comment)).toContain(comment.body)
  })

  test("diff rows emit typed review comment submit details", () => {
    const root = diffElement({
      patch: [
        "--- a/src/example.ts",
        "+++ b/src/example.ts",
        "@@ -1 +1 @@",
        "-export const oldName = true",
        "+export const newName = true",
        "",
      ].join("\n"),
    })
    document.body.append(root)

    let submitted: unknown
    root.addEventListener(KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT, event => {
      submitted = (event as CustomEvent<unknown>).detail
    })

    const buttons = root.querySelectorAll<HTMLButtonElement>(".cb-diff-comment-button")
    expect(buttons).toHaveLength(2)
    buttons[1]?.click()

    const textarea = root.querySelector<HTMLTextAreaElement>(".cb-diff-review-textarea")
    expect(textarea).not.toBeNull()
    textarea!.value = "The rename needs a migration note."

    root.querySelector<HTMLButtonElement>(".cb-diff-review-submit")?.click()

    expect(submitted).toMatchObject({
      body: "The rename needs a migration note.",
      filePath: "src/example.ts",
      lineKind: "add",
      lineNo: 1,
      lineSide: "new",
      patchRef: "diff.src/example.ts.add.1",
    })
  })
})
