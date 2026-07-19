import { describe, expect, test } from "vite-plus/test"

import {
  macOSCodeDocumentContentTypes,
  macOSCodeDocumentExtensions,
  macOSCodeDocumentTypes,
  resolveMacOSDocumentOpenTarget,
} from "./macos-document-open.ts"

describe("macOS code document integration", () => {
  test("advertises editor-ranked system types and the owner's initial formats", () => {
    expect(macOSCodeDocumentContentTypes).toContain("public.source-code")
    expect(macOSCodeDocumentContentTypes).toContain("net.daringfireball.markdown")
    expect(macOSCodeDocumentExtensions).toEqual(expect.arrayContaining(["md", "js", "jsx", "ts", "tsx"]))
    expect(macOSCodeDocumentTypes).toHaveLength(2)
    expect(macOSCodeDocumentTypes.every(type =>
      type.CFBundleTypeRole === "Editor" && type.LSHandlerRank === "Alternate",
    )).toBe(true)
  })

  test("reduces an explicit absolute file selection to its parent and relative filename", () => {
    expect(resolveMacOSDocumentOpenTarget("/work/project/README.md", () => true)).toEqual({
      workspaceRoot: "/work/project",
      pathRef: "README.md",
    })
    expect(resolveMacOSDocumentOpenTarget("/work/project/App.TSX", () => true)).toEqual({
      workspaceRoot: "/work/project",
      pathRef: "App.TSX",
    })
  })

  test("rejects relative, unsupported, directory, and invalid path selections", () => {
    expect(resolveMacOSDocumentOpenTarget("README.md", () => true)).toBeNull()
    expect(resolveMacOSDocumentOpenTarget("/work/project/archive.zip", () => true)).toBeNull()
    expect(resolveMacOSDocumentOpenTarget("/work/project/source.ts", () => false)).toBeNull()
    expect(resolveMacOSDocumentOpenTarget("/work/project/../", () => true)).toBeNull()
  })
})
