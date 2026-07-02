import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Schema as S } from "effect"
import { Window } from "happy-dom"

import {
  KHALA_CODE_SOURCE_CONTROL_ACTION_SCHEMA,
  KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT,
  KhalaCodeSourceControlActionPromptSchema,
  khalaCodeSourceControlActionPrompt,
  khalaCodeSourceControlActionPromptText,
} from "../src/shared/source-control-action"
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

describe("Khala Code source-control AI actions", () => {
  beforeEach(installDom)
  afterEach(restoreDom)

  test("formats source-control prompts as structured steering notes", () => {
    const prompt = khalaCodeSourceControlActionPrompt({
      action: "commit_message",
      actionRef: "source_control_action.test.1",
      filePath: "src/example.ts",
      sourceRef: "diff.src/example.ts",
    })

    expect(S.decodeUnknownSync(KhalaCodeSourceControlActionPromptSchema)(prompt)).toEqual(prompt)
    const text = khalaCodeSourceControlActionPromptText(prompt)
    expect(text).toContain(`Source-control AI action (${KHALA_CODE_SOURCE_CONTROL_ACTION_SCHEMA})`)
    expect(text).toContain("Action: commit message")
    expect(text).toContain("File: src/example.ts")
    expect(text).toContain("do not run git commit")
  })

  test("diff headers emit typed source-control action details", () => {
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
    root.addEventListener(KHALA_CODE_SOURCE_CONTROL_ACTION_SUBMIT_EVENT, event => {
      submitted = (event as CustomEvent<unknown>).detail
    })

    const buttons = root.querySelectorAll<HTMLButtonElement>(".cb-diff-source-action-button")
    expect(buttons).toHaveLength(3)
    expect(buttons[0]?.textContent).toContain("Commit")
    expect(buttons[1]?.textContent).toContain("PR body")
    expect(buttons[2]?.textContent).toContain("Fix checks")

    buttons[1]?.click()
    expect(submitted).toMatchObject({
      action: "pr_body",
      filePath: "src/example.ts",
      sourceRef: "diff.src/example.ts",
    })
  })
})
