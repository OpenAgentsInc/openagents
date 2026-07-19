import path from "node:path"

import { Schema } from "effect"

import { DesktopWorkspacePathRefSchema } from "./workspace-contract.ts"

/**
 * Text/code formats the current UTF-8 workspace editor can represent without
 * pretending to support binary project formats. Finder uses this bounded list
 * for extension-first matching when a more specific system UTI is absent
 * (notably TypeScript and TSX on some macOS releases).
 */
export const macOSCodeDocumentExtensions = [
  "md", "markdown", "mdx",
  "js", "mjs", "cjs", "jsx",
  "ts", "mts", "cts", "tsx",
  "json", "jsonc", "yaml", "yml", "toml",
  "css", "scss", "sass", "less", "html", "htm",
  "sh", "bash", "zsh", "fish",
  "py", "rb", "php",
  "go", "rs", "swift", "java", "kt", "kts",
  "c", "h", "cc", "cpp", "cxx", "hh", "hpp", "hxx",
  "cs", "fs", "fsx", "vb",
  "sql", "graphql", "gql",
  "vue", "svelte", "astro",
  "xml", "svg", "txt", "log", "ini", "cfg", "conf",
] as const

export const macOSCodeDocumentContentTypes = [
  "public.source-code",
  "net.daringfireball.markdown",
  "com.netscape.javascript-source",
  "public.json",
  "public.yaml",
  "public.plain-text",
] as const

export const macOSCodeDocumentTypes = [
  {
    CFBundleTypeName: "OpenAgents Code Document",
    CFBundleTypeRole: "Editor",
    LSHandlerRank: "Alternate",
    LSItemContentTypes: [...macOSCodeDocumentContentTypes],
  },
  {
    CFBundleTypeName: "OpenAgents Source File",
    CFBundleTypeRole: "Editor",
    LSHandlerRank: "Alternate",
    CFBundleTypeExtensions: [...macOSCodeDocumentExtensions],
  },
] as const

const supportedExtensions = new Set<string>(macOSCodeDocumentExtensions)

export type MacOSDocumentOpenTarget = Readonly<{
  workspaceRoot: string
  pathRef: typeof DesktopWorkspacePathRefSchema.Type
}>

/**
 * Converts a main-process-only absolute OS selection into the smallest
 * workspace grant that can use the existing editor: its containing directory
 * plus one relative filename. No absolute path crosses into renderer state.
 */
export const resolveMacOSDocumentOpenTarget = (
  selectedPath: string,
  isRegularFile: (absolutePath: string) => boolean,
): MacOSDocumentOpenTarget | null => {
  if (!path.isAbsolute(selectedPath) || !isRegularFile(selectedPath)) return null
  const absolutePath = path.resolve(selectedPath)
  const filename = path.basename(absolutePath)
  const extension = path.extname(filename).slice(1).toLowerCase()
  if (extension === "" || !supportedExtensions.has(extension)) return null
  const decoded = Schema.decodeUnknownExit(DesktopWorkspacePathRefSchema)(filename)
  if (decoded._tag !== "Success") return null
  return { workspaceRoot: path.dirname(absolutePath), pathRef: decoded.value }
}
