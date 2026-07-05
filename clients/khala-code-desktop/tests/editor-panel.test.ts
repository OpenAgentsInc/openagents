import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  mountKhalaCodeEditorPanel,
  type KhalaCodeEditorMonacoLoader,
  type KhalaCodeEditorPanelServices,
} from "../src/ui/editor-panel"
import type {
  KhalaCodeEditorTreeNode,
  KhalaCodeEditorWorkspaceRoot,
} from "../src/shared/editor"

type FakeMonacoSnapshot = {
  createOptions: unknown
  modelContent: string | null
  modelLanguage: string | null
  modelPath: string | null
}

const workspaceRoot: KhalaCodeEditorWorkspaceRoot = {
  label: "workspace",
  path: "/workspace",
  providerId: "local-workspace",
  readonly: false,
}

const node = (
  path: string,
  kind: KhalaCodeEditorTreeNode["kind"],
  input: {
    readonly depth: number
    readonly name: string
    readonly parentPath: string | null
    readonly sizeBytes?: number | null
  },
): KhalaCodeEditorTreeNode => ({
  childrenLoaded: false,
  depth: input.depth,
  kind,
  mtime: null,
  name: input.name,
  parentPath: input.parentPath,
  path,
  providerId: workspaceRoot.providerId,
  readonly: false,
  rootPath: workspaceRoot.path,
  sizeBytes: input.sizeBytes ?? (kind === "file" ? 12 : null),
  symlink: false,
})

const createServices = (
  overrides: {
    readonly binaryPath?: string
    readonly largePath?: string
  } = {},
): KhalaCodeEditorPanelServices & {
  readonly openedFiles: string[]
  readonly readDirectories: string[]
} => {
  const readDirectories: string[] = []
  const openedFiles: string[] = []
  const rootNode = node("/workspace", "directory", {
    depth: 0,
    name: "workspace",
    parentPath: null,
    sizeBytes: null,
  })
  const srcNode = node("/workspace/src", "directory", {
    depth: 1,
    name: "src",
    parentPath: "/workspace",
    sizeBytes: null,
  })
  const readmeNode = node("/workspace/README.md", "file", {
    depth: 1,
    name: "README.md",
    parentPath: "/workspace",
    sizeBytes: 24,
  })
  const binaryNode = node("/workspace/image.png", "file", {
    depth: 1,
    name: "image.png",
    parentPath: "/workspace",
    sizeBytes: 2048,
  })
  const mainNode = node("/workspace/src/main.ts", "file", {
    depth: 2,
    name: "main.ts",
    parentPath: "/workspace/src",
    sizeBytes: 18,
  })

  return {
    openedFiles,
    readDirectories,
    async editorDirectoryRead(request = {}) {
      const path = request.path ?? workspaceRoot.path
      readDirectories.push(path)
      if (path === "/workspace/src") {
        return {
          entries: [mainNode],
          node: { ...srcNode, childrenLoaded: true },
          ok: true,
          providerId: workspaceRoot.providerId,
          rootPath: workspaceRoot.path,
          truncated: false,
        }
      }
      return {
        entries: overrides.binaryPath === undefined && overrides.largePath === undefined
          ? [srcNode, readmeNode]
          : [binaryNode],
        node: { ...rootNode, childrenLoaded: true },
        ok: true,
        providerId: workspaceRoot.providerId,
        rootPath: workspaceRoot.path,
        truncated: false,
      }
    },
    async editorFileRead(request) {
      openedFiles.push(request.path)
      if (request.path === overrides.binaryPath) {
        return {
          error: {
            code: "binary_file",
            message: "Binary files are not rendered by the editor.",
            path: request.path,
            providerId: workspaceRoot.providerId,
          },
          ok: false,
        }
      }
      if (request.path === overrides.largePath) {
        return {
          error: {
            code: "file_too_large",
            message: "File is larger than the editor read cap.",
            path: request.path,
            providerId: workspaceRoot.providerId,
          },
          ok: false,
        }
      }
      return {
        content: "const answer = 42\n",
        encoding: "utf8",
        mtime: null,
        ok: true,
        path: request.path,
        providerId: workspaceRoot.providerId,
        rootPath: workspaceRoot.path,
        sizeBytes: 18,
      }
    },
    async editorWorkspaceRead() {
      return {
        ok: true,
        roots: [workspaceRoot],
      }
    },
  }
}

const createMonacoLoader = (): {
  readonly loadMonaco: KhalaCodeEditorMonacoLoader
  readonly snapshot: FakeMonacoSnapshot
  calls: number
} => {
  const snapshot: FakeMonacoSnapshot = {
    createOptions: null,
    modelContent: null,
    modelLanguage: null,
    modelPath: null,
  }
  const loader = {
    calls: 0,
    loadMonaco: async () => {
      loader.calls += 1
      return {
        Uri: {
          file: (path: string) => ({ path }),
        },
        editor: {
          create: (_host: HTMLElement, options: unknown) => {
            snapshot.createOptions = options
            return {
              dispose() {},
              layout() {},
              setModel() {},
            }
          },
          createModel: (content: string, language: string, uri: { readonly path: string }) => {
            snapshot.modelContent = content
            snapshot.modelLanguage = language
            snapshot.modelPath = uri.path
            return {
              dispose() {},
            }
          },
          defineTheme() {},
          setTheme() {},
        },
      } as unknown as Awaited<ReturnType<KhalaCodeEditorMonacoLoader>>
    },
    snapshot,
  }
  return loader
}

const withWindow = async (
  run: (window: Window, container: HTMLElement) => Promise<void>,
): Promise<void> => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousNavigator = globalThis.navigator
  const previousGetComputedStyle = globalThis.getComputedStyle
  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: window.navigator })
  Object.defineProperty(globalThis, "getComputedStyle", {
    configurable: true,
    value: window.getComputedStyle.bind(window),
  })
  const container = document.createElement("section")
  document.body.append(container)
  try {
    await run(window, container)
  } finally {
    Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
    Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: previousNavigator })
    Object.defineProperty(globalThis, "getComputedStyle", {
      configurable: true,
      value: previousGetComputedStyle,
    })
    window.close()
  }
}

const settle = async (): Promise<void> => {
  await Promise.resolve()
  await new Promise(resolve => setTimeout(resolve, 0))
  await Promise.resolve()
}

const rowByPath = (container: HTMLElement, path: string): HTMLButtonElement | null =>
  container.querySelector<HTMLButtonElement>(`[data-path="${path}"]`)

describe("Khala Code editor panel", () => {
  test("renders a lazy workspace tree and opens source in a read-only Monaco model", async () => {
    await withWindow(async (_window, container) => {
      const services = createServices()
      const monaco = createMonacoLoader()
      const handle = mountKhalaCodeEditorPanel(container, {
        ...services,
        loadMonaco: monaco.loadMonaco,
      })

      await handle.refresh()
      expect(monaco.calls).toBe(0)
      expect(services.readDirectories).toEqual(["/workspace"])
      expect(rowByPath(container, "/workspace")?.getAttribute("aria-expanded")).toBe("true")
      expect(rowByPath(container, "/workspace/src")?.getAttribute("role")).toBe("treeitem")
      expect(rowByPath(container, "/workspace/README.md")?.getAttribute("aria-level")).toBe("2")

      rowByPath(container, "/workspace/src")?.click()
      await settle()
      expect(services.readDirectories).toEqual(["/workspace", "/workspace/src"])
      expect(rowByPath(container, "/workspace/src/main.ts")).not.toBeNull()

      rowByPath(container, "/workspace/src/main.ts")?.click()
      await settle()
      expect(services.openedFiles).toEqual(["/workspace/src/main.ts"])
      expect(monaco.calls).toBe(1)
      expect(monaco.snapshot.modelContent).toBe("const answer = 42\n")
      expect(monaco.snapshot.modelLanguage).toBe("typescript")
      expect(monaco.snapshot.modelPath).toBe("/workspace/src/main.ts")
      expect(monaco.snapshot.createOptions).toMatchObject({
        automaticLayout: true,
        largeFileOptimizations: true,
        minimap: { enabled: false },
        readOnly: true,
      })

      handle.destroy()
    })
  })

  test("supports keyboard navigation for expansion, focus, and file open", async () => {
    await withWindow(async (window, container) => {
      const services = createServices()
      const monaco = createMonacoLoader()
      const handle = mountKhalaCodeEditorPanel(container, {
        ...services,
        loadMonaco: monaco.loadMonaco,
      })
      await handle.refresh()

      rowByPath(container, "/workspace")?.focus()
      const tree = container.querySelector<HTMLElement>('[role="tree"]')
      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "ArrowDown" }) as unknown as Event)
      await settle()
      expect(document.activeElement).toBe(rowByPath(container, "/workspace/src"))

      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "ArrowRight" }) as unknown as Event)
      await settle()
      expect(rowByPath(container, "/workspace/src/main.ts")).not.toBeNull()

      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "ArrowDown" }) as unknown as Event)
      await settle()
      expect(document.activeElement).toBe(rowByPath(container, "/workspace/src/main.ts"))

      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "Enter" }) as unknown as Event)
      await settle()
      expect(services.openedFiles).toEqual(["/workspace/src/main.ts"])

      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "Home" }) as unknown as Event)
      await settle()
      expect(document.activeElement).toBe(rowByPath(container, "/workspace"))
      tree?.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "End" }) as unknown as Event)
      await settle()
      expect(document.activeElement).toBe(rowByPath(container, "/workspace/README.md"))

      handle.destroy()
    })
  })

  test("renders binary files as unsupported without loading Monaco", async () => {
    await withWindow(async (_window, container) => {
      const services = createServices({ binaryPath: "/workspace/image.png" })
      const monaco = createMonacoLoader()
      const handle = mountKhalaCodeEditorPanel(container, {
        ...services,
        loadMonaco: monaco.loadMonaco,
      })

      await handle.refresh()
      rowByPath(container, "/workspace/image.png")?.click()
      await settle()

      expect(services.openedFiles).toEqual(["/workspace/image.png"])
      expect(monaco.calls).toBe(0)
      expect(container.querySelector(".khala-code-editor-source-title")?.textContent).toBe("Unsupported file")
      expect(container.querySelector(".khala-code-editor-source-state")?.textContent).toContain("Binary files")

      handle.destroy()
    })
  })

  test("renders oversized files as unsupported without loading Monaco", async () => {
    await withWindow(async (_window, container) => {
      const services = createServices({ largePath: "/workspace/image.png" })
      const monaco = createMonacoLoader()
      const handle = mountKhalaCodeEditorPanel(container, {
        ...services,
        loadMonaco: monaco.loadMonaco,
      })

      await handle.refresh()
      rowByPath(container, "/workspace/image.png")?.click()
      await settle()

      expect(services.openedFiles).toEqual(["/workspace/image.png"])
      expect(monaco.calls).toBe(0)
      expect(container.querySelector(".khala-code-editor-source-title")?.textContent).toBe("File too large")
      expect(container.querySelector(".khala-code-editor-source-state")?.textContent).toContain("read cap")

      handle.destroy()
    })
  })
})
