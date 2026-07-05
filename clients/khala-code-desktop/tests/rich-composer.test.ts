import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  createKhalaComposerPromptHistory,
  insertKhalaComposerTextAtSelection,
  khalaRichComposerCommandForKey,
  normalizeKhalaComposerPasteText,
  readKhalaComposerPlainText,
  setKhalaComposerCaretToEnd,
  syncKhalaComposerEmptyState,
  writeKhalaComposerPlainText,
} from "../src/ui/rich-composer"

const createComposerElement = (): HTMLElement => {
  const window = new Window()
  const element = window.document.createElement("div")
  element.setAttribute("contenteditable", "plaintext-only")
  window.document.body.append(element)
  return element as unknown as HTMLElement
}

describe("khala rich composer", () => {
  test("round-trips multiline and trailing newline DOM content", () => {
    const element = createComposerElement()

    writeKhalaComposerPlainText(element, "first line\n\nthird line\n")

    expect(readKhalaComposerPlainText(element)).toBe("first line\n\nthird line\n")
    expect(element.dataset.empty).toBe("false")
    expect(element.querySelectorAll("br")).toHaveLength(3)
  })

  test("normalizes browser block wrappers into plain multiline text", () => {
    const element = createComposerElement()
    element.innerHTML = "<div>alpha</div><div>beta</div><div><br></div>"

    expect(readKhalaComposerPlainText(element)).toBe("alpha\nbeta\n")
  })

  test("normalizes pasted text before inserting into the editor", () => {
    const element = createComposerElement()
    writeKhalaComposerPlainText(element, "run")
    setKhalaComposerCaretToEnd(element)

    const paste = normalizeKhalaComposerPasteText("\r\nnpm\u00a0test\r")
    insertKhalaComposerTextAtSelection(element, paste)

    expect(paste).toBe("\nnpm test\n")
    expect(readKhalaComposerPlainText(element)).toBe("run\nnpm test\n")
  })

  test("keeps prompt history separate for normal and shell modes", () => {
    const history = createKhalaComposerPromptHistory(3)

    history.push("normal", "explain the diff")
    history.push("shell", "bun test")
    history.push("normal", "summarize failures")

    expect(history.all("normal")).toEqual(["explain the diff", "summarize failures"])
    expect(history.all("shell")).toEqual(["bun test"])
    expect(history.previous("normal", "")).toBe("summarize failures")
    expect(history.previous("shell", "")).toBe("bun test")
    expect(history.next("shell")).toBe("")
  })

  test("keeps empty state and keyboard submission predictable", () => {
    const element = createComposerElement()

    writeKhalaComposerPlainText(element, "")
    syncKhalaComposerEmptyState(element)

    expect(element.dataset.empty).toBe("true")
    expect(khalaRichComposerCommandForKey({ key: "Enter" }, "hello")).toBe("submit")
    expect(khalaRichComposerCommandForKey({ key: "Enter", shiftKey: true }, "hello")).toBe("newline")
    expect(khalaRichComposerCommandForKey({ isComposing: true, key: "Enter" }, "hello")).toBeNull()
    expect(khalaRichComposerCommandForKey({ key: "Enter", metaKey: true }, "hello")).toBeNull()
    expect(khalaRichComposerCommandForKey({ key: "ArrowUp" }, "")).toBe("history-previous")
    expect(khalaRichComposerCommandForKey({ key: "ArrowDown", altKey: true }, "draft")).toBe("history-next")
  })
})
