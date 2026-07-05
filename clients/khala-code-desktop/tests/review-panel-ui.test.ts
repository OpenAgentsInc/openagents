import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  mountKhalaCodeReviewPanel,
  khalaCodeReviewPanelFileButton,
  type KhalaCodeReviewPanelOptions,
  type KhalaCodeReviewPanelStorage,
} from "../src/ui/review-panel"
import type { KhalaCodeDesktopReviewDiffReadResult } from "../src/shared/rpc"
import { khalaCodeDiffReviewComment } from "../src/shared/diff-review"

const withWindow = async (
  run: (window: Window, container: HTMLElement) => Promise<void>,
): Promise<void> => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousCustomEvent = globalThis.CustomEvent
  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "CustomEvent", { configurable: true, value: window.CustomEvent })
  const container = document.createElement("section")
  document.body.append(container)
  try {
    await run(window, container)
  } finally {
    Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
    Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    Object.defineProperty(globalThis, "CustomEvent", { configurable: true, value: previousCustomEvent })
    window.close()
  }
}

const settle = async (): Promise<void> => {
  await Promise.resolve()
  await new Promise(resolve => setTimeout(resolve, 0))
  await Promise.resolve()
}

const memoryStorage = (): KhalaCodeReviewPanelStorage => {
  const store = new Map<string, string>()
  return {
    getItem: key => store.get(key) ?? null,
    setItem: (key, value) => {
      store.set(key, value)
    },
  }
}

const mixedDiffResult: KhalaCodeDesktopReviewDiffReadResult = {
  files: [
    { additions: 1, deletions: 0, diffKind: "added", path: "src/new-thing.ts" },
    { additions: 2, deletions: 1, diffKind: "modified", path: "src/existing.ts" },
    { additions: 0, deletions: 3, diffKind: "deleted", path: "src/gone.ts" },
  ],
  ok: true,
  sha: "abc1234",
  truncated: false,
}

const emptyDiffResult: KhalaCodeDesktopReviewDiffReadResult = {
  files: [],
  ok: true,
  sha: null,
  truncated: false,
}

const panelOptions = (
  overrides: Partial<KhalaCodeReviewPanelOptions> = {},
): KhalaCodeReviewPanelOptions => ({
  reviewDiffRead: async () => mixedDiffResult,
  storage: memoryStorage(),
  ...overrides,
})

describe("Khala Code review panel", () => {
  test("renders added, modified, and deleted files as distinct review states", async () => {
    await withWindow(async (_window, container) => {
      const handle = mountKhalaCodeReviewPanel(container, panelOptions())
      handle.setVisible(true)
      await settle()

      const groups = container.querySelectorAll<HTMLElement>(".khala-code-review-group")
      expect(groups).toHaveLength(3)
      expect([...groups].map(group => group.dataset.diffKind)).toEqual(["added", "modified", "deleted"])

      const added = khalaCodeReviewPanelFileButton(container, "src/new-thing.ts")
      const modified = khalaCodeReviewPanelFileButton(container, "src/existing.ts")
      const deleted = khalaCodeReviewPanelFileButton(container, "src/gone.ts")
      expect(added?.dataset.diffKind).toBe("added")
      expect(modified?.dataset.diffKind).toBe("modified")
      expect(deleted?.dataset.diffKind).toBe("deleted")
      expect(modified?.textContent).toContain("+2/-1")

      handle.destroy()
    })
  })

  test("shows an explicit empty/no-change state when the diff has no files", async () => {
    await withWindow(async (_window, container) => {
      const handle = mountKhalaCodeReviewPanel(container, panelOptions({
        reviewDiffRead: async () => emptyDiffResult,
      }))
      handle.setVisible(true)
      await settle()

      const filesRegion = container.querySelector<HTMLElement>(".khala-code-review-files")
      expect(filesRegion?.dataset.state).toBe("empty")
      expect(filesRegion?.textContent).toContain("No changes to review")
      expect(container.querySelectorAll(".khala-code-review-group")).toHaveLength(0)

      handle.destroy()
    })
  })

  test("surfaces a provider_unavailable error state instead of pretending the diff loaded", async () => {
    await withWindow(async (_window, container) => {
      const handle = mountKhalaCodeReviewPanel(container, panelOptions({
        reviewDiffRead: async () => ({
          error: { code: "provider_unavailable", message: "Codex app-server host is not configured." },
          ok: false,
        }),
      }))
      handle.setVisible(true)
      await settle()

      const filesRegion = container.querySelector<HTMLElement>(".khala-code-review-files")
      expect(filesRegion?.dataset.state).toBe("error")
      expect(filesRegion?.textContent).toContain("Codex app-server host is not configured.")

      handle.destroy()
    })
  })

  test("revert is explicit, unavailable, and keyboard reachable rather than silently disabled", async () => {
    await withWindow(async (_window, container) => {
      const handle = mountKhalaCodeReviewPanel(container, panelOptions())
      handle.setVisible(true)
      await settle()

      khalaCodeReviewPanelFileButton(container, "src/existing.ts")?.click()
      await settle()

      const revertButton = container.querySelector<HTMLButtonElement>(".khala-code-review-revert-button")
      expect(revertButton).not.toBeNull()
      // aria-disabled (not the `disabled` attribute) keeps the control in the
      // tab order so it stays keyboard reachable even while unavailable.
      expect(revertButton?.getAttribute("aria-disabled")).toBe("true")
      expect(revertButton?.disabled).toBe(false)
      expect(revertButton?.getAttribute("aria-label")).toContain("no safe backend")

      const status = container.querySelector<HTMLElement>(".khala-code-review-revert-status")
      expect(status?.hidden).toBe(true)
      revertButton?.click()
      expect(status?.hidden).toBe(false)
      expect(status?.textContent).toContain("no safe backend")

      handle.destroy()
    })
  })

  test("renders comments added via the handle and via the in-panel comment form", async () => {
    await withWindow(async (_window, container) => {
      let submittedDetail: unknown
      const handle = mountKhalaCodeReviewPanel(container, panelOptions({
        onCommentSubmit: detail => {
          submittedDetail = detail
        },
      }))
      handle.setVisible(true)
      await settle()

      const emptyComments = container.querySelector<HTMLElement>(".khala-code-review-comments")
      expect(emptyComments?.textContent).toContain("No review comments yet")

      handle.addComment(khalaCodeDiffReviewComment({
        body: "Looks good, one nit below.",
        commentRef: "comment.1",
        filePath: "src/existing.ts",
        lineKind: "add",
        lineNo: 12,
        lineSide: "new",
        patchRef: "diff.src/existing.ts.add.12",
      }))
      await settle()
      expect(container.querySelectorAll(".khala-code-review-comment-item")).toHaveLength(1)
      expect(container.textContent).toContain("Looks good, one nit below.")

      khalaCodeReviewPanelFileButton(container, "src/existing.ts")?.click()
      await settle()
      const textarea = container.querySelector<HTMLTextAreaElement>(".khala-code-review-comment-textarea")
      expect(textarea).not.toBeNull()
      textarea!.value = "Please add a regression test for this path."
      container.querySelector<HTMLButtonElement>(".khala-code-review-comment-submit")?.click()

      expect(submittedDetail).toMatchObject({
        body: "Please add a regression test for this path.",
        filePath: "src/existing.ts",
        lineKind: "context",
        lineNo: 0,
        lineSide: "new",
      })

      handle.destroy()
    })
  })

  test("persists side-panel layout (tab, collapsed, width) across remounts of the same panel", async () => {
    await withWindow(async (_window, container) => {
      const storage = memoryStorage()
      const first = mountKhalaCodeReviewPanel(container, panelOptions({ storage }))
      first.setVisible(true)
      await settle()

      container.querySelector<HTMLButtonElement>('[data-review-tab="comments"]')?.click()
      container.querySelector<HTMLButtonElement>(".khala-code-review-collapse-button")?.click()
      container.querySelector<HTMLButtonElement>(".khala-code-review-width-button")?.click()
      first.destroy()

      const second = mountKhalaCodeReviewPanel(container, panelOptions({ storage }))
      const panel = container.querySelector<HTMLElement>(".khala-code-review-panel")
      expect(panel?.dataset.activeTab).toBe("comments")
      expect(panel?.dataset.collapsed).toBe("true")
      second.destroy()
    })
  })

  test("review state (loaded files) remains stable across repeated setVisible toggles, as when switching sessions", async () => {
    await withWindow(async (_window, container) => {
      let readCount = 0
      const handle = mountKhalaCodeReviewPanel(container, panelOptions({
        reviewDiffRead: async () => {
          readCount += 1
          return mixedDiffResult
        },
      }))
      handle.setVisible(true)
      await settle()
      expect(readCount).toBe(1)
      expect(container.querySelectorAll(".khala-code-review-group")).toHaveLength(3)

      // Simulate switching away to another view (e.g. a different Codex
      // thread/session) and back; the panel must not lose or refetch state.
      handle.setVisible(false)
      handle.setVisible(true)
      await settle()
      expect(readCount).toBe(1)
      expect(container.querySelectorAll(".khala-code-review-group")).toHaveLength(3)

      handle.destroy()
    })
  })
})
