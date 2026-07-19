import { Schema } from "effect"

import { DesktopWorkspacePathRefSchema } from "./workspace-contract.ts"

const documentOpenArgumentPrefix = "--openagents-document-open="

export const DesktopLaunchContextSchema = Schema.Struct({
  documentOpenPathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
})

export type DesktopLaunchContext = typeof DesktopLaunchContextSchema.Type

export const decodeDesktopLaunchContext = (value: unknown): DesktopLaunchContext => {
  const decoded = Schema.decodeUnknownExit(DesktopLaunchContextSchema)(value)
  return decoded._tag === "Success" ? decoded.value : { documentOpenPathRef: null }
}

export const desktopDocumentOpenRendererArgument = (
  pathRef: typeof DesktopWorkspacePathRefSchema.Type,
): string => `${documentOpenArgumentPrefix}${encodeURIComponent(pathRef)}`

/**
 * Renderer-process arguments carry only the already-reduced workspace-relative
 * filename. The absolute Finder selection remains main-owned and never crosses
 * the preload boundary.
 */
export const desktopLaunchContextFromArgv = (
  argv: ReadonlyArray<string>,
): DesktopLaunchContext => {
  const encoded = argv.find(value => value.startsWith(documentOpenArgumentPrefix))
    ?.slice(documentOpenArgumentPrefix.length)
  if (encoded === undefined) return { documentOpenPathRef: null }
  try {
    const decoded = Schema.decodeUnknownExit(DesktopWorkspacePathRefSchema)(decodeURIComponent(encoded))
    return decoded._tag === "Success" &&
      !decoded.value.includes("/") && !decoded.value.includes("\\")
      ? { documentOpenPathRef: decoded.value }
      : { documentOpenPathRef: null }
  } catch {
    return { documentOpenPathRef: null }
  }
}
