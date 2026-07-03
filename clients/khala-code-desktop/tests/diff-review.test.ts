import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"
import { Schema as S } from "effect"

import {
  KHALA_CODE_DIFF_REVIEW_COMMENT_SCHEMA,
  KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT,
  KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA,
  KhalaCodeDiffReviewCommentSchema,
  KhalaCodeJudgeDiffVerdictSchema,
  khalaCodeDiffReviewComment,
  khalaCodeDiffReviewSteeringNote,
  khalaCodeJudgeDiffFindingToReviewDetail,
  type KhalaCodeJudgeDiffVerdict,
} from "../src/shared/diff-review"
import { diffElement, judgeDiffVerdictElement, parseMessageSegments } from "../src/ui/transcript-render"

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

  const verdictFixture = (
    verdict: KhalaCodeJudgeDiffVerdict["verdict"],
  ): KhalaCodeJudgeDiffVerdict => ({
    confidence: verdict === "accept" ? 0.91 : verdict === "request_changes" ? 0.84 : 0.72,
    diffRef: "diff.fixture",
    findings: verdict === "accept"
      ? []
      : [{
          body: verdict === "replan"
            ? "The diff solves a different problem than the approved plan."
            : "The new branch drops the retry guard used by the worker closeout path.",
          confidence: 0.87,
          filePath: "src/worker.ts",
          findingRef: `judge.finding.${verdict}.1`,
          lineStart: 42,
          priority: verdict === "replan" ? "P1" : "P2",
          title: verdict === "replan" ? "Plan no longer matches implementation" : "Retry guard was removed",
        }],
    schema: KHALA_CODE_JUDGE_DIFF_VERDICT_SCHEMA,
    summary: `${verdict} fixture verdict`,
    verdict,
    verifyAuthority: "verify_command_required",
  })

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

  test("decodes judge verdict fixtures with priority and confidence fields", () => {
    for (const verdict of ["accept", "request_changes", "replan"] as const) {
      const fixture = verdictFixture(verdict)
      expect(S.decodeUnknownSync(KhalaCodeJudgeDiffVerdictSchema)(fixture)).toEqual(fixture)
      for (const finding of fixture.findings) {
        expect(finding.priority).toMatch(/^P[0-3]$/)
        expect(finding.confidence).toBeGreaterThan(0)
      }
    }
  })

  test("renders judge verdict cards for every verdict kind", () => {
    for (const verdict of ["accept", "request_changes", "replan"] as const) {
      const root = judgeDiffVerdictElement(verdictFixture(verdict))
      expect(root.className).toContain("judge-verdict-card")
      expect(root.dataset.verdict).toBe(verdict)
      expect(root.textContent).toContain("Judge")
      expect(root.textContent).toContain("Advisory verdict only")
      expect(root.textContent).toContain("verify command remains the merge authority")
    }
  })

  test("parses judge verdict JSON blocks into transcript verdict segments", () => {
    const fixture = verdictFixture("request_changes")
    const segments = parseMessageSegments([
      "Judge result:",
      "",
      "```json",
      JSON.stringify(fixture),
      "```",
    ].join("\n"))

    expect(segments.map(segment => segment.kind)).toEqual(["prose", "judge-diff-verdict"])
  })

  test("request-change verdict findings feed the diff annotation steering event", () => {
    const fixture = verdictFixture("request_changes")
    const root = judgeDiffVerdictElement(fixture)
    document.body.append(root)

    let submitted: unknown
    root.addEventListener(KHALA_CODE_DIFF_REVIEW_SUBMIT_EVENT, event => {
      submitted = (event as CustomEvent<unknown>).detail
    })

    root.querySelector<HTMLButtonElement>(".judge-verdict-steer-button")?.click()

    expect(submitted).toMatchObject(khalaCodeJudgeDiffFindingToReviewDetail(fixture.findings[0]!))
  })
})
